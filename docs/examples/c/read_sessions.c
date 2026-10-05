/* Coherent reads, detached bases, explicit edits and atomic receipts. */
#include <postproject/postproject.h>
#include <stdio.h>
#include <string.h>

/* [coherent-reads] */
static int lookup(const pp_read_session_t *view, const pp_uuid_t *asset, const char *media) {
  pp_media_root_set_t *roots = NULL;
  pp_external_identifier_set_t *identifiers = NULL;
  pp_object_ref_set_t *objects = NULL;
  pp_known_media_set_t *matches = NULL;
  pp_fingerprint_t *fingerprint = NULL;
  pp_error_t *error = NULL;
  char *uri = NULL;
  const char *algorithm;
  const uint8_t *value;
  uint16_t version;
  uint64_t length;
  const pp_object_ref_t target = {PP_OBJECT_ASSET, *asset};
  int result = 1;
#define CHECK(call) do { if ((call) != PP_OK) goto cleanup; } while (0)
  CHECK(pp_read_session_media_roots(view, &roots, &error));
  if (pp_media_root_set_count(roots) != 0) goto cleanup;
  CHECK(pp_read_session_external_identifiers(view, &target, &identifiers, &error));
  if (pp_external_identifier_set_count(identifiers) != 1) goto cleanup;
  CHECK(pp_read_session_find_by_external_identifier(view, "https://example.com/id", "camera", NULL, &objects, &error));
  if (pp_object_ref_set_count(objects) != 1) goto cleanup;
  CHECK(pp_file_path_to_locator(media, &uri, &error));
  CHECK(pp_read_session_find_known_media_by_locator(view, uri, NULL, 10, NULL, &matches, &error));
  if (pp_known_media_set_count(matches) != 1) goto cleanup;
  pp_known_media_set_release(matches); matches = NULL;
  CHECK(pp_fingerprint_file(media, &fingerprint, &error));
  CHECK(pp_fingerprint_get(fingerprint, &algorithm, &version, &value, &length, &error));
  CHECK(pp_read_session_find_known_media_by_fingerprint(view, algorithm, version, value, length, 10, NULL, &matches, &error));
  if (pp_known_media_set_count(matches) != 1) goto cleanup;
  result = 0;
cleanup:
  pp_error_release(error);
  pp_media_root_set_release(roots);
  pp_external_identifier_set_release(identifiers);
  pp_object_ref_set_release(objects);
  pp_known_media_set_release(matches);
  pp_fingerprint_release(fingerprint);
  pp_string_release(uri);
  return result;
#undef CHECK
}

static int exercise(const char *path, const char *media) {
  pp_production_t *production = NULL;
  pp_read_session_t *empty = NULL, *view = NULL;
  pp_transaction_t *edit = NULL;
  pp_media_source_t *source = NULL;
  pp_asset_set_t *assets = NULL;
  pp_representation_set_t *representations = NULL;
  pp_object_query_set_t *resource_page = NULL;
  pp_locator_query_set_t *locator_page = NULL;
  pp_error_t *error = NULL;
  pp_uuid_t asset, representation;
  pp_decision_base_t base;
  pp_commit_receipt_t receipt;
  int result = 1;
#define CHECK(call) do { if ((call) != PP_OK) goto cleanup; } while (0)
  CHECK(pp_production_create(path, "Read views", &production, &error));
  CHECK(pp_production_read_session(production, &empty, &error));
  CHECK(pp_read_session_decision_base(empty, &base, &error));
  if (base.has_revision || base.revision_sequence != 0) goto cleanup;
  CHECK(pp_read_session_begin_edit(empty, &edit, &error));
  CHECK(pp_media_source_create_file(media, &source, &error));
  CHECK(pp_transaction_import_media(edit, source, NULL, &asset, &error));
  const pp_object_ref_t target = {PP_OBJECT_ASSET, asset};
  CHECK(pp_transaction_add_external_identifier(edit, &target, "https://example.com/id", "camera", NULL, &error));
  CHECK(pp_transaction_commit_with_receipt(edit, &receipt, &error));
  if (receipt.outcome != PP_COMMIT_REVISION_CREATED || receipt.revision_sequence != 1)
    goto cleanup;
  pp_transaction_release(edit); edit = NULL;
  CHECK(pp_read_session_assets_page(empty, 10, NULL, &assets, &error));
  if (pp_asset_set_count(assets) != 0) goto cleanup;
  pp_asset_set_release(assets); assets = NULL;
  pp_read_session_release(empty); empty = NULL;

  CHECK(pp_production_read_session(production, &view, &error));
  CHECK(pp_read_session_decision_base(view, &base, &error));
  CHECK(pp_read_session_asset(view, &asset, &assets, &error));
  if (pp_asset_set_count(assets) != 1) goto cleanup;
  CHECK(pp_read_session_representations_page(view, &asset, 10, NULL, &representations, &error));
  uint32_t kind, structure;
  pp_uuid_t owner;
  uint64_t members, resources, fingerprints;
  CHECK(pp_representation_set_get(representations, 0, &representation, &owner,
      &kind, &structure, &members, &resources, &fingerprints, &error));
  pp_representation_set_release(representations); representations = NULL;
  CHECK(pp_read_session_representation(view, &representation, &representations, &error));
  CHECK(pp_read_session_resources_page(view, &representation, 10, NULL, &resource_page, &error));
  if (pp_object_query_set_count(resource_page) != 1) goto cleanup;
  pp_object_ref_t resource;
  uint32_t depth;
  CHECK(pp_object_query_set_get(resource_page, 0, &resource, &depth, &error));
  if (resource.kind != PP_OBJECT_RESOURCE) goto cleanup;
  CHECK(pp_read_session_locators_page(view, &resource.id, 10, NULL, &locator_page, &error));
  if (pp_locator_query_set_count(locator_page) != 1) goto cleanup;
  pp_representation_set_release(representations); representations = NULL;
  CHECK(pp_read_session_representations_using_resource(view, &resource.id, 10, NULL, &representations, &error));
  if (pp_representation_set_count(representations) != 1) goto cleanup;
  if (lookup(view, &asset, media)) goto cleanup;
  pp_read_session_release(view); view = NULL;
  /* Copied sets and a detached base survive closing the pinned view. */
  CHECK(pp_production_begin_edit(production, &base, &edit, &error));
  CHECK(pp_transaction_commit_with_receipt(edit, &receipt, &error));
  if (receipt.outcome != PP_COMMIT_NO_CHANGE || pp_asset_set_count(assets) != 1 ||
      pp_locator_query_set_count(locator_page) != 1 ||
      pp_object_query_set_count(resource_page) != 1)
    goto cleanup;
  result = 0;
cleanup:
  if (result) fprintf(stderr, "%s\n", error ? pp_error_message(error) : "read-view assertion");
  pp_error_release(error);
  pp_asset_set_release(assets);
  pp_representation_set_release(representations);
  pp_object_query_set_release(resource_page);
  pp_locator_query_set_release(locator_page);
  pp_media_source_release(source);
  pp_transaction_release(edit);
  pp_read_session_release(empty);
  pp_read_session_release(view);
  pp_production_release(production);
  return result;
#undef CHECK
}
/* [/coherent-reads] */

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  char path[4096], media[4096];
  if (snprintf(path, sizeof(path), "%s/views.pproj", argv[1]) >= (int)sizeof(path) ||
      snprintf(media, sizeof(media), "%s/rushes/A001.mov", argv[1]) >= (int)sizeof(media))
    return 2;
  return exercise(path, media);
}
