/* Prepare portable intent through typed native staging. */
#include <postproject/postproject.h>
#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
  char path[4096], checkpoint[4096], mirror_path[4096], changes[4096];
  if (argc != 2 || snprintf(path, sizeof path, "%s/authority.pproj", argv[1]) >= (int)sizeof path) return 2;
  if (snprintf(checkpoint, sizeof checkpoint, "%s/checkpoint.ppxc", argv[1]) >= (int)sizeof checkpoint || snprintf(mirror_path, sizeof mirror_path, "%s/mirror.pproj", argv[1]) >= (int)sizeof mirror_path) return 2;
  if (snprintf(changes, sizeof changes, "%s/changes.ppxd", argv[1]) >= (int)sizeof changes) return 2;
  pp_production_t *production = NULL;
  pp_production_t *mirror = NULL;
  pp_read_session_t *view = NULL;
  pp_transaction_t *edit = NULL, *next = NULL;
  pp_proposal_t *proposal = NULL, *transported = NULL;
  pp_submission_t *submission = NULL, *duplicate = NULL;
  pp_outcome_t *outcome = NULL, *recovered = NULL;
  pp_metadata_input_t *value = NULL;
  pp_metadata_set_t *metadata = NULL;
  pp_media_root_set_t *roots = NULL;
  pp_error_t *error = NULL, *rejection = NULL;
  int result = 1;
#define CHECK(call) do { if ((call) != PP_OK) goto cleanup; } while (0)
  CHECK(pp_production_create(path, "Authority", &production, &error));
  /* [exchange-checkpoint] */
  pp_exchange_position_t saved, validated;
  pp_exchange_state_t mirror_state;
  CHECK(pp_production_export_checkpoint(production, checkpoint, &saved, &error));
  CHECK(pp_checkpoint_validate_file(checkpoint, NULL, &validated, &error));
  if (memcmp(saved.digest, validated.digest, sizeof saved.digest)) goto cleanup;
  CHECK(pp_checkpoint_import_file(checkpoint, mirror_path, NULL, &mirror, &error));
  CHECK(pp_production_exchange_state(mirror, &mirror_state, &error));
  if (mirror_state.role != PP_EXCHANGE_PASSIVE_MIRROR || !mirror_state.has_head || memcmp(mirror_state.head.digest, saved.digest, sizeof saved.digest)) goto cleanup;
  /* [/exchange-checkpoint] */
  pp_transaction_t *forbidden = NULL;
  if (pp_production_begin_edit(mirror, NULL, NULL, &forbidden, NULL) != PP_ERROR_UNSUPPORTED || forbidden != NULL) goto cleanup;
  if (pp_production_begin_edit(mirror, &mirror_state.observed_base.decision, NULL, &forbidden, NULL) != PP_ERROR_UNSUPPORTED || forbidden != NULL) goto cleanup;
  pp_exchange_limits_t tiny = mirror_state.limits;
  tiny.file_bytes = 1;
  if (pp_checkpoint_validate_file(checkpoint, &tiny, &validated, NULL) != PP_ERROR_LIMIT_EXCEEDED || validated.base.decision.revision_sequence != 0) goto cleanup;
  if (pp_production_export_checkpoint(production, checkpoint, &validated, NULL) != PP_ERROR_ALREADY_EXISTS) goto cleanup;
  /* [exchange-prepare] */
  pp_decision_base_t base;
  pp_exchange_state_t state;
  pp_object_ref_t target;
  pp_media_root_id_t root;
  const uint8_t *bytes;
  uint64_t length;
  /* Application-generated identities are retained for this complete intent. */
  pp_client_id_t client = {{1}};
  pp_request_id_t request = {{2}};
  CHECK(pp_production_exchange_state(production, &state, &error));
  if (state.protocol_version != 1 || state.role != PP_EXCHANGE_AUTHORITY || !state.has_head || state.floor.base.decision.revision_sequence != 0) goto cleanup;
  CHECK(pp_production_read_session(production, &view, &error));
  CHECK(pp_read_session_decision_base(view, &base, &error));
  CHECK(pp_production_begin_edit(production, &base, NULL, &edit, &error));
  target.kind = PP_OBJECT_PRODUCTION;
  memcpy(target.id.bytes, base.production_id.bytes, sizeof target.id.bytes);
  CHECK(pp_metadata_input_create_u64(UINT64_MAX, &value, &error));
  CHECK(pp_transaction_add_metadata_value(edit, &target, "example:Exact", "count", value, &error));
  CHECK(pp_transaction_add_media_root(edit, "rushes", NULL, -5, &root, &error));
  CHECK(pp_transaction_prepare_proposal(edit, client, request, &state.observed_base, &proposal, &error));
  CHECK(pp_proposal_document(proposal, &bytes, &length, &error));
  if (length == 0 || bytes[0] != '{') goto cleanup;
  /* bytes are borrowed public transport, with exact integers and prepared IDs. */
  /* [/exchange-prepare] */
  /* Preparation writes no domain state, and closes the original edit. */
  CHECK(pp_production_media_roots(production, &roots, &error));
  if (pp_media_root_set_count(roots) != 0) goto cleanup;
  CHECK(pp_production_begin_edit(production, &base, NULL, &next, &error));
  pp_transaction_release(edit); edit = NULL;
  /* Releasing the old closed handle must not discard the newer edit guard. */
  pp_transaction_t *blocked = NULL;
  if (pp_production_begin_edit(production, &base, NULL, &blocked, NULL) != PP_ERROR_CONFLICT || blocked != NULL) goto cleanup;
  pp_transaction_release(next); next = NULL;
  /* [exchange-submit] */
  pp_request_identity_t identity;
  pp_commit_receipt_t receipt;
  uint32_t status;
  CHECK(pp_proposal_identity(proposal, &identity, &error));
  CHECK(pp_proposal_parse(bytes, length, &transported, &error));
  CHECK(pp_production_submit_proposal(production, transported, NULL, 0, NULL, 0, &submission, &error));
  CHECK(pp_submission_outcome(submission, &outcome, &error));
  if (pp_submission_lease_count(submission) != 0) goto cleanup;
  pp_job_lease_t *absent = NULL;
  if (pp_submission_take_lease(submission, 0, &absent, NULL) != PP_ERROR_INVALID_ARGUMENT || absent != NULL) goto cleanup;
  CHECK(pp_outcome_status(outcome, &status, &error));
  if (status != PP_OUTCOME_ACCEPTED) goto cleanup;
  CHECK(pp_outcome_receipt(outcome, &receipt, &error));
  if (receipt.outcome != PP_COMMIT_REVISION_CREATED || receipt.revision_sequence != 1) goto cleanup;
  CHECK(pp_outcome_rejection(outcome, &rejection, &error));
  if (rejection != NULL) goto cleanup;
  pp_submission_release(submission); submission = NULL;
  /* Simulate response loss: reopen and recover the original scoped request. */
  pp_production_release(production); production = NULL;
  CHECK(pp_production_open(path, &production, &error));
  CHECK(pp_production_submission_outcome(production, &identity, &recovered, &error));
  const uint8_t *original_bytes, *recovered_bytes;
  uint64_t original_length, recovered_length;
  CHECK(pp_outcome_document(outcome, &original_bytes, &original_length, &error));
  CHECK(pp_outcome_document(recovered, &recovered_bytes, &recovered_length, &error));
  if (original_length != recovered_length || memcmp(original_bytes, recovered_bytes, (size_t)original_length)) goto cleanup;
  CHECK(pp_production_submit_proposal(production, proposal, NULL, 0, NULL, 0, &duplicate, &error));
  if (pp_submission_lease_count(duplicate) != 0) goto cleanup;
  pp_outcome_release(outcome); outcome = NULL;
  CHECK(pp_submission_outcome(duplicate, &outcome, &error));
  CHECK(pp_outcome_receipt(outcome, &receipt, &error));
  if (receipt.revision_sequence != 1) goto cleanup;
  pp_outcome_release(recovered); recovered = NULL;
  /* [/exchange-submit] */
  if (pp_outcome_job_count(outcome) != 0) goto cleanup;
  pp_job_id_t observed_job = {{1}};
  uint32_t observed_state = 99;
  int64_t observed_expiry = 99;
  pp_job_completion_t observed_completion;
  memset(&observed_completion, 1, sizeof observed_completion);
  if (pp_outcome_job_get(outcome, 0, &observed_job, &observed_state, NULL) != PP_ERROR_INVALID_ARGUMENT || observed_state != 0 || observed_job.bytes[0] != 0) goto cleanup;
  if (pp_outcome_job_expiry(outcome, 0, &observed_expiry, NULL) != PP_ERROR_INVALID_ARGUMENT || observed_expiry != 0) goto cleanup;
  if (pp_outcome_job_completion(outcome, 0, &observed_completion, NULL) != PP_ERROR_INVALID_ARGUMENT || observed_completion.activity_id.bytes[0] != 0) goto cleanup;
  /* [exchange-catch-up] */
  pp_exchange_position_t through, applied;
  CHECK(pp_production_export_changes(production, &saved, changes, &through, &error));
  CHECK(pp_production_apply_changes(mirror, changes, NULL, &applied, &error));
  if (applied.base.decision.revision_sequence != 1 || memcmp(through.digest, applied.digest, sizeof applied.digest)) goto cleanup;
  pp_production_release(mirror); mirror = NULL;
  CHECK(pp_production_open(mirror_path, &mirror, &error));
  CHECK(pp_production_apply_changes(mirror, changes, NULL, &applied, &error));
  CHECK(pp_production_exchange_state(mirror, &mirror_state, &error));
  if (mirror_state.head.base.decision.revision_sequence != 1 || memcmp(mirror_state.head.digest, through.digest, sizeof through.digest)) goto cleanup;
  /* Ordinary read APIs see the authority's exact copied knowledge. */
  pp_media_root_set_release(roots); roots = NULL;
  CHECK(pp_production_media_roots(mirror, &roots, &error));
  if (pp_media_root_set_count(roots) != 1) goto cleanup;
  CHECK(pp_production_metadata(mirror, &target, &metadata, &error));
  pp_object_ref_t copied_target;
  const pp_metadata_value_t *copied;
  const char *vocabulary, *property;
  uint64_t exact;
  CHECK(pp_metadata_set_get(metadata, 0, &copied_target, &vocabulary, &property, &copied, &error));
  CHECK(pp_metadata_value_get_u64(copied, &exact, &error));
  if (copied_target.kind != PP_OBJECT_PRODUCTION || memcmp(copied_target.id.bytes, target.id.bytes, sizeof target.id.bytes) || strcmp(vocabulary, "example:Exact") || strcmp(property, "count")) goto cleanup;
  pp_metadata_set_release(metadata); metadata = NULL;
  if (exact != UINT64_MAX) goto cleanup;
  /* [/exchange-catch-up] */
  if (pp_production_apply_changes(production, changes, NULL, &validated, NULL) != PP_ERROR_MIRROR_READ_ONLY) goto cleanup;
  CHECK(pp_production_exchange_state(production, &state, &error));
  CHECK(pp_production_resynchronize_history(production, &state.observed_base, 100, UINT64_C(1048576), &validated, &error));
  if (validated.base.decision.revision_sequence != 0) goto cleanup;
  if (pp_production_resynchronize_history(mirror, &mirror_state.observed_base, 100, UINT64_C(1048576), &validated, NULL) != PP_ERROR_MIRROR_READ_ONLY) goto cleanup;
  result = 0;
cleanup:
  if (error) fprintf(stderr, "%s\n", pp_error_message(error));
  pp_error_release(error);
  pp_error_release(rejection);
  pp_outcome_release(outcome);
  pp_outcome_release(recovered);
  pp_submission_release(submission);
  pp_submission_release(duplicate);
  pp_media_root_set_release(roots);
  pp_metadata_input_release(value);
  pp_metadata_set_release(metadata);
  pp_proposal_release(proposal);
  pp_proposal_release(transported);
  pp_transaction_release(edit);
  pp_transaction_release(next);
  pp_read_session_release(view);
  pp_production_release(production);
  pp_production_release(mirror);
  return result;
#undef CHECK
}
