#include <postproject/postproject.h>

#include <stdio.h>
#include <string.h>

/* Untrusted stack bases must fail without leaking a transaction guard. */
static int rejects_base(pp_production_t *production, pp_decision_base_t base) {
  pp_transaction_t *edit = NULL;
  pp_error_t *error = NULL;
  pp_error_code_t status = pp_production_begin_edit(production, &base, &edit, &error);
  int rejected = status == PP_ERROR_INVALID_ARGUMENT && edit == NULL;
  pp_error_release(error);
  pp_transaction_release(edit);
  return rejected;
}

static int metadata_edits_contract(pp_production_t *production) {
  pp_read_session_t *view = NULL;
  pp_transaction_t *edit = NULL;
  pp_metadata_input_t *value = NULL;
  pp_metadata_set_t *values = NULL;
  pp_error_t *error = NULL;
  pp_decision_base_t base;
  pp_commit_receipt_t receipt;
  pp_transaction_conflict_t conflict;
  pp_object_ref_t target = {PP_OBJECT_PRODUCTION, {{0}}};
  const char *vocabulary = "com.example.editor";
  int valid = 0;
  if (pp_production_read_session(production, &view, &error) != PP_OK ||
      pp_read_session_decision_base(view, &base, &error) != PP_OK ||
      pp_metadata_input_create_string("keyword", NULL, &value, &error) != PP_OK)
    goto cleanup;
  memcpy(target.id.bytes, base.production_id.bytes, sizeof target.id.bytes);
  if (pp_production_begin_transaction(production, &edit, &error) != PP_OK ||
      pp_transaction_add_metadata_value(edit, &target, vocabulary, "keywords",
                                       value, &error) != PP_OK ||
      pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_OK)
    goto cleanup;
  pp_transaction_release(edit); edit = NULL;
  if (pp_production_begin_transaction(production, &edit, &error) != PP_OK ||
      pp_transaction_remove_metadata_property(edit, &target, vocabulary,
          "keywords", &error) != PP_ERROR_INVALID_ARGUMENT || error == NULL)
    goto cleanup;
  pp_error_release(error); error = NULL;
  /* Validation rejection must not stage a removal or close the transaction. */
  if (pp_transaction_add_metadata_value(edit, &target, vocabulary, "keywords",
                                       value, &error) != PP_OK ||
      pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_OK)
    goto cleanup;
  pp_transaction_release(edit); edit = NULL;
  if (pp_read_session_begin_edit(view, &edit, &error) != PP_OK ||
      pp_transaction_remove_metadata_property(edit, &target, vocabulary,
          "keywords", &error) != PP_OK ||
      pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_ERROR_CONFLICT ||
      receipt.outcome != PP_COMMIT_NO_CHANGE ||
      pp_error_transaction_conflict(error, &conflict) != 1)
    goto cleanup;
  pp_error_release(error); error = NULL;
  pp_transaction_release(edit); edit = NULL;
  if (pp_production_metadata(production, &target, &values, &error) != PP_OK ||
      pp_metadata_set_count(values) != 2)
    goto cleanup;
  pp_metadata_set_release(values); values = NULL;
  pp_read_session_release(view); view = NULL;
  if (pp_production_read_session(production, &view, &error) != PP_OK ||
      pp_read_session_begin_edit(view, &edit, &error) != PP_OK ||
      pp_transaction_remove_metadata_property(edit, &target, vocabulary,
          "keywords", &error) != PP_OK ||
      pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_OK ||
      receipt.outcome != PP_COMMIT_REVISION_CREATED ||
      pp_production_metadata(production, &target, &values, &error) != PP_OK ||
      pp_metadata_set_count(values) != 0)
    goto cleanup;
  valid = 1;
cleanup:
  pp_error_release(error);
  pp_metadata_set_release(values);
  pp_metadata_input_release(value);
  pp_transaction_release(edit);
  pp_read_session_release(view);
  return valid;
}

static int root_edits_contract(pp_production_t *production) {
  pp_transaction_t *edit = NULL;
  pp_read_session_t *view = NULL;
  pp_error_t *error = NULL;
  pp_media_root_id_t root;
  pp_commit_receipt_t receipt;
  pp_transaction_conflict_t conflict;
  int valid = 0;
  if (pp_production_begin_transaction(production, &edit, &error) != PP_OK ||
      pp_transaction_add_media_root(edit, "protected", NULL, 0, &root, &error) != PP_OK ||
      pp_transaction_set_media_root_enabled(edit, root, 0, &error) != PP_ERROR_INVALID_ARGUMENT ||
      error == NULL) goto cleanup;
  pp_error_release(error); error = NULL;
  if (pp_transaction_remove_media_root(edit, root, &error) != PP_ERROR_INVALID_ARGUMENT ||
      error == NULL) goto cleanup;
  pp_error_release(error); error = NULL;
  /* Neither rejection staged a mutation or discarded the additive creation. */
  if (pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_OK ||
      receipt.outcome != PP_COMMIT_REVISION_CREATED) goto cleanup;
  pp_transaction_release(edit); edit = NULL;
  if (pp_production_read_session(production, &view, &error) != PP_OK ||
      pp_read_session_begin_edit(view, &edit, &error) != PP_OK)
    goto cleanup;
  if (pp_transaction_set_media_root_enabled(edit, root, 2, &error) != PP_ERROR_INVALID_ARGUMENT ||
      error == NULL) goto cleanup;
  pp_error_release(error); error = NULL;
  if (pp_transaction_set_media_root_enabled(edit, root, 0, &error) != PP_OK ||
      pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_OK ||
      receipt.outcome != PP_COMMIT_REVISION_CREATED) goto cleanup;
  pp_revision_id_t superseding = receipt.revision_id;
  pp_transaction_release(edit); edit = NULL;
  if (pp_read_session_begin_edit(view, &edit, &error) != PP_OK ||
      pp_transaction_remove_media_root(edit, root, &error) != PP_OK ||
      pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_ERROR_CONFLICT ||
      receipt.outcome != PP_COMMIT_NO_CHANGE ||
      pp_error_transaction_conflict(error, &conflict) != 1 ||
      conflict.kind != PP_CONFLICT_MEDIA_ROOT || conflict.target.kind != 0 ||
      memcmp(conflict.media_root_id.bytes, root.bytes, 16) ||
      memcmp(conflict.superseding_revision_id.bytes, superseding.bytes, 16)) goto cleanup;
  pp_error_release(error); error = NULL;
  pp_transaction_release(edit); edit = NULL;
  pp_read_session_release(view); view = NULL;
  /* Fresh removal succeeds only if the stale removal rolled back. */
  if (pp_production_read_session(production, &view, &error) != PP_OK ||
      pp_read_session_begin_edit(view, &edit, &error) != PP_OK ||
      pp_transaction_remove_media_root(edit, root, &error) != PP_OK ||
      pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_OK ||
      receipt.outcome != PP_COMMIT_REVISION_CREATED) goto cleanup;
  valid = 1;
cleanup:
  pp_error_release(error);
  pp_transaction_release(edit);
  pp_read_session_release(view);
  return valid;
}

static int rejects_null_storage_reads(void) {
  pp_object_query_set_t *resources = (pp_object_query_set_t *)(uintptr_t)1;
  pp_locator_query_set_t *locators = (pp_locator_query_set_t *)(uintptr_t)1;
  pp_representation_set_t *representations = (pp_representation_set_t *)(uintptr_t)1;
  pp_error_t *error = NULL;
  pp_error_code_t status = pp_read_session_resources_page(
      NULL, (pp_representation_id_t){{0}}, 1, NULL, &resources, &error);
  int valid = status == PP_ERROR_INVALID_ARGUMENT && resources == NULL;
  pp_error_release(error); error = NULL;
  status = pp_read_session_locators_page(NULL, NULL, 1, NULL, &locators, &error);
  valid = valid && status == PP_ERROR_INVALID_ARGUMENT && locators == NULL;
  pp_error_release(error); error = NULL;
  status = pp_read_session_representations_using_resource(
      NULL, NULL, 1, NULL, &representations, &error);
  valid = valid && status == PP_ERROR_INVALID_ARGUMENT && representations == NULL;
  pp_error_release(error); error = NULL;
  pp_dependency_set_t *dependencies = (pp_dependency_set_t *)(uintptr_t)1;
  status = pp_read_session_dependency_set(NULL, (pp_representation_id_t){{0}}, &dependencies, &error);
  valid = valid && status == PP_ERROR_INVALID_ARGUMENT && dependencies == NULL;
  pp_error_release(error); error = NULL;
  pp_dependency_query_set_t *matches = (pp_dependency_query_set_t *)(uintptr_t)1;
  status = pp_read_session_dependencies(NULL, (pp_representation_id_t){{0}}, 64, 1000, 1, NULL, &matches, &error);
  valid = valid && status == PP_ERROR_INVALID_ARGUMENT && matches == NULL;
  pp_error_release(error); error = NULL;
  matches = (pp_dependency_query_set_t *)(uintptr_t)1;
  status = pp_read_session_dependents(NULL, NULL, 64, 1000, 1, NULL, &matches, &error);
  valid = valid && status == PP_ERROR_INVALID_ARGUMENT && matches == NULL;
  pp_error_release(error); error = NULL;
  pp_revision_set_t *revisions = (pp_revision_set_t *)(uintptr_t)1;
  uint64_t through = 42;
  status = pp_read_session_changes_since_filtered(
      NULL, 0, NULL, 0, 1, &revisions, &through, &error);
  valid = valid && status == PP_ERROR_INVALID_ARGUMENT && revisions == NULL && through == 0;
  pp_error_release(error); error = NULL;
  pp_revision_event_set_t *events = (pp_revision_event_set_t *)(uintptr_t)1;
  status = pp_read_session_revision_events_page(NULL, (pp_revision_id_t){{0}}, 1, NULL, &events, &error);
  valid = valid && status == PP_ERROR_INVALID_ARGUMENT && events == NULL;
  pp_error_release(error); error = NULL;
  pp_regeneration_plan_set_t *plans = (pp_regeneration_plan_set_t *)(uintptr_t)1;
  status = pp_read_session_plan_regeneration(NULL, NULL, 0, &plans, &error);
  valid = valid && status == PP_ERROR_INVALID_ARGUMENT && plans == NULL;
  pp_error_release(error);
  return valid;
}

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
  if (!rejects_null_storage_reads())
    return 1;
  remove(argv[1]);
  pp_production_t *production = NULL;
  pp_read_session_t *view = NULL;
  pp_transaction_t *edit = NULL;
  pp_asset_set_t *assets = NULL;
  pp_error_t *error = NULL;
  pp_decision_base_t base, invalid;
  pp_commit_receipt_t receipt;
  int result = 1;
  if (pp_production_create(argv[1], NULL, &production, &error) != PP_OK ||
      pp_production_read_session(production, &view, &error) != PP_OK ||
      pp_read_session_decision_base(view, &base, &error) != PP_OK)
    goto cleanup;
  if (base.has_revision != 0 || base.revision_sequence != 0)
    goto cleanup;
  pp_regeneration_plan_set_t *plans = (pp_regeneration_plan_set_t *)(uintptr_t)1;
  /* The count cap is checked before an array is read or allocated. */
  if (pp_read_session_plan_regeneration(view, NULL, PP_MAX_REGENERATION_PLANS + 1,
                                      &plans, &error) != PP_ERROR_INVALID_ARGUMENT || plans)
    goto cleanup;
  pp_error_release(error); error = NULL;
  invalid = base;
  invalid.has_revision = 2;
  if (!rejects_base(production, invalid))
    goto cleanup;
  invalid = base;
  invalid.revision_id.bytes[0] = 1;
  if (!rejects_base(production, invalid))
    goto cleanup;
  invalid = base;
  invalid.revision_sequence = 1;
  if (!rejects_base(production, invalid))
    goto cleanup;
  invalid = base;
  invalid.production_id.bytes[0] ^= 1;
  if (!rejects_base(production, invalid))
    goto cleanup;
  if (pp_production_begin_edit(production, &base, &edit, &error) != PP_OK ||
      pp_transaction_rollback(edit, &error) != PP_OK)
    goto cleanup;
  pp_transaction_release(edit);
  edit = NULL;

  if (!metadata_edits_contract(production))
    goto cleanup;
  if (!root_edits_contract(production))
    goto cleanup;

  /* The session retains a production for edits and owns its read connection. */
  pp_production_release(production);
  production = NULL;
  if (pp_read_session_assets_page(view, 1, NULL, &assets, &error) != PP_OK ||
      pp_asset_set_count(assets) != 0 ||
      pp_read_session_begin_edit(view, &edit, &error) != PP_OK ||
      pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_OK)
    goto cleanup;
  if (receipt.outcome != PP_COMMIT_NO_CHANGE ||
      memcmp(receipt.production_id.bytes, base.production_id.bytes, 16) != 0)
    goto cleanup;
  pp_read_session_release(view);
  view = NULL;
  /* Copies and their owning sets remain valid after the view closes. */
  if (pp_asset_set_count(assets) != 0)
    goto cleanup;
  result = 0;
cleanup:
  if (result)
    fprintf(stderr, "read-session boundary: %s\n",
            error ? pp_error_message(error) : "unexpected scope or lifetime result");
  pp_error_release(error);
  pp_transaction_release(edit);
  pp_asset_set_release(assets);
  pp_read_session_release(view);
  pp_production_release(production);
  return result;
}
