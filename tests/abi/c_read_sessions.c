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

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
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
