#include <postproject/postproject.h>

#include <string.h>

_Static_assert(_Generic((pp_production_id_t){0}, pp_uuid_t: 1, default: 0) == 0,
               "production IDs must be distinct from UUID interchange bytes");
_Static_assert(_Generic((pp_revision_id_t){0}, pp_transaction_id_t: 1, pp_uuid_t: 1, default: 0) == 0,
               "revision and transaction identities must be distinct");
_Static_assert(_Generic((pp_asset_id_t){0}, pp_uuid_t: 1, pp_production_id_t: 1, default: 0) == 0,
               "asset identities must be distinct from other kinds");

int main(void) {
  pp_production_id_t id = {{0}};
  pp_error_t *error = NULL;
  char *text = NULL;
  const char *canonical = "12345678-1234-5678-9abc-123456789abc";
  if (pp_production_id_parse(canonical, &id, &error) != PP_OK ||
      pp_production_id_format(id, &text, &error) != PP_OK ||
      strcmp(text, canonical) != 0)
    return 1;
  pp_string_release(text);
  if (pp_production_id_parse("broken", &id, &error) != PP_ERROR_INVALID_ARGUMENT)
    return 2;
  pp_error_release(error); error = NULL;
  const unsigned char zero[16] = {0};
  if (memcmp(id.bytes, zero, sizeof zero) != 0)
    return 3;
  if (pp_production_id_parse("00000000-0000-0000-0000-000000000000", &id,
                             &error) != PP_OK)
    return 4;
  if (pp_production_id_format(id, NULL, &error) != PP_ERROR_INVALID_ARGUMENT)
    return 5;
  pp_error_release(error); error = NULL;
  if (pp_production_id_parse(NULL, &id, &error) != PP_ERROR_INVALID_ARGUMENT)
    return 6;
  pp_error_release(error); error = NULL;
  if (pp_production_id_format(id, &text, &error) != PP_OK ||
      strcmp(text, "00000000-0000-0000-0000-000000000000") != 0)
    return 7;
  pp_string_release(text);
  pp_revision_id_t revision;
  pp_transaction_id_t transaction;
  if (pp_revision_id_parse(canonical, &revision, &error) != PP_OK ||
      pp_transaction_id_parse(canonical, &transaction, &error) != PP_OK)
    return 8;
  if (pp_revision_id_parse(NULL, &revision, &error) != PP_ERROR_INVALID_ARGUMENT)
    return 9;
  pp_error_release(error); error = NULL;
  if (memcmp(revision.bytes, zero, sizeof zero)) return 10;
  if (pp_transaction_id_parse("broken", &transaction, &error) != PP_ERROR_INVALID_ARGUMENT)
    return 11;
  pp_error_release(error); error = NULL;
  if (memcmp(transaction.bytes, zero, sizeof zero)) return 12;
  if (pp_revision_id_format(revision, &text, &error) != PP_OK ||
      strcmp(text, "00000000-0000-0000-0000-000000000000")) return 13;
  pp_string_release(text); text = NULL;
  if (pp_transaction_id_format(transaction, NULL, &error) != PP_ERROR_INVALID_ARGUMENT)
    return 14;
  pp_error_release(error); error = NULL;
  pp_asset_id_t asset;
  if (pp_asset_id_parse(canonical, &asset, &error) != PP_OK ||
      pp_asset_id_format(asset, &text, &error) != PP_OK ||
      strcmp(text, canonical)) return 15;
  pp_string_release(text); text = NULL;
  if (pp_asset_id_parse("broken", &asset, &error) != PP_ERROR_INVALID_ARGUMENT)
    return 16;
  pp_error_release(error); error = NULL;
  if (memcmp(asset.bytes, zero, sizeof zero)) return 17;
  if (pp_asset_id_format(asset, NULL, &error) != PP_ERROR_INVALID_ARGUMENT) return 18;
  pp_error_release(error);
  return 0;
}
