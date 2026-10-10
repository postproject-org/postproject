/* Prepare portable intent through typed native staging. */
#include <postproject/postproject.h>
#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
  char path[4096];
  if (argc != 2 || snprintf(path, sizeof path, "%s/authority.pproj", argv[1]) >= (int)sizeof path) return 2;
  pp_production_t *production = NULL;
  pp_read_session_t *view = NULL;
  pp_transaction_t *edit = NULL, *next = NULL;
  pp_proposal_t *proposal = NULL;
  pp_metadata_input_t *value = NULL;
  pp_media_root_set_t *roots = NULL;
  pp_error_t *error = NULL;
  int result = 1;
#define CHECK(call) do { if ((call) != PP_OK) goto cleanup; } while (0)
  CHECK(pp_production_create(path, "Authority", &production, &error));
  /* [exchange-prepare] */
  pp_decision_base_t base;
  pp_object_ref_t target;
  pp_media_root_id_t root;
  const uint8_t *bytes;
  uint64_t length;
  /* Application-generated identities are retained for this complete intent. */
  pp_client_id_t client = {{1}};
  pp_request_id_t request = {{2}};
  CHECK(pp_production_read_session(production, &view, &error));
  CHECK(pp_read_session_decision_base(view, &base, &error));
  CHECK(pp_production_begin_edit(production, &base, &edit, &error));
  CHECK(pp_object_ref_from_production(base.production_id, &target, &error));
  CHECK(pp_metadata_input_create_u64(UINT64_MAX, &value, &error));
  CHECK(pp_transaction_add_metadata_value(edit, &target, "example:Exact", "count", value, &error));
  CHECK(pp_transaction_add_media_root(edit, "rushes", NULL, -5, &root, &error));
  CHECK(pp_transaction_prepare_proposal(edit, client, request, NULL, &proposal, &error));
  CHECK(pp_proposal_document(proposal, &bytes, &length, &error));
  if (length == 0 || bytes[0] != '{') goto cleanup;
  /* bytes are borrowed public transport, with exact integers and prepared IDs. */
  pp_proposal_release(proposal); proposal = NULL;
  /* [/exchange-prepare] */
  /* Preparation writes no domain state, and closes the original edit. */
  CHECK(pp_production_media_roots(production, &roots, &error));
  if (pp_media_root_set_count(roots) != 0) goto cleanup;
  CHECK(pp_production_begin_edit(production, &base, &next, &error));
  pp_transaction_release(edit); edit = NULL;
  /* Releasing the old closed handle must not discard the newer edit guard. */
  pp_transaction_t *blocked = NULL;
  if (pp_production_begin_edit(production, &base, &blocked, NULL) != PP_ERROR_CONFLICT || blocked != NULL) goto cleanup;
  result = 0;
cleanup:
  if (error) fprintf(stderr, "%s\n", pp_error_message(error));
  pp_error_release(error);
  pp_media_root_set_release(roots);
  pp_metadata_input_release(value);
  pp_proposal_release(proposal);
  pp_transaction_release(edit);
  pp_transaction_release(next);
  pp_read_session_release(view);
  pp_production_release(production);
  return result;
#undef CHECK
}
