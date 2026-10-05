/* Runs the C listing in the semantic-conflicts guide. */
#include <postproject/postproject.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* [semantic-conflicts] */
static pp_error_code_t latest_revision(pp_production_t *production,
                                       pp_revision_id_t *out_id,
                                       uint64_t *out_sequence,
                                       pp_error_t **error) {
  pp_revision_set_t *revisions = NULL;
  pp_transaction_id_t transaction_id;
  int64_t committed_at = 0;
  const char *origin_name = NULL;
  const char *origin_version = NULL;
  const char *origin_uri = NULL;
  const char *message = NULL;
  pp_error_code_t status =
      pp_production_latest_revision(production, &revisions, error);
  if (status == PP_OK && pp_revision_set_count(revisions) != UINT64_C(1)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = pp_revision_set_get(
        revisions, 0, out_id, out_sequence, &transaction_id, &committed_at,
        &origin_name, &origin_version, &origin_uri, &message, error);
  }
  pp_revision_set_release(revisions);
  return status;
}

static pp_error_code_t
update_from_a_base_revision(pp_production_t *production, pp_error_t **error) {
  pp_uuid_t root_id;
  pp_revision_id_t base_id;
  pp_revision_id_t superseding_id;
  uint64_t base_sequence = 0;
  uint64_t superseding_sequence = 0;
  pp_transaction_t *transaction = NULL;

  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_add_media_root(transaction, "rushes", NULL, 0,
                                           &root_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  if (status == PP_OK) {
    status = latest_revision(production, &base_id, &base_sequence, error);
  }

  if (status == PP_OK) {
    status = pp_production_begin_transaction_at(production, base_id,
                                                &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_set_media_root_enabled(transaction, &root_id, 0,
                                                   error);
  }
  if (status == PP_OK) {
    pp_commit_receipt_t receipt;
    status = pp_transaction_commit_with_receipt(transaction, &receipt, error);
    if (status == PP_OK && receipt.outcome != PP_COMMIT_REVISION_CREATED)
      status = PP_ERROR_INTERNAL;
    if (status == PP_OK) {
      superseding_id = receipt.revision_id;
      superseding_sequence = receipt.revision_sequence;
    }
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  if (status == PP_OK) {
    status = pp_production_begin_transaction_at(production, base_id,
                                                &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_set_media_root_enabled(transaction, &root_id, 1,
                                                   error);
  }
  if (status == PP_OK) {
    pp_error_t *conflict_error = NULL;
    pp_transaction_conflict_t conflict;
    status = pp_transaction_commit(transaction, &conflict_error);
    if (status == PP_ERROR_CONFLICT &&
        pp_error_transaction_conflict(conflict_error, &conflict) != 0 &&
        conflict.kind == PP_CONFLICT_MEDIA_ROOT &&
        memcmp(conflict.target.id.bytes, root_id.bytes, sizeof(root_id.bytes)) ==
            0 &&
        memcmp(conflict.base_revision_id.bytes, base_id.bytes,
               sizeof(base_id.bytes)) == 0 &&
        conflict.base_revision_sequence == base_sequence &&
        memcmp(conflict.superseding_revision_id.bytes, superseding_id.bytes,
               sizeof(superseding_id.bytes)) == 0 &&
        conflict.superseding_revision_sequence == superseding_sequence) {
      status = PP_OK;
    }
    pp_error_release(conflict_error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  /* Retry only after re-reading and deciding that enabling is still right. */
  pp_revision_id_t refreshed_id;
  uint64_t refreshed_sequence = 0;
  if (status == PP_OK) {
    status = latest_revision(production, &refreshed_id, &refreshed_sequence,
                             error);
  }
  if (status == PP_OK) {
    status = pp_production_begin_transaction_at(production, refreshed_id,
                                                &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_set_media_root_enabled(transaction, &root_id, 1,
                                                   error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  return status;
}
/* [/semantic-conflicts] */

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: postproject-c-conflicts WORK_DIRECTORY\n");
    return 2;
  }
  char path[4096];
  if (snprintf(path, sizeof(path), "%s/conflicts.pproj", argv[1]) < 0) {
    return 2;
  }
  pp_error_t *error = NULL;
  pp_production_t *production = NULL;
  pp_error_code_t status = pp_production_create(path, NULL, &production, &error);
  if (status == PP_OK) {
    status = update_from_a_base_revision(production, &error);
  }
  if (status != PP_OK) {
    fprintf(stderr, "operation failed (%u): %s\n", status,
            error != NULL ? pp_error_message(error) : "no details");
  }
  pp_error_release(error);
  pp_production_release(production);
  return status == PP_OK ? EXIT_SUCCESS : EXIT_FAILURE;
}
