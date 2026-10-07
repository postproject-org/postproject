#include <postproject/postproject.h>

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#ifdef _WIN32
#include <direct.h>
#endif

static int bytes_are_zero(const uint8_t *bytes) {
  static const uint8_t zero[16] = {0};
  return memcmp(bytes, zero, sizeof zero) == 0;
}

static int uuid_is_zero(const pp_uuid_t *id) {
  static const uint8_t zero[16] = {0};
  return memcmp(id->bytes, zero, sizeof(zero)) == 0;
}

static int representation_id_is_zero(const pp_representation_id_t *id) {
  static const uint8_t zero[16] = {0};
  return memcmp(id->bytes, zero, sizeof(zero)) == 0;
}

static int locator_id_is_zero(pp_locator_id_t id) {
  static const uint8_t zero[16] = {0};
  return memcmp(id.bytes, zero, sizeof(zero)) == 0;
}

static int job_id_is_zero(pp_job_id_t id) {
  static const uint8_t zero[16] = {0};
  return memcmp(id.bytes, zero, sizeof(zero)) == 0;
}

static int activity_id_is_zero(pp_activity_id_t id) {
  static const uint8_t zero[16] = {0};
  return memcmp(id.bytes, zero, sizeof(zero)) == 0;
}

static pp_error_code_t import_file(pp_transaction_t *transaction,
                                   const char *path, const char *display_name,
                                   pp_asset_id_t *out_asset_id,
                                   pp_error_t **out_error) {
  pp_media_source_t *source = NULL;
  pp_error_code_t status = pp_media_source_create_file(path, &source, out_error);
  if (status == PP_OK) {
    status = pp_transaction_import_media(transaction, source, display_name,
                                         out_asset_id, out_error);
  }
  pp_media_source_release(source);
  return status;
}

static pp_error_code_t add_file(pp_transaction_t *transaction,
                                const pp_asset_id_t *asset_id,
                                pp_representation_kind_t kind, const char *path,
                                pp_representation_id_t *out_representation_id,
                                pp_error_t **out_error) {
  pp_media_source_t *source = NULL;
  pp_error_code_t status = pp_media_source_create_file(path, &source, out_error);
  if (status == PP_OK) {
    status = pp_transaction_add_representation(
        transaction, *asset_id, kind, source, out_representation_id, out_error);
  }
  pp_media_source_release(source);
  return status;
}

static int make_directory(const char *path) {
#ifdef _WIN32
  return _mkdir(path) == 0 || errno == EEXIST ? 0 : -1;
#else
  return mkdir(path, 0777) == 0 || errno == EEXIST ? 0 : -1;
#endif
}

static int join_path(char *buffer, size_t size, const char *directory,
                     const char *name) {
  int length = snprintf(buffer, size, "%s/%s", directory, name);
  return length < 0 || (size_t)length >= size ? -1 : 0;
}

static int frame_path(char *buffer, size_t size, const char *directory,
                      const char *prefix, int frame) {
  int length =
      snprintf(buffer, size, "%s/%s%04d.png", directory, prefix, frame);
  return length < 0 || (size_t)length >= size ? -1 : 0;
}

static int write_frame(const char *directory, const char *prefix, int frame,
                       const char *content) {
  char path[4096];
  FILE *file = NULL;
  if (frame_path(path, sizeof(path), directory, prefix, frame) != 0 ||
      (file = fopen(path, "wb")) == NULL) {
    return -1;
  }
  size_t length = strlen(content);
  int written = fwrite(content, 1, length, file) == length;
  return fclose(file) == 0 && written ? 0 : -1;
}

static const char *const frame_contents[] = {"frame one", "frame two",
                                             "frame three"};

/* Resolves the asset searching directory and reads the first resource. */
static pp_error_code_t resolve_in(pp_production_t *production,
                                  const pp_asset_id_t *asset_id,
                                  const char *directory,
                                  pp_resolution_set_t **out_resolutions,
                                  pp_resource_resolution_state_t *out_state,
                                  uint64_t *out_candidates,
                                  pp_error_t **out_error) {
  pp_resolution_options_t *options = NULL;
  pp_error_code_t status = pp_resolution_options_create(&options, out_error);
  if (status == PP_OK) {
    status =
        pp_resolution_options_add_search_directory(options, directory, out_error);
  }
  if (status == PP_OK) {
    status = pp_production_resolve_assets(production, asset_id, 1, options,
                                          out_resolutions, out_error);
  }
  pp_resolution_options_release(options);
  if (status == PP_OK) {
    pp_resource_id_t resource_id = {{0}};
    uint64_t evidence_count = 0;
    status = pp_resolution_set_get_resource(*out_resolutions, 0, 0,
                                            &resource_id, out_state,
                                            out_candidates, &evidence_count,
                                            out_error);
  }
  return status;
}

/* A sequence imported as shot_0001.png to shot_0003.png and renamed into
 * another directory as shot-graded_0001.png ... is found by content, and
 * confirming it records a second locator beside the original naming. A group
 * with the same names but other content and an incomplete group are not
 * candidates; two identical renamed copies are ambiguous. */
static int renamed_sequence_scenario(pp_production_t *production,
                                     const char *work) {
  char base[4096];
  char plates[4096];
  char graded[4096];
  char other[4096];
  char partial[4096];
  char copy[4096];
  char path[4096];
  char target[4096];
  pp_error_t *error = NULL;
  if (join_path(base, sizeof(base), work, "c-renamed-sequence") != 0 ||
      join_path(plates, sizeof(plates), base, "plates") != 0 ||
      join_path(graded, sizeof(graded), base, "graded") != 0 ||
      join_path(other, sizeof(other), base, "other") != 0 ||
      join_path(partial, sizeof(partial), base, "partial") != 0 ||
      join_path(copy, sizeof(copy), base, "copy") != 0 ||
      make_directory(base) != 0 || make_directory(plates) != 0 ||
      make_directory(graded) != 0 || make_directory(other) != 0 ||
      make_directory(partial) != 0 || make_directory(copy) != 0) {
    return 130;
  }
  for (int frame = 1; frame <= 3; ++frame) {
    const char *directories[] = {plates, graded, other, partial, copy};
    for (size_t index = 0; index < 5; ++index) {
      if (frame_path(path, sizeof(path), directories[index], "shot-graded_",
                     frame) != 0) {
        return 130;
      }
      (void)remove(path);
    }
    if (write_frame(plates, "shot_", frame, frame_contents[frame - 1]) != 0 ||
        write_frame(other, "shot-graded_", frame, "other content") != 0) {
      return 130;
    }
  }
  if (write_frame(partial, "shot-graded_", 1, frame_contents[0]) != 0 ||
      write_frame(partial, "shot-graded_", 2, frame_contents[1]) != 0) {
    return 130;
  }

  const pp_sequence_naming_t naming = {"shot_", ".png", UINT8_C(4)};
  pp_media_source_t *source = NULL;
  pp_transaction_t *transaction = NULL;
  pp_asset_id_t asset_id = {{0}};
  pp_error_code_t status = pp_media_source_create_image_sequence(
      plates, &naming, INT64_C(1), INT64_C(3), UINT32_C(1), UINT32_C(24),
      UINT32_C(1), NULL, 0, &source, &error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, &error);
  }
  if (status == PP_OK) {
    status = pp_transaction_import_media(transaction, source, "Shot", &asset_id,
                                         &error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, &error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  pp_media_source_release(source);
  if (status != PP_OK) {
    pp_error_release(error);
    return 131;
  }
  for (int frame = 1; frame <= 3; ++frame) {
    if (frame_path(path, sizeof(path), plates, "shot_", frame) != 0 ||
        frame_path(target, sizeof(target), graded, "shot-graded_", frame) !=
            0 ||
        rename(path, target) != 0) {
      return 132;
    }
  }

  pp_resolution_set_t *resolutions = NULL;
  pp_resource_resolution_state_t state = 0;
  uint64_t candidate_count = 0;
  status = resolve_in(production, &asset_id, base, &resolutions, &state,
                      &candidate_count, &error);
  const char *uri = NULL;
  uint16_t confidence = 0;
  const char *media_root = NULL;
  uint8_t has_naming = 0;
  pp_sequence_naming_t found = {NULL, NULL, 0};
  uint64_t evidence_count = 0;
  if (status != PP_OK || state != PP_RESOURCE_RESOLVED_PROBABLE ||
      candidate_count != UINT64_C(1) ||
      pp_resolution_set_get_candidate(resolutions, 0, 0, 0, &uri, &confidence,
                                      &media_root, &has_naming, &found,
                                      &evidence_count, &error) != PP_OK ||
      has_naming != UINT8_C(1) || strcmp(found.prefix, "shot-graded_") != 0 ||
      strcmp(found.suffix, ".png") != 0 || found.padding != UINT8_C(4) ||
      strstr(uri, "graded") == NULL) {
    pp_resolution_set_release(resolutions);
    pp_error_release(error);
    return 133;
  }
  int partial_match = 0;
  for (uint64_t index = 0; index < evidence_count; ++index) {
    pp_evidence_kind_t kind = 0;
    const char *detail = NULL;
    if (pp_resolution_set_get_candidate_evidence(resolutions, 0, 0, 0, index,
                                                 &kind, &detail,
                                                 &error) != PP_OK) {
      pp_resolution_set_release(resolutions);
      pp_error_release(error);
      return 134;
    }
    partial_match |= kind == PP_EVIDENCE_PARTIAL_FINGERPRINT_MATCH;
  }
  if (!partial_match) {
    pp_resolution_set_release(resolutions);
    return 134;
  }

  // The candidate is confirmed with the naming it was found under.
  pp_resource_id_t resource_id = {{0}};
  pp_resource_resolution_state_t unused_state = 0;
  uint64_t unused_count = 0;
  status = pp_resolution_set_get_resource(resolutions, 0, 0, &resource_id,
                                          &unused_state, &unused_count,
                                          &unused_count, &error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, &error);
  }
  if (status == PP_OK) {
    status = pp_transaction_confirm_locator(transaction, resource_id, uri,
                                            media_root, &found, &error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, &error);
  }
  pp_transaction_release(transaction);
  pp_resolution_set_release(resolutions);
  if (status != PP_OK) {
    pp_error_release(error);
    return 135;
  }

  pp_locator_query_set_t *locators = NULL;
  int original = 0;
  int renamed = 0;
  status = pp_production_locators_page(production, resource_id, 10, NULL,
                                       &locators, &error);
  for (uint64_t index = 0;
       status == PP_OK && index < pp_locator_query_set_count(locators);
       ++index) {
    pp_locator_id_t locator_id = {{0}};
    pp_resource_id_t owner_id = {{0}};
    const char *locator_uri = NULL;
    pp_locator_availability_t availability = 0;
    uint8_t has_last_seen = 0;
    int64_t last_seen = 0;
    const char *root = NULL;
    uint8_t has_locator_naming = 0;
    pp_sequence_naming_t locator_naming = {NULL, NULL, 0};
    status = pp_locator_query_set_get(
        locators, index, &locator_id, &owner_id, &locator_uri, &availability,
        &has_last_seen, &last_seen, &root, &has_locator_naming,
        &locator_naming, &error);
    if (status == PP_OK && has_locator_naming == UINT8_C(1)) {
      original += strcmp(locator_naming.prefix, "shot_") == 0;
      renamed += strcmp(locator_naming.prefix, "shot-graded_") == 0;
    }
  }
  pp_locator_query_set_release(locators);
  if (status != PP_OK || original != 1 || renamed != 1) {
    pp_error_release(error);
    return 136;
  }

  // An identical renamed copy elsewhere makes the search ambiguous once the
  // confirmed directory is gone.
  for (int frame = 1; frame <= 3; ++frame) {
    if (write_frame(copy, "shot-graded_", frame, frame_contents[frame - 1]) !=
        0) {
      return 137;
    }
  }
  for (int frame = 1; frame <= 3; ++frame) {
    if (frame_path(path, sizeof(path), graded, "shot-graded_", frame) != 0 ||
        frame_path(target, sizeof(target), plates, "shot-graded_", frame) !=
            0) {
      return 137;
    }
    (void)remove(target);
    if (rename(path, target) != 0) {
      return 137;
    }
  }
  status = resolve_in(production, &asset_id, base, &resolutions, &state,
                      &candidate_count, &error);
  pp_resolution_set_release(resolutions);
  if (status != PP_OK || state != PP_RESOURCE_AMBIGUOUS ||
      candidate_count != UINT64_C(2)) {
    pp_error_release(error);
    return 138;
  }
  return 0;
}

static pp_error_code_t begin_decision_edit(pp_production_t *production,
                                      pp_transaction_t **edit, pp_error_t **error) {
  pp_read_session_t *view = NULL;
  pp_error_code_t status = pp_production_read_session(production, &view, error);
  if (status == PP_OK) status = pp_read_session_begin_edit(view, edit, error);
  pp_read_session_release(view);
  return status;
}

static int observation_base_contract(pp_production_t *production,
                                     pp_resource_id_t resource_id,
                                     pp_representation_id_t representation_id) {
  pp_transaction_t *edit = NULL;
  pp_error_t *error = NULL;
  pp_commit_receipt_t receipt;
  const uint8_t value[] = {1};
  int valid = 0;
  if (pp_production_begin_transaction(production, &edit, &error) != PP_OK)
    goto cleanup;
  for (int operation = 0; operation < 3; ++operation) {
    pp_content_observation_t outcome = UINT32_MAX;
    pp_error_code_t status;
    if (operation == 0) {
      status = pp_transaction_record_resource_fingerprint(
          edit, resource_id, "test", 1, value, sizeof value, &error);
    } else if (operation == 1) {
      status = pp_transaction_record_representation_fingerprint(
          edit, representation_id, "test", 1, value, sizeof value, &error);
    } else {
      status = pp_transaction_observe_resource_content(
          edit, resource_id, "absent-observation-file", NULL, &outcome, &error);
      if (outcome != 0) goto cleanup;
    }
    if (status != PP_ERROR_INVALID_ARGUMENT || error == NULL) goto cleanup;
    pp_error_release(error); error = NULL;
  }
  /* All three rejections leave an open edit with no staged observations. */
  if (pp_transaction_commit_with_receipt(edit, &receipt, &error) != PP_OK ||
      receipt.outcome != PP_COMMIT_NO_CHANGE) goto cleanup;
  valid = 1;
cleanup:
  pp_error_release(error);
  pp_transaction_release(edit);
  return valid;
}

int main(int argc, char **argv) {
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_error_t *error = NULL;
  pp_production_id_t id = {{0}};
  pp_asset_id_t asset_id = {{0}};
  pp_asset_id_t rolled_back_asset_id = {{0}};
  pp_media_root_id_t root_id = {{0}};
  pp_representation_id_t representation_id = {{0}};
  pp_resource_id_t resource_id = {{0}};
  pp_revision_id_t revision_id = {{0}};
  pp_transaction_id_t revision_transaction_id = {{0}};
  pp_revision_event_t revision_event = {0};
  char media_path[4096];
  char moved_media_path[4096];
  char sequence_frame_path[4096];

  if (argc != 3) {
    return 64;
  }
  (void)remove(argv[1]);
  if (pp_abi_version() != UINT32_C(47)) {
    return 1;
  }
  pp_error_code_t status =
      pp_production_create(argv[1], "C smoke test", &production, &error);
  if (status != PP_OK) {
    fprintf(stderr, "create failed (%u): %s\n", status,
            error != NULL ? pp_error_message(error) : "no details");
    pp_error_release(error);
    return 2;
  }
  if (pp_production_id(production, &id, &error) != PP_OK || memcmp(id.bytes, (const unsigned char[16]){0}, sizeof id.bytes) == 0) {
    pp_production_release(production);
    pp_error_release(error);
    return 3;
  }
  int media_path_length =
      snprintf(media_path, sizeof(media_path), "%s.media", argv[1]);
  if (media_path_length < 0 ||
      (size_t)media_path_length >= sizeof(media_path)) {
    pp_production_release(production);
    return 6;
  }
  FILE *media = fopen(media_path, "wb");
  if (media == NULL) {
    pp_production_release(production);
    return 7;
  }
  size_t written = fwrite("C ABI media", 1, 11, media);
  int close_status = fclose(media);
  if (written != 11 || close_status != 0) {
    pp_production_release(production);
    return 7;
  }
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK || transaction == NULL) {
    pp_production_release(production);
    pp_error_release(error);
    return 8;
  }
  status = import_file(transaction, media_path, NULL, &rolled_back_asset_id,
                       &error);
  if (status != PP_OK || bytes_are_zero(rolled_back_asset_id.bytes) ||
      pp_transaction_rollback(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 9;
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  uint8_t asset_exists = 1;
  status = pp_production_asset_exists(production, rolled_back_asset_id,
                                   &asset_exists, &error);
  if (status != PP_OK || asset_exists != UINT8_C(0)) {
    pp_production_release(production);
    pp_error_release(error);
    return 10;
  }

  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK || transaction == NULL) {
    pp_production_release(production);
    pp_error_release(error);
    return 11;
  }
  status = pp_transaction_set_revision_context(
      transaction, "C smoke", "1.0", NULL, "Import fixture", &error);
  if (status != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 42;
  }
  status = import_file(transaction, media_path, "C asset", &asset_id, &error);
  if (status != PP_OK || bytes_are_zero(asset_id.bytes)) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 12;
  }
  pp_object_ref_t asset_ref;
  pp_error_code_t reference_status = pp_object_ref_from_asset(asset_id, &asset_ref, &error);
  if (reference_status != PP_OK) return reference_status;
  char *host_binding = NULL;
  pp_production_id_t bound_production_id = {{0}};
  pp_object_ref_t bound_object = {0};
  status = pp_host_binding_format(id, &asset_ref, &host_binding, &error);
  if (status != PP_OK || host_binding == NULL ||
      strncmp(host_binding, "https://postproject.org/ref/v1/",
              sizeof("https://postproject.org/ref/v1/") - 1) != 0 ||
      pp_host_binding_parse(host_binding, &bound_production_id, &bound_object,
                            &error) != PP_OK ||
      memcmp(bound_production_id.bytes, id.bytes, sizeof(id.bytes)) != 0 ||
      bound_object.kind != PP_OBJECT_ASSET ||
      memcmp(bound_object.id.bytes, asset_id.bytes, sizeof(asset_id.bytes)) !=
          0) {
    pp_string_release(host_binding);
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 65;
  }
  pp_string_release(host_binding);
  status = pp_transaction_add_external_identifier(
      transaction, &asset_ref, "com.example.asset", "asset-42", "primary",
      &error);
  if (status != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 23;
  }
  pp_metadata_input_t *title_input = NULL;
  status = pp_metadata_input_create_string("C title", "en-US", &title_input,
                                           &error);
  if (status == PP_OK) {
    status = pp_transaction_add_metadata_value(
        transaction, &asset_ref, "com.example.metadata", "title", title_input,
        &error);
  }
  pp_metadata_input_release(title_input);
  if (status != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 26;
  }
  status = pp_transaction_add_media_root(transaction, "fixtures", "Fixture root",
                                         0, &root_id, &error);
  const unsigned char empty_root[16] = {0};
  if (status != PP_OK || memcmp(root_id.bytes, empty_root, 16) == 0) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 13;
  }
  status = pp_transaction_commit(transaction, &error);
  if (status != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 14;
  }
  pp_transaction_release(transaction);
  pp_production_release(production);
  production = NULL;

  status = pp_production_open(argv[1], &production, &error);
  if (status != PP_OK) {
    fprintf(stderr, "open failed (%u): %s\n", status,
            error != NULL ? pp_error_message(error) : "no details");
    pp_error_release(error);
    return 4;
  }
  pp_asset_set_t *assets = NULL;
  pp_asset_id_t read_asset_id = {{0}};
  int64_t asset_created_at = 0;
  const char *asset_display_name = NULL;
  const char *asset_import_source = NULL;
  status = pp_production_assets(production, &assets, &error);
  if (status != PP_OK || assets == NULL ||
      pp_asset_set_count(assets) != UINT64_C(1) ||
      pp_asset_set_get(assets, 0, &read_asset_id, &asset_created_at,
                       &asset_display_name, &asset_import_source, &error) !=
          PP_OK ||
      memcmp(read_asset_id.bytes, asset_id.bytes, sizeof(asset_id.bytes)) != 0 ||
      asset_created_at == 0 || asset_display_name == NULL ||
      strcmp(asset_display_name, "C asset") != 0 || asset_import_source != NULL) {
    pp_asset_set_release(assets);
    pp_production_release(production);
    pp_error_release(error);
    return 74;
  }
  pp_asset_set_release(assets);
  assets = NULL;
  status = pp_production_assets_page(production, UINT32_C(1), NULL, &assets,
                                     &error);
  if (status != PP_OK || assets == NULL ||
      pp_asset_set_count(assets) != UINT64_C(1) ||
      pp_asset_set_next_cursor(assets) != NULL) {
    pp_asset_set_release(assets);
    pp_production_release(production);
    pp_error_release(error);
    return 79;
  }
  pp_asset_set_release(assets);
  pp_media_root_set_t *roots = NULL;
  pp_media_root_id_t read_root_id = {{0}};
  const char *root_name = NULL;
  const char *root_label = NULL;
  const char *root_legacy_uri = NULL;
  int32_t root_priority = 0;
  uint8_t root_enabled = 0;
  status = pp_production_media_roots(production, &roots, &error);
  if (status != PP_OK || roots == NULL ||
      pp_media_root_set_count(roots) != UINT64_C(1) ||
      pp_media_root_set_get(roots, 0, &read_root_id, &root_name, &root_label,
                            &root_legacy_uri, &root_priority, &root_enabled,
                            &error) != PP_OK ||
      memcmp(read_root_id.bytes, root_id.bytes, sizeof(root_id.bytes)) != 0 ||
      root_name == NULL || strcmp(root_name, "fixtures") != 0 ||
      root_label == NULL || strcmp(root_label, "Fixture root") != 0 ||
      root_legacy_uri != NULL || root_priority != 0 ||
      root_enabled != UINT8_C(1)) {
    pp_media_root_set_release(roots);
    pp_production_release(production);
    pp_error_release(error);
    return 75;
  }
  pp_media_root_set_release(roots);
  pp_revision_set_t *revisions = NULL;
  uint64_t revision_sequence = 0;
  int64_t revision_committed_at = 0;
  const char *revision_origin_name = NULL;
  const char *revision_origin_version = NULL;
  const char *revision_origin_uri = NULL;
  const char *revision_message = NULL;
  status = pp_production_latest_revision(production, &revisions, &error);
  if (status != PP_OK || revisions == NULL ||
      pp_revision_set_count(revisions) != UINT64_C(1) ||
      pp_revision_set_get(
          revisions, 0, &revision_id, &revision_sequence,
          &revision_transaction_id, &revision_committed_at,
          &revision_origin_name, &revision_origin_version,
          &revision_origin_uri, &revision_message, &error) != PP_OK ||
      bytes_are_zero(revision_id.bytes) || bytes_are_zero(revision_transaction_id.bytes) ||
      revision_sequence != UINT64_C(1) || revision_committed_at == 0 ||
      revision_origin_name == NULL || revision_origin_version == NULL ||
      revision_message == NULL ||
      strcmp(revision_origin_name, "C smoke") != 0 ||
      strcmp(revision_origin_version, "1.0") != 0 ||
      revision_origin_uri != NULL ||
      strcmp(revision_message, "Import fixture") != 0) {
    pp_revision_set_release(revisions);
    pp_production_release(production);
    pp_error_release(error);
    return 36;
  }
  pp_revision_set_release(revisions);
  revisions = NULL;
  status = pp_production_changes_since(production, 0, 1, &revisions, &error);
  if (status != PP_OK || revisions == NULL ||
      pp_revision_set_count(revisions) != UINT64_C(1)) {
    pp_revision_set_release(revisions);
    pp_production_release(production);
    pp_error_release(error);
    return 37;
  }
  pp_revision_set_release(revisions);
  pp_revision_event_set_t *revision_events = NULL;
  status = pp_production_revision_events(production, revision_id, &revision_events,
                                      &error);
  if (status != PP_OK || revision_events == NULL ||
      pp_revision_event_set_count(revision_events) != UINT64_C(8) ||
      pp_revision_event_set_get(revision_events, 0, &revision_event, &error) !=
          PP_OK ||
      revision_event.kind != PP_REVISION_ASSET_IMPORTED ||
      revision_event.position != UINT32_C(0) ||
      memcmp(revision_event.asset_id.bytes, asset_id.bytes,
             sizeof(asset_id.bytes)) != 0) {
    pp_revision_event_set_release(revision_events);
    pp_production_release(production);
    pp_error_release(error);
    return 38;
  }
  status = pp_revision_event_set_get(revision_events, 5, &revision_event,
                                     &error);
  if (status != PP_OK ||
      revision_event.kind != PP_REVISION_EXTERNAL_IDENTIFIER_ADDED ||
      revision_event.target.kind != PP_OBJECT_ASSET ||
      memcmp(revision_event.target.id.bytes, asset_id.bytes,
             sizeof(asset_id.bytes)) != 0 ||
      strcmp(revision_event.identifier_scheme, "com.example.asset") != 0 ||
      strcmp(revision_event.identifier_value, "asset-42") != 0 ||
      strcmp(revision_event.identifier_qualifier, "primary") != 0) {
    pp_revision_event_set_release(revision_events);
    pp_production_release(production);
    pp_error_release(error);
    return 39;
  }
  status = pp_revision_event_set_get(revision_events, 6, &revision_event,
                                     &error);
  if (status != PP_OK ||
      revision_event.kind != PP_REVISION_METADATA_ADDED_OR_REPLACED ||
      revision_event.target.kind != PP_OBJECT_ASSET ||
      strcmp(revision_event.vocabulary, "com.example.metadata") != 0 ||
      strcmp(revision_event.property, "title") != 0) {
    pp_revision_event_set_release(revision_events);
    pp_production_release(production);
    pp_error_release(error);
    return 40;
  }
  status = pp_revision_event_set_get(revision_events, 7, &revision_event,
                                     &error);
  if (status != PP_OK ||
      revision_event.kind != PP_REVISION_MEDIA_ROOT_ADDED ||
      memcmp(revision_event.media_root_id.bytes, root_id.bytes,
             sizeof(root_id.bytes)) != 0) {
    pp_revision_event_set_release(revision_events);
    pp_production_release(production);
    pp_error_release(error);
    return 41;
  }
  pp_revision_event_set_release(revision_events);
  asset_exists = 0;
  status = pp_production_asset_exists(production, asset_id, &asset_exists, &error);
  if (status != PP_OK || asset_exists != UINT8_C(1)) {
    pp_production_release(production);
    pp_error_release(error);
    return 15;
  }
  pp_representation_set_t *representations = NULL;
  pp_asset_id_t representation_asset_id = {{0}};
  pp_representation_kind_t representation_kind = 0;
  pp_content_structure_kind_t structure_kind = 0;
  uint64_t member_count = 0;
  uint64_t representation_resource_count = 0;
  uint64_t representation_fingerprint_count = 0;
  status = pp_production_representations(production, asset_id,
                                         &representations, &error);
  if (status != PP_OK || representations == NULL ||
      pp_representation_set_count(representations) != UINT64_C(1) ||
      pp_representation_set_get(
          representations, 0, &representation_id, &representation_asset_id,
          &representation_kind, &structure_kind, &member_count,
          &representation_resource_count, &representation_fingerprint_count,
          &error) != PP_OK ||
      memcmp(representation_asset_id.bytes, asset_id.bytes,
             sizeof(asset_id.bytes)) != 0 ||
      representation_kind != PP_REPRESENTATION_ORIGINAL ||
      structure_kind != PP_CONTENT_SINGLE_RESOURCE ||
      member_count != UINT64_C(1) ||
      representation_resource_count != UINT64_C(1) ||
      representation_fingerprint_count != UINT64_C(1)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 43;
  }
  pp_dependency_set_t *dependencies = NULL;
  uint8_t dependencies_present = UINT8_C(1);
  pp_representation_id_t dependency_source_id = {{0}};
  uint64_t dependency_revision = UINT64_C(1);
  pp_dependency_set_status_t dependency_status = PP_DEPENDENCY_SET_CURRENT;
  uint64_t dependency_count = UINT64_C(1);
  status = pp_production_dependency_set(production, representation_id,
                                        &dependencies, &error);
  if (status != PP_OK || dependencies == NULL ||
      pp_dependency_set_get(dependencies, &dependencies_present,
                            &dependency_source_id, &dependency_revision,
                            &dependency_status, &dependency_count,
                            &error) != PP_OK ||
      dependencies_present != UINT8_C(0) || dependency_revision != UINT64_C(0) ||
      dependency_status != UINT32_C(0) || dependency_count != UINT64_C(0) ||
      memcmp(dependency_source_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0) {
    pp_dependency_set_release(dependencies);
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 43;
  }
  pp_dependency_set_release(dependencies);
  dependencies = NULL;
  const char *representation_fingerprint_algorithm = NULL;
  uint16_t representation_fingerprint_version = 0;
  const uint8_t *representation_fingerprint_value = NULL;
  uint64_t representation_fingerprint_value_length = 0;
  status = pp_representation_set_get_fingerprint(
      representations, 0, 0, &representation_fingerprint_algorithm,
      &representation_fingerprint_version, &representation_fingerprint_value,
      &representation_fingerprint_value_length, &error);
  if (status != PP_OK || representation_fingerprint_algorithm == NULL ||
      strcmp(representation_fingerprint_algorithm,
             "pp-blake3-representation") != 0 ||
      representation_fingerprint_version != UINT16_C(2) ||
      representation_fingerprint_value == NULL ||
      representation_fingerprint_value_length != UINT64_C(32)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 48;
  }
  const char *member_role = NULL;
  uint8_t member_required = 0;
  status = pp_representation_set_get_member(
      representations, 0, 0, &resource_id, &member_role, &member_required,
      &error);
  if (status != PP_OK || bytes_are_zero(resource_id.bytes) || member_role != NULL ||
      member_required != UINT8_C(1)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 44;
  }
  pp_resource_id_t inspected_resource_id = {{0}};
  uint8_t has_file_facts = 0;
  uint64_t file_size = 0;
  uint8_t has_modified_at = 0;
  int64_t modified_at = 0;
  uint64_t locator_count = 0;
  uint64_t resource_fingerprint_count = 0;
  status = pp_representation_set_get_resource(
      representations, 0, 0, &inspected_resource_id, &has_file_facts,
      &file_size, &has_modified_at, &modified_at, &locator_count,
      &resource_fingerprint_count, &error);
  if (status != PP_OK ||
      memcmp(inspected_resource_id.bytes, resource_id.bytes,
             sizeof(resource_id.bytes)) != 0 ||
      has_file_facts != UINT8_C(1) || file_size != UINT64_C(11) ||
      has_modified_at != UINT8_C(1) || modified_at == 0 ||
      locator_count != UINT64_C(1) ||
      resource_fingerprint_count != UINT64_C(1)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 45;
  }
  const char *fingerprint_algorithm = NULL;
  uint16_t fingerprint_version = 0;
  const uint8_t *fingerprint_value = NULL;
  uint64_t fingerprint_value_length = 0;
  status = pp_representation_set_get_resource_fingerprint(
      representations, 0, 0, 0, &fingerprint_algorithm, &fingerprint_version,
      &fingerprint_value, &fingerprint_value_length, &error);
  if (status != PP_OK || fingerprint_algorithm == NULL ||
      fingerprint_version != UINT16_C(1) || fingerprint_value == NULL ||
      fingerprint_value_length == UINT64_C(0)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 47;
  }
  pp_locator_id_t locator_id = {{0}};
  const char *locator_uri = NULL;
  pp_locator_availability_t locator_availability = 0;
  uint8_t has_last_seen = 0;
  int64_t last_seen = 0;
  uint8_t has_locator_naming = UINT8_C(9);
  pp_sequence_naming_t locator_naming = {NULL, NULL, 0};
  status = pp_representation_set_get_locator(
      representations, 0, 0, 0, &locator_id, &locator_uri,
      &locator_availability, &has_last_seen, &last_seen, &has_locator_naming,
      &locator_naming, &error);
  if (status != PP_OK || locator_id_is_zero(locator_id) || locator_uri == NULL ||
      has_locator_naming != UINT8_C(0) || locator_naming.prefix != NULL ||
      locator_availability != PP_LOCATOR_ONLINE ||
      has_last_seen != UINT8_C(1) || last_seen == 0) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 46;
  }
  char *media_locator = NULL;
  char *media_locator_path = NULL;
  status = pp_file_path_to_locator(media_path, &media_locator, &error);
  if (status == PP_OK) {
    status = pp_locator_to_file_path(media_locator, &media_locator_path,
                                     &error);
  }
  const int locator_matches =
      status == PP_OK && strcmp(media_locator, locator_uri) == 0 &&
      media_locator_path != NULL && media_locator_path[0] != '\0';
  pp_string_release(media_locator_path);
  pp_string_release(media_locator);
  if (!locator_matches) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 120;
  }
  pp_known_media_set_t *known_media = NULL;
  pp_asset_id_t known_asset_id = {{0}};
  pp_representation_id_t known_representation_id = {{0}};
  pp_resource_id_t known_resource_id = {{0}};
  status = pp_production_find_known_media_by_locator(
      production, locator_uri, NULL, UINT32_C(1), NULL, &known_media, &error);
  if (status != PP_OK || known_media == NULL ||
      pp_known_media_set_count(known_media) != UINT64_C(1) ||
      pp_known_media_set_next_cursor(known_media) != NULL ||
      pp_known_media_set_get(known_media, 0, &known_asset_id,
                             &known_representation_id, &known_resource_id,
                             &error) != PP_OK ||
      memcmp(known_asset_id.bytes, asset_id.bytes, sizeof(asset_id.bytes)) != 0 ||
      memcmp(known_representation_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      memcmp(known_resource_id.bytes, resource_id.bytes,
             sizeof(resource_id.bytes)) != 0) {
    pp_known_media_set_release(known_media);
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 181;
  }
  pp_known_media_set_release(known_media);
  known_media = NULL;
  status = pp_production_find_known_media_by_fingerprint(
      production, fingerprint_algorithm, fingerprint_version,
      fingerprint_value, fingerprint_value_length, UINT32_C(1), NULL,
      &known_media, &error);
  if (status != PP_OK || known_media == NULL ||
      pp_known_media_set_count(known_media) != UINT64_C(1) ||
      pp_known_media_set_get(known_media, 0, &known_asset_id,
                             &known_representation_id, &known_resource_id,
                             &error) != PP_OK ||
      memcmp(known_asset_id.bytes, asset_id.bytes, sizeof(asset_id.bytes)) != 0 ||
      memcmp(known_representation_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      memcmp(known_resource_id.bytes, resource_id.bytes,
             sizeof(resource_id.bytes)) != 0) {
    pp_known_media_set_release(known_media);
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 182;
  }
  pp_known_media_set_release(known_media);
  pp_representation_set_release(representations);
  pp_external_identifier_set_t *identifiers = NULL;
  status = pp_production_external_identifiers(production, &asset_ref, &identifiers,
                                           &error);
  const char *scheme = NULL;
  const char *external_value = NULL;
  const char *qualifier = NULL;
  if (status != PP_OK || identifiers == NULL ||
      pp_external_identifier_set_count(identifiers) != UINT64_C(1) ||
      pp_external_identifier_set_get(identifiers, 0, &scheme, &external_value,
                                     &qualifier, &error) != PP_OK ||
      strcmp(scheme, "com.example.asset") != 0 ||
      strcmp(external_value, "asset-42") != 0 ||
      strcmp(qualifier, "primary") != 0) {
    pp_external_identifier_set_release(identifiers);
    pp_production_release(production);
    pp_error_release(error);
    return 24;
  }
  pp_external_identifier_set_release(identifiers);
  pp_object_ref_set_t *objects = NULL;
  pp_object_ref_t found_object = {0, {{0}}};
  status = pp_production_find_by_external_identifier(
      production, "com.example.asset", "asset-42", "other", &objects, &error);
  if (status != PP_OK || objects == NULL ||
      pp_object_ref_set_count(objects) != 0) {
    pp_object_ref_set_release(objects);
    pp_production_release(production);
    pp_error_release(error);
    return 119;
  }
  pp_object_ref_set_release(objects);
  objects = NULL;
  status = pp_production_find_by_external_identifier(
      production, "com.example.asset", "asset-42", "primary", &objects, &error);
  if (status != PP_OK || objects == NULL ||
      pp_object_ref_set_count(objects) != UINT64_C(1) ||
      pp_object_ref_set_get(objects, 0, &found_object, &error) != PP_OK ||
      found_object.kind != PP_OBJECT_ASSET ||
      memcmp(found_object.id.bytes, asset_id.bytes, sizeof(asset_id.bytes)) !=
          0) {
    pp_object_ref_set_release(objects);
    pp_production_release(production);
    pp_error_release(error);
    return 25;
  }
  pp_object_ref_set_release(objects);

  pp_metadata_set_t *metadata = NULL;
  status = pp_production_metadata(production, &asset_ref, &metadata, &error);
  pp_object_ref_t metadata_target = {0, {{0}}};
  const char *vocabulary = NULL;
  const char *property = NULL;
  const pp_metadata_value_t *metadata_value = NULL;
  const char *metadata_text = NULL;
  const char *metadata_language = NULL;
  if (status != PP_OK || metadata == NULL ||
      pp_metadata_set_count(metadata) != UINT64_C(1) ||
      pp_metadata_set_get(metadata, 0, &metadata_target, &vocabulary, &property,
                          &metadata_value, &error) != PP_OK ||
      metadata_target.kind != PP_OBJECT_ASSET ||
      strcmp(vocabulary, "com.example.metadata") != 0 ||
      strcmp(property, "title") != 0 ||
      pp_metadata_value_kind(metadata_value) != PP_METADATA_LANG_STRING ||
      pp_metadata_value_get_string(metadata_value, &metadata_text,
                                   &metadata_language, &error) != PP_OK ||
      strcmp(metadata_text, "C title") != 0 ||
      strcmp(metadata_language, "en-US") != 0) {
    pp_metadata_set_release(metadata);
    pp_production_release(production);
    pp_error_release(error);
    return 27;
  }
  pp_metadata_set_release(metadata);
  metadata = NULL;
  status = pp_production_query_metadata(
      production, "com.example.metadata", "title", NULL, UINT32_C(1), NULL,
      &metadata, &error);
  if (status != PP_OK || metadata == NULL ||
      pp_metadata_set_count(metadata) != UINT64_C(1) ||
      pp_metadata_set_next_cursor(metadata) != NULL) {
    pp_metadata_set_release(metadata);
    pp_production_release(production);
    pp_error_release(error);
    return 80;
  }
  pp_metadata_set_release(metadata);
  metadata = NULL;
  status = pp_production_find_metadata(production, "com.example.metadata", "title",
                                    &metadata, &error);
  if (status != PP_OK || metadata == NULL ||
      pp_metadata_set_count(metadata) != UINT64_C(1)) {
    pp_metadata_set_release(metadata);
    pp_production_release(production);
    pp_error_release(error);
    return 28;
  }
  pp_metadata_set_release(metadata);

  pp_fingerprint_t *computed = NULL;
  const char *computed_algorithm = NULL;
  uint16_t computed_version = 0;
  const uint8_t *computed_value = NULL;
  uint64_t computed_length = 0;
  status = pp_fingerprint_file(media_path, &computed, &error);
  if (status != PP_OK ||
      pp_fingerprint_get(computed, &computed_algorithm, &computed_version,
                         &computed_value, &computed_length, &error) != PP_OK ||
      strcmp(computed_algorithm, "pp-blake3-full-file") != 0 ||
      computed_version != 1 || computed_length != UINT64_C(32) ||
      computed_value == NULL) {
    pp_fingerprint_release(computed);
    pp_production_release(production);
    pp_error_release(error);
    return 115;
  }
  pp_fingerprint_release(computed);

  pp_content_verification_t verification = 0;
  status = pp_production_verify_resource(production, resource_id, media_path,
                                         NULL, &verification, &error);
  if (status != PP_OK || verification != PP_CONTENT_MATCHES) {
    pp_production_release(production);
    pp_error_release(error);
    return 116;
  }
  char other_media_path[4096];
  int other_path_length = snprintf(other_media_path, sizeof(other_media_path),
                                   "%s.other", argv[1]);
  FILE *other_media = NULL;
  if (other_path_length < 0 ||
      (size_t)other_path_length >= sizeof(other_media_path) ||
      (other_media = fopen(other_media_path, "wb")) == NULL) {
    pp_production_release(production);
    return 116;
  }
  fputs("different content", other_media);
  fclose(other_media);
  status = pp_production_verify_resource(production, resource_id,
                                         other_media_path, NULL, &verification,
                                         &error);
  remove(other_media_path);
  if (status != PP_OK || verification != PP_CONTENT_DIFFERS) {
    pp_production_release(production);
    pp_error_release(error);
    return 117;
  }

  pp_transaction_t *observation = NULL;
  pp_content_observation_t outcome = 0;
  if (!observation_base_contract(production, resource_id, representation_id)) {
    pp_production_release(production);
    return 118;
  }
  status = begin_decision_edit(production, &observation, &error);
  if (status == PP_OK) {
    status = pp_transaction_observe_resource_content(observation, resource_id,
                                                     media_path, NULL, &outcome,
                                                     &error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(observation, &error);
  }
  pp_transaction_release(observation);
  if (status != PP_OK || outcome != PP_OBSERVATION_UNCHANGED) {
    pp_production_release(production);
    pp_error_release(error);
    return 118;
  }

  int moved_path_length =
      snprintf(moved_media_path, sizeof(moved_media_path), "%s.moved", argv[1]);
  if (moved_path_length < 0 ||
      (size_t)moved_path_length >= sizeof(moved_media_path) ||
      rename(media_path, moved_media_path) != 0) {
    pp_production_release(production);
    return 16;
  }

  pp_resolution_set_t *resolutions = NULL;
  pp_resolution_options_t *resolution_options = NULL;
  pp_cancel_token_t *cancel_token = NULL;
  status = pp_resolution_options_create(&resolution_options, &error);
  if (status == PP_OK) {
    status = pp_resolution_options_add_root_mapping(resolution_options,
                                                    "fixtures", argv[2], &error);
  }
  if (status == PP_OK) {
    status = pp_resolution_options_add_search_directory(resolution_options,
                                                        argv[2], &error);
  }
  if (status == PP_OK) {
    status = pp_resolution_options_set_verification(
        resolution_options, PP_VERIFY_PRESENCE, &error);
  }
  if (status == PP_OK) {
    status = pp_resolution_options_set_limits(resolution_options, 64,
                                              UINT64_C(1000000), &error);
  }
  if (status == PP_OK) {
    status = pp_cancel_token_create(&cancel_token, &error);
  }
  if (status == PP_OK) {
    status = pp_resolution_options_set_cancel_token(resolution_options,
                                                    cancel_token, &error);
  }
  if (status == PP_OK) {
    status = pp_production_resolve_assets(production, &asset_id, UINT64_C(1),
                                          resolution_options, &resolutions,
                                          &error);
  }
  if (status != PP_OK || resolutions == NULL ||
      pp_resolution_set_representation_count(resolutions) != UINT64_C(1)) {
    pp_resolution_set_release(resolutions);
    pp_resolution_options_release(resolution_options);
    pp_cancel_token_release(cancel_token);
    pp_production_release(production);
    pp_error_release(error);
    return 17;
  }
  /* A cancelled token stops a later call; the options share its flag even
   * after the token handle is released. */
  pp_cancel_token_cancel(cancel_token);
  pp_cancel_token_release(cancel_token);
  pp_resolution_set_t *cancelled = NULL;
  status = pp_production_resolve_assets(production, &asset_id, UINT64_C(1),
                                        resolution_options, &cancelled, &error);
  pp_resolution_options_release(resolution_options);
  if (status != PP_ERROR_CANCELLED || cancelled != NULL) {
    pp_resolution_set_release(cancelled);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 121;
  }
  pp_error_release(error);
  error = NULL;
  pp_representation_availability_t availability = 0;
  uint64_t resource_count = 0;
  uint64_t issue_count = 0;
  pp_asset_id_t resolved_asset_id = {{0}};
  status = pp_resolution_set_get_representation(
      resolutions, 0, &resolved_asset_id, &representation_id, &availability,
      &resource_count, &issue_count, &error);
  if (status != PP_OK || availability != PP_AVAILABILITY_ONLINE ||
      resource_count != UINT64_C(1) || issue_count != 0 ||
      representation_id_is_zero(&representation_id) ||
      memcmp(resolved_asset_id.bytes, asset_id.bytes, sizeof asset_id.bytes) !=
          0) {
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 18;
  }
  pp_resource_resolution_state_t state = 0;
  uint64_t candidate_count = 0;
  uint64_t result_evidence_count = 0;
  status = pp_resolution_set_get_resource(
      resolutions, 0, 0, &resource_id, &state, &candidate_count,
      &result_evidence_count, &error);
  if (status != PP_OK || state != PP_RESOURCE_RESOLVED_EXACT ||
      candidate_count != UINT64_C(1) || bytes_are_zero(resource_id.bytes)) {
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 18;
  }
  const char *candidate_uri = NULL;
  uint16_t confidence = 0;
  const char *candidate_root = NULL;
  uint8_t candidate_has_naming = UINT8_C(9);
  pp_sequence_naming_t candidate_naming = {NULL, NULL, 0};
  uint64_t candidate_evidence_count = 0;
  status = pp_resolution_set_get_candidate(
      resolutions, 0, 0, 0, &candidate_uri, &confidence, &candidate_root,
      &candidate_has_naming, &candidate_naming, &candidate_evidence_count,
      &error);
  if (status != PP_OK || candidate_uri == NULL ||
      candidate_has_naming != UINT8_C(0) ||
      confidence != UINT16_C(10000) || candidate_evidence_count == 0 ||
      candidate_root == NULL || strcmp(candidate_root, "fixtures") != 0) {
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 19;
  }
  pp_evidence_kind_t evidence_kind = 0;
  const char *evidence_detail = NULL;
  status = pp_resolution_set_get_candidate_evidence(
      resolutions, 0, 0, 0, 0, &evidence_kind, &evidence_detail, &error);
  if (status != PP_OK || evidence_kind == 0) {
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 20;
  }

  status = begin_decision_edit(production, &transaction, &error);
  const uint8_t observed_resource_fingerprint[] = {0x10, 0x20, 0x30};
  const uint8_t observed_representation_fingerprint[] = {0x40, 0x50, 0x60};
  if (status != PP_OK ||
      pp_transaction_confirm_locator(transaction, resource_id, candidate_uri,
                                     "fixtures", NULL, &error) != PP_OK ||
      pp_transaction_record_resource_fingerprint(
          transaction, resource_id, "c-smoke", UINT16_C(1),
          observed_resource_fingerprint,
          sizeof(observed_resource_fingerprint), &error) != PP_OK ||
      pp_transaction_record_representation_fingerprint(
          transaction, representation_id, "c-smoke-tree", UINT16_C(1),
          observed_representation_fingerprint,
          sizeof(observed_representation_fingerprint), &error) != PP_OK ||
      pp_transaction_set_media_root_enabled(transaction, root_id, 0, &error) !=
          PP_OK ||
      pp_transaction_retire_locator(transaction, locator_id, &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 21;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  representations = NULL;
  status = pp_production_representations_under_media_root(
      production, "fixtures", UINT32_C(1), NULL, &representations, &error);
  if (status != PP_OK || representations == NULL ||
      pp_representation_set_count(representations) != UINT64_C(1) ||
      pp_representation_set_next_cursor(representations) != NULL) {
    pp_representation_set_release(representations);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 81;
  }
  pp_representation_set_release(representations);

  pp_object_query_set_t *resource_page = NULL;
  status = pp_production_resources_page(
      production, representation_id, UINT32_C(1), NULL, &resource_page,
      &error);
  pp_object_ref_t queried_resource = {0};
  uint32_t resource_depth = UINT32_MAX;
  if (status != PP_OK || resource_page == NULL ||
      pp_object_query_set_count(resource_page) != UINT64_C(1) ||
      pp_object_query_set_get(resource_page, UINT64_C(0), &queried_resource,
                              &resource_depth, &error) != PP_OK ||
      queried_resource.kind != PP_OBJECT_RESOURCE || resource_depth != 0 ||
      memcmp(queried_resource.id.bytes, resource_id.bytes,
             sizeof(resource_id.bytes)) != 0) {
    pp_object_query_set_release(resource_page);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 84;
  }
  pp_object_query_set_release(resource_page);

  pp_asset_set_t *point_asset = NULL;
  pp_asset_id_t point_asset_id = {{0}};
  int64_t point_asset_created_at = 0;
  const char *point_asset_name = NULL;
  const char *point_asset_source = NULL;
  status = pp_production_asset(production, asset_id, &point_asset, &error);
  if (status != PP_OK || point_asset == NULL ||
      pp_asset_set_count(point_asset) != UINT64_C(1) ||
      pp_asset_set_get(point_asset, UINT64_C(0), &point_asset_id,
                       &point_asset_created_at, &point_asset_name,
                       &point_asset_source, &error) != PP_OK ||
      memcmp(point_asset_id.bytes, asset_id.bytes, sizeof(asset_id.bytes)) !=
          0 ||
      pp_asset_set_next_cursor(point_asset) != NULL) {
    pp_asset_set_release(point_asset);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 110;
  }
  pp_asset_set_release(point_asset);

  pp_representation_set_t *point_representation = NULL;
  pp_representation_id_t point_representation_id = {{0}};
  pp_asset_id_t point_representation_asset_id = {{0}};
  pp_representation_kind_t point_kind = 0;
  pp_content_structure_kind_t point_structure = 0;
  uint64_t point_members = 0;
  uint64_t point_resources = 0;
  uint64_t point_fingerprints = 0;
  status = pp_production_representation(production, representation_id,
                                        &point_representation, &error);
  if (status != PP_OK || point_representation == NULL ||
      pp_representation_set_count(point_representation) != UINT64_C(1) ||
      pp_representation_set_get(
          point_representation, UINT64_C(0), &point_representation_id,
          &point_representation_asset_id, &point_kind, &point_structure,
          &point_members, &point_resources, &point_fingerprints,
          &error) != PP_OK ||
      memcmp(point_representation_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      memcmp(point_representation_asset_id.bytes, asset_id.bytes,
             sizeof(asset_id.bytes)) != 0) {
    pp_representation_set_release(point_representation);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 111;
  }
  pp_representation_set_release(point_representation);
  point_representation = NULL;

  status = pp_production_representations_using_resource(
      production, resource_id, UINT32_C(10), NULL, &point_representation,
      &error);
  if (status != PP_OK || point_representation == NULL ||
      pp_representation_set_count(point_representation) != UINT64_C(1) ||
      pp_representation_set_get(
          point_representation, UINT64_C(0), &point_representation_id,
          &point_representation_asset_id, &point_kind, &point_structure,
          &point_members, &point_resources, &point_fingerprints,
          &error) != PP_OK ||
      memcmp(point_representation_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      pp_representation_set_next_cursor(point_representation) != NULL) {
    pp_representation_set_release(point_representation);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 112;
  }
  pp_representation_set_release(point_representation);
  point_representation = NULL;

  pp_representation_id_t absent_id = {{0}};
  absent_id.bytes[0] = 0xff;
  status = pp_production_representation(production, absent_id,
                                        &point_representation, &error);
  if (status != PP_ERROR_NOT_FOUND || point_representation != NULL) {
    pp_representation_set_release(point_representation);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 113;
  }
  pp_error_release(error);
  error = NULL;

  pp_locator_query_set_t *locator_page = NULL;
  status = pp_production_locators_page(
      production, resource_id, UINT32_C(10), NULL, &locator_page, &error);
  pp_locator_id_t queried_locator_id = {{0}};
  pp_resource_id_t queried_locator_resource_id = {{0}};
  const char *queried_locator_uri = NULL;
  pp_locator_availability_t queried_locator_availability = 0;
  uint8_t queried_locator_has_last_seen = 0;
  int64_t queried_locator_last_seen = 0;
  const char *queried_locator_root = NULL;
  uint8_t queried_locator_has_naming = UINT8_C(9);
  pp_sequence_naming_t queried_locator_naming = {NULL, NULL, 0};
  if (status != PP_OK || locator_page == NULL ||
      pp_locator_query_set_count(locator_page) == UINT64_C(0) ||
      pp_locator_query_set_get(
          locator_page, pp_locator_query_set_count(locator_page) - UINT64_C(1),
          &queried_locator_id, &queried_locator_resource_id,
          &queried_locator_uri, &queried_locator_availability,
          &queried_locator_has_last_seen, &queried_locator_last_seen,
          &queried_locator_root, &queried_locator_has_naming,
          &queried_locator_naming, &error) != PP_OK ||
      queried_locator_has_naming != UINT8_C(0) ||
      locator_id_is_zero(queried_locator_id) || queried_locator_uri == NULL ||
      queried_locator_root == NULL ||
      strcmp(queried_locator_root, "fixtures") != 0 ||
      memcmp(queried_locator_resource_id.bytes, resource_id.bytes,
             sizeof(resource_id.bytes)) != 0 ||
      queried_locator_availability != PP_LOCATOR_ONLINE ||
      queried_locator_has_last_seen != UINT8_C(1) ||
      queried_locator_last_seen == 0 ||
      pp_locator_query_set_next_cursor(locator_page) != NULL) {
    pp_locator_query_set_release(locator_page);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 85;
  }
  pp_locator_query_set_release(locator_page);

  revisions = NULL;
  status = pp_production_latest_revision(production, &revisions, &error);
  if (status != PP_OK || revisions == NULL ||
      pp_revision_set_get(
          revisions, 0, &revision_id, &revision_sequence,
          &revision_transaction_id, &revision_committed_at,
          &revision_origin_name, &revision_origin_version,
          &revision_origin_uri, &revision_message, &error) != PP_OK) {
    pp_revision_set_release(revisions);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 78;
  }
  pp_revision_set_release(revisions);
  revision_events = NULL;
  status = pp_production_revision_events(production, revision_id,
                                         &revision_events, &error);
  if (status != PP_OK || revision_events == NULL ||
      pp_revision_event_set_get(revision_events, 1, &revision_event, &error) !=
          PP_OK ||
      revision_event.kind != PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED ||
      revision_event.fingerprint_algorithm == NULL ||
      strcmp(revision_event.fingerprint_algorithm, "c-smoke") != 0 ||
      revision_event.fingerprint_version != UINT16_C(1) ||
      pp_revision_event_set_get(revision_events, 2, &revision_event, &error) !=
          PP_OK ||
      revision_event.kind !=
          PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED ||
      revision_event.fingerprint_algorithm == NULL ||
      strcmp(revision_event.fingerprint_algorithm, "c-smoke-tree") != 0 ||
      revision_event.fingerprint_version != UINT16_C(1)) {
    pp_revision_event_set_release(revision_events);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 79;
  }
  pp_revision_event_set_release(revision_events);

  roots = NULL;
  status = pp_production_media_roots(production, &roots, &error);
  if (status != PP_OK || roots == NULL ||
      pp_media_root_set_get(roots, 0, &read_root_id, &root_name, &root_label,
                            &root_legacy_uri, &root_priority, &root_enabled,
                            &error) != PP_OK ||
      root_enabled != UINT8_C(0)) {
    pp_media_root_set_release(roots);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 76;
  }
  pp_media_root_set_release(roots);

  status = begin_decision_edit(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_remove_media_root(transaction, root_id, &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 77;
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  roots = NULL;
  status = pp_production_media_roots(production, &roots, &error);
  if (status != PP_OK || roots == NULL || pp_media_root_set_count(roots) != 0) {
    pp_media_root_set_release(roots);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 78;
  }
  pp_media_root_set_release(roots);

  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK || transaction == NULL) {
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 29;
  }
  const pp_activity_edge_t activity_output = {
      representation_id, "org.postproject:output.master"};
  const int64_t started_at = INT64_C(100);
  const int64_t finished_at = INT64_C(200);
  pp_activity_id_t activity_id = {{0}};
  status = pp_transaction_create_activity(
      transaction, "org.postproject:ingest", NULL, 0, &activity_output, 1,
      &started_at, &finished_at, "C ingest", "1.0",
      "https://example.com/tools/ingest", "C operator", "com.example.agent",
      "operator-1", "primary", &activity_id, &error);
  pp_object_ref_t activity_ref = {0};
  if (status == PP_OK) status = pp_object_ref_from_activity(activity_id, &activity_ref, &error);
  if (status != PP_OK || activity_id_is_zero(activity_id)) {
    pp_transaction_release(transaction);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 30;
  }
  pp_metadata_input_t *rate_input = NULL;
  status = pp_metadata_input_create_rational(INT64_C(24000), UINT64_C(1001),
                                             &rate_input, &error);
  if (status == PP_OK) {
    status = pp_transaction_add_metadata_value(
        transaction, &activity_ref, "com.example.ingest", "rate", rate_input,
        &error);
  }
  pp_metadata_input_release(rate_input);
  if (status != PP_OK || pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 30;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  metadata = NULL;
  status = pp_production_metadata(production, &activity_ref, &metadata, &error);
  int64_t rate_numerator = 0;
  uint64_t rate_denominator = 0;
  if (status != PP_OK || metadata == NULL ||
      pp_metadata_set_count(metadata) != UINT64_C(1) ||
      pp_metadata_set_get(metadata, 0, &metadata_target, &vocabulary, &property,
                          &metadata_value, &error) != PP_OK ||
      strcmp(vocabulary, "com.example.ingest") != 0 ||
      strcmp(property, "rate") != 0 ||
      pp_metadata_value_get_rational(metadata_value, &rate_numerator,
                                     &rate_denominator, &error) != PP_OK ||
      rate_numerator != INT64_C(24000) || rate_denominator != UINT64_C(1001)) {
    pp_metadata_set_release(metadata);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 30;
  }
  pp_metadata_set_release(metadata);

  pp_activity_set_t *activities = NULL;
  status = pp_production_activities(production, &activities, &error);
  pp_activity_id_t read_activity_id = {{0}};
  const char *activity_kind = NULL;
  uint8_t has_started_at = 0;
  int64_t read_started_at = 0;
  uint8_t has_finished_at = 0;
  int64_t read_finished_at = 0;
  uint64_t input_count = 0;
  uint64_t output_count = 0;
  if (status != PP_OK || activities == NULL ||
      pp_activity_set_count(activities) != UINT64_C(1) ||
      pp_activity_set_get(activities, 0, &read_activity_id, &activity_kind,
                          &has_started_at, &read_started_at, &has_finished_at,
                          &read_finished_at, &input_count, &output_count,
                          &error) != PP_OK ||
      memcmp(read_activity_id.bytes, activity_id.bytes,
             sizeof(activity_id.bytes)) != 0 ||
      activity_kind == NULL || strcmp(activity_kind, "org.postproject:ingest") != 0 ||
      has_started_at != UINT8_C(1) || read_started_at != started_at ||
      has_finished_at != UINT8_C(1) || read_finished_at != finished_at ||
      input_count != 0 || output_count != UINT64_C(1)) {
    pp_activity_set_release(activities);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 31;
  }
  const char *tool_name = NULL;
  const char *tool_version = NULL;
  const char *tool_uri = NULL;
  const char *agent_name = NULL;
  const char *agent_scheme = NULL;
  const char *agent_value = NULL;
  const char *agent_qualifier = NULL;
  pp_representation_id_t output_representation_id = {{0}};
  const char *output_role = NULL;
  uint8_t has_output_snapshot = 0;
  uint64_t output_snapshot_revision = 0;
  uint64_t output_snapshot_fingerprint_count = 0;
  const char *snapshot_algorithm = NULL;
  uint16_t snapshot_version = 0;
  const uint8_t *snapshot_value = NULL;
  uint64_t snapshot_value_length = 0;
  uint8_t has_observed_revision = 0;
  uint64_t observed_revision = 0;
  if (pp_activity_set_get_tool(activities, 0, &tool_name, &tool_version,
                               &tool_uri, &error) != PP_OK ||
      tool_name == NULL || strcmp(tool_name, "C ingest") != 0 ||
      tool_version == NULL || strcmp(tool_version, "1.0") != 0 ||
      tool_uri == NULL ||
      strcmp(tool_uri, "https://example.com/tools/ingest") != 0 ||
      pp_activity_set_get_agent(
          activities, 0, &agent_name, &agent_scheme, &agent_value,
          &agent_qualifier, &error) != PP_OK ||
      agent_name == NULL || strcmp(agent_name, "C operator") != 0 ||
      agent_scheme == NULL || strcmp(agent_scheme, "com.example.agent") != 0 ||
      agent_value == NULL || strcmp(agent_value, "operator-1") != 0 ||
      agent_qualifier == NULL || strcmp(agent_qualifier, "primary") != 0 ||
      pp_activity_set_get_output(activities, 0, 0,
                                 &output_representation_id, &output_role,
                                 &error) != PP_OK ||
      memcmp(output_representation_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      output_role == NULL ||
      strcmp(output_role, "org.postproject:output.master") != 0 ||
      pp_activity_set_get_output_snapshot(
          activities, 0, 0, &has_output_snapshot, &output_snapshot_revision,
          &output_snapshot_fingerprint_count, &error) != PP_OK ||
      has_output_snapshot != UINT8_C(1) || output_snapshot_revision == 0 ||
      output_snapshot_fingerprint_count < UINT64_C(1) ||
      pp_activity_set_get_output_snapshot_fingerprint(
          activities, 0, 0, 0, &snapshot_algorithm, &snapshot_version,
          &snapshot_value, &snapshot_value_length, &has_observed_revision,
          &observed_revision, &error) != PP_OK ||
      snapshot_algorithm == NULL || snapshot_version == 0 ||
      snapshot_value == NULL || snapshot_value_length == 0 ||
      has_observed_revision != UINT8_C(1) || observed_revision == 0) {
    pp_activity_set_release(activities);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 32;
  }
  pp_activity_set_release(activities);
  activities = NULL;
  status = pp_production_activities_producing(
      production, representation_id, &activities, &error);
  if (status != PP_OK || activities == NULL ||
      pp_activity_set_count(activities) != UINT64_C(1)) {
    pp_activity_set_release(activities);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 33;
  }
  pp_activity_set_release(activities);

  activities = NULL;
  status = pp_production_activities_producing_page(
      production, representation_id, UINT32_C(1), NULL, &activities, &error);
  if (status != PP_OK || activities == NULL ||
      pp_activity_set_count(activities) != UINT64_C(1) ||
      pp_activity_set_next_cursor(activities) != NULL) {
    pp_activity_set_release(activities);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 82;
  }
  pp_activity_set_release(activities);

  pp_object_query_set_t *query_objects = NULL;
  pp_object_ref_t query_object = {0};
  uint32_t query_depth = UINT32_MAX;
  status = pp_production_outputs_by_activity_kind(
      production, "org.postproject:ingest", UINT32_C(1), NULL, &query_objects,
      &error);
  if (status != PP_OK || query_objects == NULL ||
      pp_object_query_set_count(query_objects) != UINT64_C(1) ||
      pp_object_query_set_get(query_objects, UINT64_C(0), &query_object,
                              &query_depth, &error) != PP_OK ||
      query_object.kind != PP_OBJECT_REPRESENTATION || query_depth != 0 ||
      memcmp(query_object.id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      pp_object_query_set_next_cursor(query_objects) != NULL ||
      pp_object_query_set_traversal_truncated(query_objects) != UINT8_C(0)) {
    pp_object_query_set_release(query_objects);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 83;
  }
  pp_object_query_set_release(query_objects);

  pp_artifact_evaluation_t *artifact_evaluation = NULL;
  pp_representation_id_t evaluated_representation_id = {{0}};
  pp_artifact_knowledge_state_t artifact_state = 0;
  uint32_t visited_representations = 0;
  uint8_t artifact_truncated = 0;
  uint64_t artifact_reason_count = 0;
  status = pp_production_evaluate_artifact(
      production, representation_id, UINT32_C(64), UINT32_C(1000),
      &artifact_evaluation, &error);
  if (status != PP_OK || artifact_evaluation == NULL ||
      pp_artifact_evaluation_get(
          artifact_evaluation, &evaluated_representation_id, &artifact_state,
          &visited_representations, &artifact_truncated,
          &artifact_reason_count, &error) != PP_OK ||
      memcmp(evaluated_representation_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      artifact_state != PP_ARTIFACT_CURRENT ||
      visited_representations != UINT32_C(1) ||
      artifact_truncated != UINT8_C(0) || artifact_reason_count != 0) {
    pp_artifact_evaluation_release(artifact_evaluation);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 80;
  }
  pp_artifact_evaluation_release(artifact_evaluation);

  pp_artifact_reproducibility_t *reproducibility = NULL;
  pp_representation_id_t reproducibility_representation_id = {{0}};
  pp_activity_id_t producing_activity_id = {{0}};
  uint8_t reproducible = 0;
  uint8_t has_producing_activity = 0;
  const char *producing_activity_kind = NULL;
  uint64_t reproducibility_issue_count = 0;
  status = pp_production_artifact_reproducibility(
      production, representation_id, &reproducibility, &error);
  if (status != PP_OK || reproducibility == NULL ||
      pp_artifact_reproducibility_get(
          reproducibility, &reproducibility_representation_id, &reproducible,
          &has_producing_activity, &producing_activity_id,
          &producing_activity_kind, &reproducibility_issue_count,
          &error) != PP_OK ||
      reproducible != UINT8_C(1) ||
      has_producing_activity != UINT8_C(1) ||
      memcmp(producing_activity_id.bytes, activity_id.bytes,
             sizeof(activity_id.bytes)) != 0 ||
      producing_activity_kind == NULL ||
      strcmp(producing_activity_kind, "org.postproject:ingest") != 0 ||
      reproducibility_issue_count != 0) {
    pp_artifact_reproducibility_release(reproducibility);
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 81;
  }
  pp_artifact_reproducibility_release(reproducibility);
  pp_resolution_set_release(resolutions);
  pp_production_release(production);
  production = NULL;

  status = pp_production_open(argv[1], &production, &error);
  if (status != PP_OK ||
      pp_production_resolve_assets(production, &asset_id, UINT64_C(1), NULL,
                                   &resolutions, &error) != PP_OK ||
      pp_resolution_set_get_representation(
          resolutions, 0, &resolved_asset_id, &representation_id,
          &availability, &resource_count, &issue_count, &error) != PP_OK ||
      pp_resolution_set_get_resource(
          resolutions, 0, 0, &resource_id, &state, &candidate_count,
          &result_evidence_count, &error) != PP_OK ||
      availability != PP_AVAILABILITY_ONLINE ||
      state != PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR) {
    pp_resolution_set_release(resolutions);
    pp_production_release(production);
    pp_error_release(error);
    return 22;
  }
  pp_resolution_set_release(resolutions);

  pp_representation_id_t proxy_representation_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      add_file(transaction, &asset_id, PP_REPRESENTATION_PROXY,
               moved_media_path, &proxy_representation_id, &error) != PP_OK ||
      representation_id_is_zero(&proxy_representation_id) ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 67;
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  representations = NULL;
  if (pp_production_representations(production, asset_id, &representations,
                                    &error) != PP_OK ||
      representations == NULL ||
      pp_representation_set_count(representations) != UINT64_C(2)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 68;
  }
  pp_representation_set_release(representations);

  int sequence_path_length = snprintf(sequence_frame_path,
                                      sizeof(sequence_frame_path),
                                      "%s/frame0001.exr", argv[2]);
  FILE *sequence_frame = sequence_path_length > 0 &&
                                 (size_t)sequence_path_length <
                                     sizeof(sequence_frame_path)
                             ? fopen(sequence_frame_path, "wb")
                             : NULL;
  if (sequence_frame == NULL ||
      fwrite("sequence frame", 1, 14, sequence_frame) != 14 ||
      fclose(sequence_frame) != 0) {
    pp_production_release(production);
    return 69;
  }
  pp_media_source_t *sequence_source = NULL;
  const pp_sequence_naming_t frame_naming = {"frame", ".exr", UINT8_C(4)};
  if (pp_media_source_create_image_sequence(
          argv[2], &frame_naming, INT64_C(1), INT64_C(1),
          UINT32_C(1), UINT32_C(24000), UINT32_C(1001), NULL, 0,
          &sequence_source, &error) != PP_OK ||
      sequence_source == NULL) {
    pp_production_release(production);
    pp_error_release(error);
    return 122;
  }
  pp_representation_id_t sequence_representation_id = {{0}};
  pp_asset_id_t sequence_asset_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_add_representation(
          transaction, asset_id, PP_REPRESENTATION_DERIVED, sequence_source,
          &sequence_representation_id, &error) != PP_OK ||
      representation_id_is_zero(&sequence_representation_id) ||
      pp_transaction_import_media(transaction, sequence_source,
                                  "C image strip", &sequence_asset_id,
                                  &error) != PP_OK ||
      bytes_are_zero(sequence_asset_id.bytes) ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_media_source_release(sequence_source);
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 70;
  }
  pp_media_source_release(sequence_source);
  pp_transaction_release(transaction);
  transaction = NULL;
  representations = NULL;
  pp_representation_id_t strip_representation_id = {{0}};
  pp_asset_id_t strip_asset_id = {{0}};
  pp_representation_kind_t strip_kind = 0;
  pp_content_structure_kind_t strip_structure = 0;
  uint64_t strip_members = 0;
  uint64_t strip_resources = 0;
  uint64_t strip_fingerprints = 0;
  int64_t strip_start = 0;
  int64_t strip_end = 0;
  uint32_t strip_step = 0;
  uint32_t strip_rate_numerator = 0;
  uint32_t strip_rate_denominator = 0;
  uint64_t strip_missing = 0;
  if (pp_production_representations(production, sequence_asset_id,
                                    &representations, &error) != PP_OK ||
      representations == NULL ||
      pp_representation_set_count(representations) != UINT64_C(1) ||
      pp_representation_set_get(representations, 0, &strip_representation_id,
                                &strip_asset_id, &strip_kind, &strip_structure,
                                &strip_members, &strip_resources,
                                &strip_fingerprints, &error) != PP_OK ||
      strip_kind != PP_REPRESENTATION_ORIGINAL ||
      strip_structure != PP_CONTENT_IMAGE_SEQUENCE ||
      pp_representation_set_get_sequence(
          representations, 0, &strip_start, &strip_end, &strip_step,
          &strip_rate_numerator, &strip_rate_denominator, &strip_missing,
          &error) != PP_OK ||
      strip_start != INT64_C(1) || strip_end != INT64_C(1)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 123;
  }
  pp_representation_set_release(representations);
  representations = NULL;
  if (pp_production_representations(production, asset_id, &representations,
                                    &error) != PP_OK ||
      representations == NULL ||
      pp_representation_set_count(representations) != UINT64_C(3)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 71;
  }
  pp_representation_set_release(representations);

  const pp_file_resource_input_t ordered_members[] = {
      {moved_media_path, "org.postproject:essence.first", UINT8_C(1)},
      {sequence_frame_path, "org.postproject:essence.second", UINT8_C(1)},
  };
  const pp_file_resource_input_t package_members[] = {
      {moved_media_path, "org.postproject:essence", UINT8_C(1)},
      {sequence_frame_path, "org.postproject:sidecar", UINT8_C(0)},
  };
  pp_media_source_t *ordered_source = NULL;
  pp_media_source_t *package_source = NULL;
  pp_media_source_t *invalid_source = NULL;
  if (pp_media_source_create_ordered_parts(ordered_members, UINT64_C(2),
                                           &ordered_source, &error) != PP_OK ||
      pp_media_source_create_package(package_members, UINT64_C(2),
                                     &package_source, &error) != PP_OK) {
    pp_media_source_release(ordered_source);
    pp_production_release(production);
    pp_error_release(error);
    return 124;
  }
  /* Ordered parts must all be required; the source is rejected up front. */
  if (pp_media_source_create_ordered_parts(package_members, UINT64_C(2),
                                           &invalid_source, &error) !=
          PP_ERROR_INVALID_ARGUMENT ||
      invalid_source != NULL) {
    pp_media_source_release(invalid_source);
    pp_media_source_release(ordered_source);
    pp_media_source_release(package_source);
    pp_production_release(production);
    pp_error_release(error);
    return 125;
  }
  pp_error_release(error);
  error = NULL;
  pp_representation_id_t ordered_representation_id = {{0}};
  pp_representation_id_t package_representation_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_add_representation(
          transaction, asset_id, PP_REPRESENTATION_OPTIMIZED, ordered_source,
          &ordered_representation_id, &error) != PP_OK ||
      pp_transaction_add_representation(
          transaction, asset_id, PP_REPRESENTATION_DERIVED, package_source,
          &package_representation_id, &error) != PP_OK ||
      representation_id_is_zero(&ordered_representation_id) ||
      representation_id_is_zero(&package_representation_id) ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_media_source_release(ordered_source);
    pp_media_source_release(package_source);
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 72;
  }
  pp_media_source_release(ordered_source);
  pp_media_source_release(package_source);
  pp_media_source_release(NULL);
  pp_transaction_release(transaction);
  transaction = NULL;
  representations = NULL;
  if (pp_production_representations(production, asset_id, &representations,
                                    &error) != PP_OK ||
      representations == NULL ||
      pp_representation_set_count(representations) != UINT64_C(5)) {
    pp_representation_set_release(representations);
    pp_production_release(production);
    pp_error_release(error);
    return 73;
  }
  pp_representation_set_release(representations);

  pp_object_ref_t proxy_target;
  status = pp_object_ref_from_representation(proxy_representation_id, &proxy_target, &error);
  if (status != PP_OK) {
    pp_production_release(production);
    pp_error_release(error);
    return 117;
  }
  const pp_dependency_t recorded_dependency = {
      UINT8_C(0),
      {{0}},
      "org.postproject:requires",
      proxy_target,
      UINT8_C(0),
      {{0}},
      UINT8_C(1),
      "proxy.mov",
  };
  status = begin_decision_edit(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_record_dependency_set(
          transaction, representation_id, &recorded_dependency, UINT64_C(1),
          &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 74;
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  revisions = NULL;
  if (pp_production_latest_revision(production, &revisions, &error) != PP_OK ||
      revisions == NULL || pp_revision_set_count(revisions) != UINT64_C(1) ||
      pp_revision_set_get(revisions, UINT64_C(0), &revision_id,
                          &revision_sequence, &revision_transaction_id,
                          &revision_committed_at, &revision_origin_name,
                          &revision_origin_version, &revision_origin_uri,
                          &revision_message, &error) != PP_OK) {
    pp_revision_set_release(revisions);
    pp_production_release(production);
    pp_error_release(error);
    return 80;
  }
  pp_revision_set_release(revisions);
  revision_events = NULL;
  if (pp_production_revision_events(production, revision_id, &revision_events,
                                    &error) != PP_OK ||
      revision_events == NULL ||
      pp_revision_event_set_count(revision_events) != UINT64_C(1) ||
      pp_revision_event_set_get(revision_events, UINT64_C(0), &revision_event,
                                &error) != PP_OK ||
      revision_event.kind != PP_REVISION_DEPENDENCY_SET_RECORDED ||
      memcmp(revision_event.representation_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0) {
    pp_revision_event_set_release(revision_events);
    pp_production_release(production);
    pp_error_release(error);
    return 81;
  }
  pp_revision_event_set_release(revision_events);
  status = pp_production_dependency_set(production, representation_id,
                                        &dependencies, &error);
  pp_dependency_t read_dependency = {0};
  if (status != PP_OK || dependencies == NULL ||
      pp_dependency_set_get(dependencies, &dependencies_present,
                            &dependency_source_id, &dependency_revision,
                            &dependency_status, &dependency_count,
                            &error) != PP_OK ||
      dependencies_present != UINT8_C(1) || dependency_revision == UINT64_C(0) ||
      dependency_status != PP_DEPENDENCY_SET_CURRENT ||
      dependency_count != UINT64_C(1) ||
      pp_dependency_set_get_dependency(dependencies, UINT64_C(0),
                                       &read_dependency, &error) != PP_OK ||
      read_dependency.kind == NULL ||
      strcmp(read_dependency.kind, "org.postproject:requires") != 0 ||
      read_dependency.target.kind != PP_OBJECT_REPRESENTATION ||
      memcmp(read_dependency.target.id.bytes, proxy_representation_id.bytes,
             sizeof(proxy_representation_id.bytes)) != 0 ||
      read_dependency.required != UINT8_C(1) ||
      read_dependency.authored_reference == NULL ||
      strcmp(read_dependency.authored_reference, "proxy.mov") != 0) {
    pp_dependency_set_release(dependencies);
    pp_production_release(production);
    pp_error_release(error);
    return 75;
  }
  pp_dependency_set_release(dependencies);
  dependencies = NULL;
  const pp_object_ref_t dependency_target = proxy_target;
  pp_dependency_query_set_t *dependent_set = NULL;
  pp_dependency_match_t dependent = {0};
  if (pp_production_dependents(production, &dependency_target, UINT32_C(1),
                               UINT32_C(1000), UINT32_C(1000), NULL,
                               &dependent_set, &error) != PP_OK ||
      dependent_set == NULL ||
      pp_dependency_query_set_count(dependent_set) != UINT64_C(1) ||
      pp_dependency_query_set_get(dependent_set, UINT64_C(0), &dependent,
                                  &error) != PP_OK ||
      dependent.target.kind != PP_OBJECT_REPRESENTATION ||
      memcmp(dependent.target.id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      dependent.depth != UINT32_C(1) ||
      pp_dependency_query_set_next_cursor(dependent_set) != NULL ||
      pp_dependency_query_set_traversal_truncated(dependent_set) != UINT8_C(0)) {
    pp_dependency_query_set_release(dependent_set);
    pp_production_release(production);
    pp_error_release(error);
    return 76;
  }
  pp_dependency_query_set_release(dependent_set);

  pp_dependency_query_set_t *dependency_matches = NULL;
  pp_dependency_match_t dependency_match = {0};
  if (pp_production_dependencies(
          production, representation_id, UINT32_C(2), UINT32_C(1000),
          UINT32_C(1), NULL, &dependency_matches, &error) != PP_OK ||
      dependency_matches == NULL ||
      pp_dependency_query_set_count(dependency_matches) != UINT64_C(1) ||
      pp_dependency_query_set_get(dependency_matches, UINT64_C(0),
                                  &dependency_match, &error) != PP_OK ||
      dependency_match.target.kind != PP_OBJECT_REPRESENTATION ||
      memcmp(dependency_match.target.id.bytes, proxy_representation_id.bytes,
             sizeof(proxy_representation_id.bytes)) != 0 ||
      dependency_match.depth != UINT32_C(1)) {
    pp_dependency_query_set_release(dependency_matches);
    pp_production_release(production);
    pp_error_release(error);
    return 76;
  }
  pp_dependency_query_set_release(dependency_matches);

  pp_job_id_t job_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_request_job(
          transaction, "org.postproject:generate-proxy", &representation_id,
          UINT64_C(1), asset_id, PP_REPRESENTATION_PROXY, NULL, &job_id,
          &error) != PP_OK ||
      job_id_is_zero(job_id) || pp_transaction_commit(transaction, &error) != PP_OK) {
    fprintf(stderr, "job request failed (%u): %s\n", status,
            error != NULL ? pp_error_message(error) : "no details");
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 82;
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  pp_job_set_t *jobs = NULL;
  pp_job_t job = {0};
  pp_representation_id_t job_input_id = {{0}};
  status = pp_production_jobs(production, 0, NULL, UINT32_C(1000), NULL, &jobs,
                              &error);
  if (status != PP_OK || jobs == NULL ||
      pp_job_set_count(jobs) != UINT64_C(1) ||
      pp_job_set_get(jobs, UINT64_C(0), &job, &error) != PP_OK ||
      memcmp(job.id.bytes, job_id.bytes, sizeof(job_id.bytes)) != 0 ||
      job.kind == NULL ||
      strcmp(job.kind, "org.postproject:generate-proxy") != 0 ||
      memcmp(job.output_asset_id.bytes, asset_id.bytes, sizeof(asset_id.bytes)) !=
          0 ||
      job.output_representation_kind != PP_REPRESENTATION_PROXY ||
      job.target_root != NULL || job.state != PP_JOB_REQUESTED ||
      job.input_count != UINT64_C(1) ||
      pp_job_set_get_input(jobs, UINT64_C(0), UINT64_C(0), &job_input_id,
                           &error) != PP_OK ||
      memcmp(job_input_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 83;
  }
  pp_job_set_release(jobs);
  jobs = NULL;

  status = pp_production_job(production, job_id, &jobs, &error);
  if (status != PP_OK || jobs == NULL ||
      pp_job_set_count(jobs) != UINT64_C(1) ||
      pp_job_set_get(jobs, UINT64_C(0), &job, &error) != PP_OK ||
      memcmp(job.id.bytes, job_id.bytes, sizeof(job_id.bytes)) != 0 ||
      job.state != PP_JOB_REQUESTED || pp_job_set_next_cursor(jobs) != NULL) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 114;
  }
  pp_job_set_release(jobs);

  pp_uuid_t claim_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_claim_job(
          transaction, job_id, "C worker", "1.0", NULL, "operator", NULL,
          NULL, NULL, INT64_C(10), INT64_C(20), &claim_id, &error) != PP_OK ||
      uuid_is_zero(&claim_id) ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 84;
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  jobs = NULL;
  memset(&job, 0, sizeof(job));
  if (pp_production_jobs(production, 0, NULL, UINT32_C(1000), NULL, &jobs,
                         &error) != PP_OK ||
      jobs == NULL ||
      pp_job_set_get(jobs, UINT64_C(0), &job, &error) != PP_OK ||
      job.state != PP_JOB_CLAIMED ||
      memcmp(job.claim_id.bytes, claim_id.bytes, sizeof(claim_id.bytes)) != 0 ||
      job.claim_expires_at_unix_micros != INT64_C(20) ||
      job.claim_tool_name == NULL || strcmp(job.claim_tool_name, "C worker") != 0 ||
      job.claim_tool_version == NULL || strcmp(job.claim_tool_version, "1.0") != 0 ||
      job.claim_agent_name == NULL || strcmp(job.claim_agent_name, "operator") != 0) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 85;
  }
  pp_job_set_release(jobs);
  jobs = NULL;

  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_renew_job_claim(transaction, job_id, &claim_id,
                                     INT64_C(11), INT64_C(30), &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 86;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_release_job_claim(transaction, job_id, &claim_id,
                                       &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 87;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  pp_uuid_t second_claim_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_claim_job(
          transaction, job_id, "C worker", NULL, NULL, NULL, NULL, NULL,
          NULL, INT64_C(31), INT64_C(40), &second_claim_id, &error) != PP_OK ||
      uuid_is_zero(&second_claim_id) ||
      memcmp(second_claim_id.bytes, claim_id.bytes, sizeof(claim_id.bytes)) == 0 ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 88;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_fail_job(transaction, job_id, &second_claim_id,
                              INT64_C(32), "encoder exited", &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 89;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  pp_job_id_t cancelled_job_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_request_job(
          transaction, "org.postproject:generate-thumbnail", &representation_id,
          UINT64_C(1), asset_id, PP_REPRESENTATION_DERIVED, NULL,
          &cancelled_job_id, &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 90;
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_cancel_job(transaction, cancelled_job_id, &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 91;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  pp_job_t other_job = {0};
  jobs = NULL;
  memset(&job, 0, sizeof(job));
  if (pp_production_jobs(production, 0, NULL, UINT32_C(1000), NULL, &jobs,
                         &error) != PP_OK ||
      jobs == NULL ||
      pp_job_set_count(jobs) != UINT64_C(2) ||
      pp_job_set_get(jobs, UINT64_C(0), &job, &error) != PP_OK ||
      pp_job_set_get(jobs, UINT64_C(1), &other_job, &error) != PP_OK) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 92;
  }
  const pp_job_t *failed_job =
      memcmp(job.id.bytes, job_id.bytes, sizeof(job_id.bytes)) == 0 ? &job
                                                                   : &other_job;
  const pp_job_t *cancelled_job = failed_job == &job ? &other_job : &job;
  if (failed_job->state != PP_JOB_FAILED ||
      failed_job->failure_diagnostic == NULL ||
      strcmp(failed_job->failure_diagnostic, "encoder exited") != 0 ||
      !uuid_is_zero(&failed_job->claim_id) ||
      memcmp(cancelled_job->id.bytes, cancelled_job_id.bytes,
             sizeof(cancelled_job_id.bytes)) != 0 ||
      cancelled_job->state != PP_JOB_CANCELLED) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 93;
  }
  pp_job_set_release(jobs);

  pp_job_id_t completed_job_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_request_job(
          transaction, "org.postproject:generate-proxy", &representation_id,
          UINT64_C(1), asset_id, PP_REPRESENTATION_PROXY, NULL,
          &completed_job_id, &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 94;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  pp_uuid_t completion_claim_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status != PP_OK ||
      pp_transaction_claim_job(
          transaction, completed_job_id, "C worker", NULL, NULL, NULL, NULL,
          NULL, NULL, INT64_C(41), INT64_C(50), &completion_claim_id,
          &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 95;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  pp_representation_id_t completed_representation_id = {{0}};
  pp_activity_id_t completion_activity_id = {{0}};
  status = pp_production_begin_transaction(production, &transaction, &error);
  if (status == PP_OK) {
    status = add_file(transaction, &asset_id, PP_REPRESENTATION_PROXY,
                      moved_media_path, &completed_representation_id, &error);
  }
  const pp_activity_edge_t completion_input = {
      representation_id, "org.postproject:input.primary-video"};
  const pp_activity_edge_t completion_output = {
      completed_representation_id, "org.postproject:output.proxy"};
  if (status == PP_OK) {
    status = pp_transaction_create_activity(
        transaction, "org.postproject:transcode", &completion_input,
        UINT64_C(1), &completion_output, UINT64_C(1), NULL, NULL, "C worker",
        NULL, NULL, NULL, NULL, NULL, NULL, &completion_activity_id, &error);
  }
  if (status == PP_OK) {
    status = pp_transaction_complete_job(
        transaction, completed_job_id, &completion_claim_id, INT64_C(42),
        completed_representation_id, completion_activity_id, &error);
  }
  if (status != PP_OK || representation_id_is_zero(&completed_representation_id) ||
      activity_id_is_zero(completion_activity_id) ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 96;
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  jobs = NULL;
  pp_job_t completed_job = {0};
  uint8_t found_completed_job = UINT8_C(0);
  if (pp_production_jobs(production, 0, NULL, UINT32_C(1000), NULL, &jobs,
                         &error) != PP_OK ||
      jobs == NULL ||
      pp_job_set_count(jobs) != UINT64_C(3)) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 97;
  }
  for (uint64_t index = 0; index < pp_job_set_count(jobs); ++index) {
    pp_job_t candidate = {0};
    if (pp_job_set_get(jobs, index, &candidate, &error) != PP_OK) {
      pp_job_set_release(jobs);
      pp_production_release(production);
      pp_error_release(error);
      return 97;
    }
    if (memcmp(candidate.id.bytes, completed_job_id.bytes,
               sizeof(completed_job_id.bytes)) == 0) {
      completed_job = candidate;
      found_completed_job = UINT8_C(1);
    }
  }
  if (found_completed_job != UINT8_C(1) ||
      completed_job.state != PP_JOB_SUCCEEDED ||
      memcmp(completed_job.completion_representation_id.bytes,
             completed_representation_id.bytes,
             sizeof(completed_representation_id.bytes)) != 0 ||
      memcmp(completed_job.completion_activity_id.bytes,
             completion_activity_id.bytes, sizeof(completion_activity_id.bytes)) !=
          0) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 98;
  }
  pp_job_set_release(jobs);
  jobs = NULL;

  activities = NULL;
  if (pp_production_activities_producing(
          production, completed_representation_id, &activities, &error) != PP_OK ||
      activities == NULL || pp_activity_set_count(activities) != UINT64_C(1) ||
      pp_activity_set_get_output_snapshot(
          activities, UINT64_C(0), UINT64_C(0), &has_output_snapshot,
          &output_snapshot_revision, &output_snapshot_fingerprint_count,
          &error) != PP_OK ||
      has_output_snapshot != UINT8_C(1) || output_snapshot_revision == 0 ||
      output_snapshot_fingerprint_count == 0) {
    pp_activity_set_release(activities);
    pp_production_release(production);
    pp_error_release(error);
    return 99;
  }
  pp_activity_set_release(activities);

  const pp_representation_id_t planned_artifacts[2] = {representation_id, representation_id};
  pp_regeneration_plan_set_t *plans = NULL;
  pp_representation_id_t planned_artifact_id = {{0}};
  pp_job_set_t *planned_job_set = NULL;
  pp_metadata_set_t *planned_parameters = NULL;
  pp_job_t planned_job = {0};
  status = pp_production_plan_regeneration(
      production, planned_artifacts, UINT64_C(2), &plans, &error);
  if (status != PP_OK || plans == NULL ||
      pp_regeneration_plan_set_count(plans) != UINT64_C(1) ||
      pp_regeneration_plan_set_get(
          plans, UINT64_C(0), &planned_artifact_id, &planned_job_set,
          &planned_parameters, &error) != PP_OK ||
      planned_job_set == NULL || planned_parameters == NULL ||
      memcmp(planned_artifact_id.bytes, representation_id.bytes,
             sizeof(representation_id.bytes)) != 0 ||
      pp_job_set_count(planned_job_set) != UINT64_C(1) ||
      pp_job_set_get(planned_job_set, UINT64_C(0), &planned_job, &error) != PP_OK ||
      planned_job.kind == NULL ||
      strcmp(planned_job.kind, "org.postproject:ingest") != 0 ||
      planned_job.state != PP_JOB_REQUESTED || planned_job.input_count != 0 ||
      memcmp(planned_job.output_asset_id.bytes, asset_id.bytes,
             sizeof(asset_id.bytes)) != 0 ||
      planned_job.output_representation_kind != PP_REPRESENTATION_ORIGINAL ||
      pp_metadata_set_count(planned_parameters) != UINT64_C(1)) {
    pp_metadata_set_release(planned_parameters);
    pp_job_set_release(planned_job_set);
    pp_regeneration_plan_set_release(plans);
    pp_production_release(production);
    pp_error_release(error);
    return 100;
  }
  metadata_value = NULL;
  if (pp_metadata_set_get(planned_parameters, UINT64_C(0), &metadata_target,
                          &vocabulary, &property, &metadata_value,
                          &error) != PP_OK ||
      metadata_target.kind != PP_OBJECT_JOB ||
      memcmp(metadata_target.id.bytes, planned_job.id.bytes,
             sizeof(planned_job.id.bytes)) != 0 ||
      vocabulary == NULL || strcmp(vocabulary, "com.example.ingest") != 0 ||
      property == NULL || strcmp(property, "rate") != 0 ||
      pp_metadata_value_get_rational(metadata_value, &rate_numerator,
                                     &rate_denominator, &error) != PP_OK ||
      rate_numerator != INT64_C(24000) || rate_denominator != UINT64_C(1001)) {
    pp_metadata_set_release(planned_parameters);
    pp_job_set_release(planned_job_set);
    pp_regeneration_plan_set_release(plans);
    pp_production_release(production);
    pp_error_release(error);
    return 101;
  }
  pp_metadata_set_release(planned_parameters);
  pp_job_set_release(planned_job_set);
  pp_regeneration_plan_set_release(plans);
  jobs = NULL;
  if (pp_production_jobs(production, 0, NULL, UINT32_C(1000), NULL, &jobs,
                         &error) != PP_OK ||
      jobs == NULL ||
      pp_job_set_count(jobs) != UINT64_C(3)) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 102;
  }
  pp_job_set_release(jobs);
  jobs = NULL;
  if (pp_production_jobs(production, 0, NULL, UINT32_C(1), NULL, &jobs,
                         &error) != PP_OK ||
      jobs == NULL || pp_job_set_count(jobs) != UINT64_C(1) ||
      pp_job_set_next_cursor(jobs) == NULL) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 102;
  }
  char job_cursor[2049] = {0};
  if (snprintf(job_cursor, sizeof(job_cursor), "%s",
               pp_job_set_next_cursor(jobs)) < 0) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 102;
  }
  pp_job_set_release(jobs);
  jobs = NULL;
  if (pp_production_jobs(production, 0, NULL, UINT32_C(1), job_cursor, &jobs,
                         &error) != PP_OK ||
      jobs == NULL || pp_job_set_count(jobs) != UINT64_C(1)) {
    pp_job_set_release(jobs);
    pp_production_release(production);
    pp_error_release(error);
    return 102;
  }
  pp_job_set_release(jobs);

  const pp_revision_event_kind_t imported_kinds[] = {
      PP_REVISION_ASSET_IMPORTED, PP_REVISION_ASSET_IMPORTED};
  uint64_t through_sequence = 0;
  revisions = NULL;
  if (pp_production_changes_since_filtered(production, 0, imported_kinds, 2,
                                           UINT32_C(1), &revisions,
                                           &through_sequence,
                                           &error) != PP_OK ||
      revisions == NULL || pp_revision_set_count(revisions) != UINT64_C(1) ||
      through_sequence != UINT64_C(1)) {
    pp_revision_set_release(revisions);
    pp_production_release(production);
    pp_error_release(error);
    return 103;
  }
  pp_revision_set_release(revisions);
  revisions = NULL;
  const pp_revision_event_kind_t unknown_kind = UINT32_C(999);
  status = pp_production_changes_since_filtered(production, 0, &unknown_kind,
                                                1, UINT32_C(1), &revisions,
                                                &through_sequence, &error);
  if (status != PP_ERROR_INVALID_ARGUMENT || revisions != NULL) {
    pp_revision_set_release(revisions);
    pp_production_release(production);
    pp_error_release(error);
    return 104;
  }
  pp_error_release(error);
  error = NULL;

  pp_revision_waiter_t *waiter = NULL;
  pp_revision_wait_result_t wait_result = 0;
  if (pp_revision_waiter_create(production, &waiter, &error) != PP_OK ||
      waiter == NULL ||
      pp_revision_waiter_wait(waiter, 0, UINT32_C(2), 0, &wait_result,
                              &revisions, &error) != PP_OK ||
      wait_result != PP_REVISION_WAIT_REVISIONS || revisions == NULL ||
      pp_revision_set_count(revisions) != UINT64_C(2)) {
    pp_revision_set_release(revisions);
    pp_revision_waiter_release(waiter);
    pp_production_release(production);
    pp_error_release(error);
    return 105;
  }
  pp_revision_set_release(revisions);
  revisions = NULL;
  if (pp_revision_waiter_wait(waiter, UINT64_MAX, UINT32_C(1), 0,
                              &wait_result, &revisions, &error) != PP_OK ||
      wait_result != PP_REVISION_WAIT_TIMED_OUT || revisions == NULL ||
      pp_revision_set_count(revisions) != 0) {
    pp_revision_set_release(revisions);
    pp_revision_waiter_release(waiter);
    pp_production_release(production);
    pp_error_release(error);
    return 106;
  }
  pp_revision_set_release(revisions);
  revisions = NULL;
  pp_revision_waiter_cancel(waiter);
  if (pp_revision_waiter_wait(waiter, UINT64_MAX, UINT32_C(1),
                              PP_REVISION_WAIT_MAX_TIMEOUT_MILLIS,
                              &wait_result, &revisions, &error) != PP_OK ||
      wait_result != PP_REVISION_WAIT_CANCELLED) {
    pp_revision_set_release(revisions);
    pp_revision_waiter_release(waiter);
    pp_production_release(production);
    pp_error_release(error);
    return 107;
  }
  pp_revision_set_release(revisions);
  revisions = NULL;
  pp_revision_waiter_release(waiter);
  waiter = NULL;
  if (pp_revision_waiter_create(production, &waiter, &error) != PP_OK) {
    pp_production_release(production);
    pp_error_release(error);
    return 108;
  }
  pp_production_release(production);
  production = NULL;
  if (pp_revision_waiter_wait(waiter, UINT64_MAX, UINT32_C(1),
                              PP_REVISION_WAIT_MAX_TIMEOUT_MILLIS,
                              &wait_result, &revisions, &error) != PP_OK ||
      wait_result != PP_REVISION_WAIT_CLOSED) {
    pp_revision_set_release(revisions);
    pp_revision_waiter_release(waiter);
    pp_error_release(error);
    return 109;
  }
  pp_revision_set_release(revisions);
  pp_revision_waiter_release(waiter);
  pp_revision_waiter_cancel(NULL);
  pp_revision_waiter_release(NULL);

  production = NULL;
  if (pp_production_open(argv[1], &production, &error) != PP_OK) {
    pp_error_release(error);
    return 140;
  }
  revisions = NULL;
  if (pp_production_latest_revision(production, &revisions, &error) != PP_OK ||
      pp_revision_set_get(revisions, UINT64_C(0), &revision_id,
                          &revision_sequence, &revision_transaction_id,
                          &revision_committed_at, &revision_origin_name,
                          &revision_origin_version, &revision_origin_uri,
                          &revision_message, &error) != PP_OK) {
    pp_revision_set_release(revisions);
    pp_production_release(production);
    pp_error_release(error);
    return 141;
  }
  pp_revision_set_release(revisions);
  revisions = NULL;
  if (pp_production_begin_transaction_at(production, revision_id, &transaction,
                                         &error) != PP_OK ||
      pp_transaction_confirm_locator(transaction, resource_id,
                                     "file:///c-smoke/conflict-first.mov", NULL,
                                     NULL, &error) != PP_OK ||
      pp_transaction_commit(transaction, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 142;
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  if (pp_production_begin_transaction_at(production, revision_id, &transaction,
                                         &error) != PP_OK ||
      pp_transaction_confirm_locator(
          transaction, resource_id, "file:///c-smoke/conflict-stale.mov",
          NULL, NULL, &error) != PP_OK) {
    pp_transaction_release(transaction);
    pp_production_release(production);
    pp_error_release(error);
    return 143;
  }
  status = pp_transaction_commit(transaction, &error);
  pp_transaction_release(transaction);
  transaction = NULL;
  pp_transaction_conflict_t conflict = {0};
  if (status != PP_ERROR_CONFLICT || error == NULL ||
      pp_error_transaction_conflict(error, &conflict) != UINT8_C(1) ||
      conflict.kind != PP_CONFLICT_LOCATOR_SET ||
      conflict.target.kind != PP_OBJECT_RESOURCE ||
      memcmp(conflict.target.id.bytes, resource_id.bytes,
             sizeof(resource_id.bytes)) != 0 ||
      memcmp(conflict.base_revision_id.bytes, revision_id.bytes,
             sizeof(revision_id.bytes)) != 0 ||
      conflict.base_revision_sequence != revision_sequence ||
      conflict.superseding_revision_sequence <= revision_sequence) {
    pp_production_release(production);
    pp_error_release(error);
    return 144;
  }
  pp_error_release(error);
  error = NULL;
  pp_production_release(production);
  production = NULL;

  status = pp_production_open(NULL, &production, &error);
  if (status != PP_ERROR_INVALID_ARGUMENT || error == NULL ||
      pp_error_code(error) != status || pp_error_message(error) == NULL) {
    pp_error_release(error);
    return 5;
  }
  pp_error_release(error);
  error = NULL;

  char renamed_path[4096];
  int renamed_length =
      snprintf(renamed_path, sizeof(renamed_path), "%s.renamed", argv[1]);
  if (renamed_length < 0 || (size_t)renamed_length >= sizeof(renamed_path)) {
    return 139;
  }
  (void)remove(renamed_path);
  production = NULL;
  if (pp_production_create(renamed_path, "Renamed sequence", &production,
                           &error) != PP_OK) {
    pp_error_release(error);
    return 139;
  }
  int renamed_status = renamed_sequence_scenario(production, argv[2]);
  pp_production_release(production);
  if (renamed_status != 0) {
    return renamed_status;
  }
  remove(moved_media_path);
  remove(sequence_frame_path);
  pp_production_release(NULL);
  pp_transaction_release(NULL);
  return 0;
}
