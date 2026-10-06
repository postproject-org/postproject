#include <postproject/postproject.h>

#include <string.h>

_Static_assert(_Generic((pp_production_id_t){0}, pp_uuid_t: 1, default: 0) == 0,
               "production IDs must be distinct from UUID interchange bytes");
_Static_assert(_Generic((pp_revision_id_t){0}, pp_transaction_id_t: 1, pp_uuid_t: 1, default: 0) == 0,
               "revision and transaction identities must be distinct");
_Static_assert(_Generic((pp_asset_id_t){0}, pp_uuid_t: 1, pp_production_id_t: 1, default: 0) == 0,
               "asset identities must be distinct from other kinds");
_Static_assert(_Generic((pp_media_root_id_t){0}, pp_asset_id_t: 1, pp_uuid_t: 1, default: 0) == 0,
               "media roots must have their own identity type");

_Static_assert(_Generic((pp_locator_id_t){0}, pp_media_root_id_t: 1, pp_uuid_t: 1, default: 0) == 0,
               "locators must have their own identity type");

_Static_assert(_Generic((pp_job_id_t){0}, pp_asset_id_t: 1, pp_locator_id_t: 1, pp_uuid_t: 1, default: 0) == 0,
               "jobs must have their own identity type");

_Static_assert(_Generic((pp_activity_id_t){0}, pp_job_id_t: 1, pp_asset_id_t: 1, pp_uuid_t: 1, default: 0) == 0,
               "activities must have their own identity type");

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
  pp_error_release(error); error = NULL;
  pp_object_ref_t target;
  if (pp_object_ref_from_asset(asset, &target, &error) != PP_OK ||
      target.kind != PP_OBJECT_ASSET || memcmp(target.id.bytes, asset.bytes, 16)) return 19;
  if (pp_object_ref_from_asset(asset, NULL, &error) != PP_ERROR_INVALID_ARGUMENT) return 20;
  pp_error_release(error); error = NULL;
  pp_asset_set_t *assets = (pp_asset_set_t *)&target;
  if (pp_production_asset(NULL, asset, &assets, &error) != PP_ERROR_INVALID_ARGUMENT ||
      assets != NULL) return 21;
  pp_error_release(error); error = NULL;
  target.kind = PP_OBJECT_RESOURCE;
  asset.bytes[0] = 1;
  if (pp_object_ref_get_asset(&target, &asset, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(asset.bytes, zero, 16)) return 22;
  pp_error_release(error); error = NULL;
  if (pp_object_ref_get_asset(NULL, &asset, &error) != PP_ERROR_INVALID_ARGUMENT) return 23;
  pp_error_release(error);
  error = NULL;
  pp_media_root_id_t root;
  if (pp_media_root_id_parse(canonical, &root, &error) != PP_OK ||
      pp_media_root_id_format(root, &text, &error) != PP_OK ||
      strcmp(text, canonical)) return 24;
  pp_string_release(text); text = NULL;
  if (pp_media_root_id_parse("broken", &root, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(root.bytes, zero, 16)) return 25;
  pp_error_release(error); error = NULL;
  if (pp_media_root_id_parse(NULL, &root, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(root.bytes, zero, 16)) return 26;
  pp_error_release(error); error = NULL;
  if (pp_media_root_id_format(root, &text, &error) != PP_OK ||
      strcmp(text, "00000000-0000-0000-0000-000000000000")) return 27;
  pp_string_release(text);
  if (pp_media_root_id_format(root, NULL, &error) != PP_ERROR_INVALID_ARGUMENT) return 28;
  pp_error_release(error); error = NULL;
  pp_locator_id_t locator;
  if (pp_locator_id_parse(canonical, &locator, &error) != PP_OK ||
      pp_locator_id_format(locator, &text, &error) != PP_OK ||
      strcmp(text, canonical)) return 29;
  pp_string_release(text); text = NULL;
  if (pp_locator_id_parse("broken", &locator, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(locator.bytes, zero, 16)) return 30;
  pp_error_release(error); error = NULL;
  if (pp_locator_id_parse(NULL, &locator, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(locator.bytes, zero, 16)) return 31;
  pp_error_release(error); error = NULL;
  if (pp_locator_id_format(locator, &text, &error) != PP_OK ||
      strcmp(text, "00000000-0000-0000-0000-000000000000")) return 32;
  pp_string_release(text);
  if (pp_locator_id_format(locator, NULL, &error) != PP_ERROR_INVALID_ARGUMENT) return 33;
  pp_error_release(error); error = NULL;
  pp_job_id_t job;
  if (pp_job_id_parse(canonical, &job, &error) != PP_OK ||
      pp_job_id_format(job, &text, &error) != PP_OK ||
      strcmp(text, canonical)) return 34;
  pp_string_release(text); text = NULL;
  if (pp_object_ref_from_job(job, &target, &error) != PP_OK ||
      target.kind != PP_OBJECT_JOB ||
      pp_object_ref_get_job(&target, &job, &error) != PP_OK) return 35;
  target.kind = PP_OBJECT_ASSET;
  if (pp_object_ref_get_job(&target, &job, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(job.bytes, zero, 16)) return 36;
  pp_error_release(error); error = NULL;
  if (pp_job_id_parse("broken", &job, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(job.bytes, zero, 16)) return 37;
  pp_error_release(error); error = NULL;
  if (pp_job_id_parse(NULL, &job, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(job.bytes, zero, 16)) return 38;
  pp_error_release(error); error = NULL;
  if (pp_job_id_format(job, &text, &error) != PP_OK ||
      strcmp(text, "00000000-0000-0000-0000-000000000000")) return 39;
  pp_string_release(text);
  if (pp_job_id_format(job, NULL, &error) != PP_ERROR_INVALID_ARGUMENT) return 40;
  pp_error_release(error);
  error = NULL;
  pp_activity_id_t activity;
  if (pp_activity_id_parse(canonical, &activity, &error) != PP_OK ||
      pp_activity_id_format(activity, &text, &error) != PP_OK ||
      strcmp(text, canonical)) return 41;
  pp_string_release(text); text = NULL;
  if (pp_object_ref_from_activity(activity, &target, &error) != PP_OK ||
      target.kind != PP_OBJECT_ACTIVITY ||
      pp_object_ref_get_activity(&target, &activity, &error) != PP_OK) return 42;
  target.kind = PP_OBJECT_ASSET;
  if (pp_object_ref_get_activity(&target, &activity, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(activity.bytes, zero, 16)) return 43;
  pp_error_release(error); error = NULL;
  if (pp_activity_id_parse("broken", &activity, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(activity.bytes, zero, 16)) return 44;
  pp_error_release(error); error = NULL;
  if (pp_activity_id_parse(NULL, &activity, &error) != PP_ERROR_INVALID_ARGUMENT ||
      memcmp(activity.bytes, zero, 16)) return 45;
  pp_error_release(error); error = NULL;
  if (pp_activity_id_format(activity, &text, &error) != PP_OK ||
      strcmp(text, "00000000-0000-0000-0000-000000000000")) return 46;
  pp_string_release(text);
  if (pp_activity_id_format(activity, NULL, &error) != PP_ERROR_INVALID_ARGUMENT) return 47;
  pp_error_release(error);
  return 0;
}
