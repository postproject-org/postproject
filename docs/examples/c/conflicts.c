/* Runs the C listing in the semantic-conflicts guide. */
#include <postproject/postproject.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* [semantic-conflicts] */
static pp_error_code_t decision_base(pp_production_t *production,
                                      pp_decision_base_t *out_base,
                                      pp_error_t **error) {
  pp_read_session_t *view = NULL;
  pp_error_code_t status = pp_production_read_session(production, &view, error);
  if (status == PP_OK)
    status = pp_read_session_decision_base(view, out_base, error);
  pp_read_session_release(view);
  return status;
}

static pp_error_code_t
update_from_a_base_revision(pp_production_t *production, pp_error_t **error) {
  pp_media_root_id_t root_id = {0};
  pp_decision_base_t base = {0};
  pp_revision_id_t superseding_id;
  uint64_t superseding_sequence = 0;
  pp_transaction_t *transaction = NULL;

  /* Saved UUID text is parsed into its semantic type at interchange. */
  pp_media_root_id_t parsed_root;
  char *root_text = NULL;
  pp_error_code_t parsed = pp_media_root_id_parse(
      "00000000-0000-0000-0000-000000000001", &parsed_root, error);
  if (parsed == PP_OK)
    parsed = pp_media_root_id_format(parsed_root, &root_text, error);
  if (parsed == PP_OK && strcmp(root_text,
      "00000000-0000-0000-0000-000000000001")) parsed = PP_ERROR_INTERNAL;
  pp_string_release(root_text);
  if (parsed != PP_OK) return parsed;

  pp_error_code_t status =
      pp_production_begin_edit(production, NULL, NULL, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_add_media_root(transaction, "rushes", NULL, 0,
                                           &root_id, error);
  }
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit(transaction, &commit_receipt, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  if (status == PP_OK) {
    status = decision_base(production, &base, error);
  }

  if (status == PP_OK) {
    status = pp_production_begin_edit(production, &base, NULL,
                                                &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_set_media_root_enabled(transaction, root_id, 0,
                                                   error);
  }
  if (status == PP_OK) {
    pp_commit_receipt_t receipt;
    status = pp_transaction_commit(transaction, &receipt, error);
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
    status = pp_production_begin_edit(production, &base, NULL,
                                                &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_set_media_root_enabled(transaction, root_id, 1,
                                                   error);
  }
  if (status == PP_OK) {
    pp_error_t *conflict_error = NULL;
    pp_transaction_conflict_t conflict;
    /* Status-only commit deliberately discards any receipt; this call must fail. */
    status = pp_transaction_commit(transaction, &(pp_commit_receipt_t){0}, &conflict_error);
    if (status == PP_ERROR_CONFLICT &&
        pp_error_transaction_conflict(conflict_error, &conflict) != 0 &&
        conflict.kind == PP_CONFLICT_MEDIA_ROOT &&
        conflict.target.kind == 0 &&
        memcmp(conflict.media_root_id.bytes, root_id.bytes, sizeof(root_id.bytes)) ==
            0 &&
        memcmp(conflict.base_revision_id.bytes, base.revision_id.bytes,
               sizeof(base.revision_id.bytes)) == 0 &&
        conflict.base_revision_sequence == base.revision_sequence &&
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
  pp_decision_base_t refreshed = {0};
  if (status == PP_OK) {
    status = decision_base(production, &refreshed, error);
  }
  if (status == PP_OK) {
    status = pp_production_begin_edit(production, &refreshed, NULL,
                                                &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_set_media_root_enabled(transaction, root_id, 1,
                                                   error);
  }
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit(transaction, &commit_receipt, error);
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
