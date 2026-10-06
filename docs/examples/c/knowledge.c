/* Runs the C listings for external identifiers and typed metadata.
 *
 * Each "[name]" ... "[/name]" region is included verbatim by the documentation
 * build, so keep regions self-contained and readable. Usage:
 *   postproject-c-knowledge WORK_DIRECTORY
 * The work directory is prepared by prepare-workdir.cmake.
 */
#include <postproject/postproject.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* [known-media-adoption] */
static pp_error_code_t find_known_media(pp_production_t *production,
                                        const char *media_path,
                                        const pp_asset_id_t *expected_asset,
                                        pp_error_t **error) {
  char *uri = NULL;
  pp_fingerprint_t *fingerprint = NULL;
  pp_known_media_set_t *by_locator = NULL;
  pp_known_media_set_t *by_content = NULL;
  const char *algorithm = NULL;
  const uint8_t *value = NULL;
  uint16_t version = 0;
  uint64_t value_length = 0;
  pp_asset_id_t asset;
  pp_representation_id_t representation;
  pp_uuid_t resource;

  pp_error_code_t status = pp_file_path_to_locator(media_path, &uri, error);
  if (status == PP_OK) {
    status = pp_production_find_known_media_by_locator(
        production, uri, NULL, 100, NULL, &by_locator, error);
  }
  if (status == PP_OK &&
      (pp_known_media_set_count(by_locator) != 1 ||
       pp_known_media_set_next_cursor(by_locator) != NULL)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = pp_known_media_set_get(by_locator, 0, &asset, &representation,
                                    &resource, error);
  }
  if (status == PP_OK && memcmp(&asset, expected_asset, sizeof asset) != 0) {
    status = PP_ERROR_INTERNAL;
  }

  if (status == PP_OK) {
    status = pp_fingerprint_file(media_path, &fingerprint, error);
  }
  if (status == PP_OK) {
    status = pp_fingerprint_get(fingerprint, &algorithm, &version, &value,
                                &value_length, error);
  }
  if (status == PP_OK) {
    status = pp_production_find_known_media_by_fingerprint(
        production, algorithm, version, value, value_length, 100, NULL,
        &by_content, error);
  }
  if (status == PP_OK && pp_known_media_set_count(by_content) == 1) {
    status = pp_known_media_set_get(by_content, 0, &asset, &representation,
                                    &resource, error);
  } else if (status == PP_OK) {
    /* Multiple matches are candidates; the host must never guess one. */
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK && memcmp(&asset, expected_asset, sizeof asset) != 0) {
    status = PP_ERROR_INTERNAL;
  }

  pp_string_release(uri);
  pp_fingerprint_release(fingerprint);
  pp_known_media_set_release(by_locator);
  pp_known_media_set_release(by_content);
  return status;
}
/* [/known-media-adoption] */

/* [remove-identifier] */
static pp_error_code_t replace_identifiers(pp_production_t *production,
                                           const pp_asset_id_t *asset_id,
                                           uint64_t *out_remaining,
                                           pp_error_t **error) {
  pp_object_ref_t target;
  pp_error_code_t reference_status = pp_object_ref_from_asset(*asset_id, &target, error);
  if (reference_status != PP_OK) return reference_status;
  pp_transaction_t *transaction = NULL;
  pp_external_identifier_set_t *identifiers = NULL;
  pp_object_ref_set_t *matches = NULL;

  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_add_external_identifier(transaction, &target,
                                                    "com.example.camera.serial",
                                                    "A-0007", NULL, error);
  }
  if (status == PP_OK) {
    /* Unknown schemes and qualifiers round-trip verbatim. */
    status = pp_transaction_add_external_identifier(
        transaction, &target, "com.example.mam.id", "MAM-42", "staging", error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  if (status == PP_OK) {
    status = pp_production_external_identifiers(production, &target,
                                                &identifiers, error);
  }
  for (uint64_t i = 0;
       status == PP_OK && i < pp_external_identifier_set_count(identifiers);
       ++i) {
    const char *scheme, *value, *qualifier;
    status = pp_external_identifier_set_get(identifiers, i, &scheme, &value,
                                            &qualifier, error);
    if (status == PP_OK) {
      printf("%s = %s%s%s\n", scheme, value, qualifier != NULL ? " / " : "",
             qualifier != NULL ? qualifier : "");
    }
  }
  if (status == PP_OK) {
    status = pp_production_find_by_external_identifier(
        production, "com.example.mam.id", "MAM-42", "staging", &matches, error);
  }
  for (uint64_t i = 0; status == PP_OK && i < pp_object_ref_set_count(matches);
       ++i) {
    pp_object_ref_t object;
    status = pp_object_ref_set_get(matches, i, &object, error);
    if (status == PP_OK) {
      printf("found object of kind %u\n", object.kind);
    }
  }

  /* Removal names the exact scheme, value, and qualifier. */
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_remove_external_identifier(
        transaction, &target, "com.example.mam.id", "MAM-42", "staging", error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_external_identifier_set_release(identifiers);
  identifiers = NULL;
  if (status == PP_OK) {
    status = pp_production_external_identifiers(production, &target,
                                                &identifiers, error);
  }
  if (status == PP_OK) {
    *out_remaining = pp_external_identifier_set_count(identifiers);
  }

  pp_external_identifier_set_release(identifiers);
  pp_object_ref_set_release(matches);
  pp_transaction_release(transaction);
  return status;
}
/* [/remove-identifier] */

/* [typed-metadata] */
static const char *const EDITORIAL = "https://example.com/ns/editorial/1";

enum { VALUE_COUNT = 13 };

static pp_error_code_t add_typed_values(pp_production_t *production,
                                        const pp_asset_id_t *asset_id,
                                        const pp_representation_id_t *representation_id,
                                        pp_error_t **error) {
  static const uint8_t thumbnail[] = {0x89, 0x50, 0x4e, 0x47};
  pp_object_ref_t target;
  pp_error_code_t reference_status = pp_object_ref_from_asset(*asset_id, &target, error);
  if (reference_status != PP_OK) return reference_status;
  pp_object_ref_t source;
  reference_status = pp_object_ref_from_representation(*representation_id, &source, error);
  if (reference_status != PP_OK) return reference_status;
  const char *const properties[VALUE_COUNT] = {
      "slate",    "title",       "offset",   "frame-count", "gain",
      "approved", "reviewed-at", "homepage", "thumbnail",   "frame-rate",
      "keywords", "camera",      "source"};
  pp_metadata_input_t *values[VALUE_COUNT] = {NULL};
  pp_metadata_input_t *keyword[2] = {NULL, NULL};
  pp_metadata_input_t *field[2] = {NULL, NULL};
  pp_transaction_t *transaction = NULL;

  pp_error_code_t status =
      pp_metadata_input_create_string("A001 T3", NULL, &values[0], error);
  if (status == PP_OK) {
    /* A language tag makes this a language string. */
    status = pp_metadata_input_create_string("Interview", "en-US", &values[1],
                                             error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_i64(-12, &values[2], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_u64(2400, &values[3], error);
  }
  if (status == PP_OK) {
    /* Decimals are exact: coefficient 325 with scale 2 is 3.25. */
    status = pp_metadata_input_create_decimal("325", 2, &values[4], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_bool(1, &values[5], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_timestamp(INT64_C(1767225600000000),
                                                &values[6], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_uri("https://example.com/interviews/3",
                                          &values[7], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_bytes(thumbnail, sizeof thumbnail,
                                            &values[8], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_rational(24000, 1001, &values[9], error);
  }
  if (status == PP_OK) {
    status =
        pp_metadata_input_create_string("interview", NULL, &keyword[0], error);
  }
  if (status == PP_OK) {
    status =
        pp_metadata_input_create_string("exterior", NULL, &keyword[1], error);
  }
  if (status == PP_OK) {
    /* Collections copy their children, which stay owned by the caller. */
    const pp_metadata_input_t *items[] = {keyword[0], keyword[1]};
    status = pp_metadata_input_create_list(items, 2, &values[10], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_string("ARRI", NULL, &field[0], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_u64(800, &field[1], error);
  }
  if (status == PP_OK) {
    const char *names[] = {"make", "iso"};
    const pp_metadata_input_t *fields[] = {field[0], field[1]};
    status =
        pp_metadata_input_create_struct(names, fields, 2, &values[11], error);
  }
  if (status == PP_OK) {
    status = pp_metadata_input_create_reference(&source, &values[12], error);
  }
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  for (int i = 0; status == PP_OK && i < VALUE_COUNT; ++i) {
    status = pp_transaction_add_metadata_value(transaction, &target, EDITORIAL,
                                               properties[i], values[i], error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }

  pp_transaction_release(transaction);
  for (int i = 0; i < 2; ++i) {
    pp_metadata_input_release(field[i]);
    pp_metadata_input_release(keyword[i]);
  }
  for (int i = 0; i < VALUE_COUNT; ++i) {
    pp_metadata_input_release(values[i]);
  }
  return status;
}

static pp_error_code_t print_value(const pp_metadata_value_t *value,
                                   uint32_t *seen_kinds, pp_error_t **error) {
  const pp_metadata_value_kind_t kind = pp_metadata_value_kind(value);
  pp_error_code_t status = PP_OK;
  *seen_kinds |= kind < 32 ? UINT32_C(1) << kind : 0;
  switch (kind) {
  case PP_METADATA_STRING:
  case PP_METADATA_LANG_STRING: {
    const char *text, *language;
    status = pp_metadata_value_get_string(value, &text, &language, error);
    if (status == PP_OK) {
      printf("\"%s\"%s%s", text, language != NULL ? "@" : "",
             language != NULL ? language : "");
    }
    break;
  }
  case PP_METADATA_I64: {
    int64_t number = 0;
    status = pp_metadata_value_get_i64(value, &number, error);
    if (status == PP_OK) {
      printf("%lld", (long long)number);
    }
    break;
  }
  case PP_METADATA_U64: {
    uint64_t number = 0;
    status = pp_metadata_value_get_u64(value, &number, error);
    if (status == PP_OK) {
      printf("%llu", (unsigned long long)number);
    }
    break;
  }
  case PP_METADATA_DECIMAL: {
    const char *coefficient;
    uint32_t scale = 0;
    status = pp_metadata_value_get_decimal(value, &coefficient, &scale, error);
    if (status == PP_OK) {
      printf("%se-%u", coefficient, scale);
    }
    break;
  }
  case PP_METADATA_BOOL: {
    uint8_t flag = 0;
    status = pp_metadata_value_get_bool(value, &flag, error);
    if (status == PP_OK) {
      printf("%s", flag ? "true" : "false");
    }
    break;
  }
  case PP_METADATA_TIMESTAMP: {
    int64_t micros = 0;
    status = pp_metadata_value_get_timestamp(value, &micros, error);
    if (status == PP_OK) {
      printf("%lld us since 1970", (long long)micros);
    }
    break;
  }
  case PP_METADATA_URI: {
    const char *uri;
    status = pp_metadata_value_get_uri(value, &uri, error);
    if (status == PP_OK) {
      printf("<%s>", uri);
    }
    break;
  }
  case PP_METADATA_BYTES: {
    const uint8_t *bytes;
    uint64_t length = 0;
    status = pp_metadata_value_get_bytes(value, &bytes, &length, error);
    if (status == PP_OK) {
      printf("%llu bytes", (unsigned long long)length);
    }
    break;
  }
  case PP_METADATA_RATIONAL: {
    int64_t numerator = 0;
    uint64_t denominator = 0;
    status =
        pp_metadata_value_get_rational(value, &numerator, &denominator, error);
    if (status == PP_OK) {
      printf("%lld/%llu", (long long)numerator,
             (unsigned long long)denominator);
    }
    break;
  }
  case PP_METADATA_LIST: {
    printf("[");
    for (uint64_t i = 0;
         status == PP_OK && i < pp_metadata_value_list_count(value); ++i) {
      const pp_metadata_value_t *item;
      status = pp_metadata_value_list_get(value, i, &item, error);
      if (status == PP_OK) {
        printf("%s", i > 0 ? ", " : "");
        status = print_value(item, seen_kinds, error);
      }
    }
    printf("]");
    break;
  }
  case PP_METADATA_STRUCT: {
    printf("{");
    for (uint64_t i = 0;
         status == PP_OK && i < pp_metadata_value_struct_count(value); ++i) {
      const char *name;
      const pp_metadata_value_t *field;
      status = pp_metadata_value_struct_get(value, i, &name, &field, error);
      if (status == PP_OK) {
        printf("%s%s: ", i > 0 ? ", " : "", name);
        status = print_value(field, seen_kinds, error);
      }
    }
    printf("}");
    break;
  }
  case PP_METADATA_REFERENCE: {
    pp_object_ref_t reference;
    status = pp_metadata_value_get_reference(value, &reference, error);
    if (status == PP_OK) {
      printf("object of kind %u", reference.kind);
    }
    break;
  }
  default:
    /* Newer libraries may add kinds; skip what this host does not know. */
    printf("(unknown kind %u)", kind);
    break;
  }
  return status;
}

static pp_error_code_t print_typed_values(const pp_production_t *production,
                                          const pp_asset_id_t *asset_id,
                                          uint32_t *out_seen_kinds,
                                          pp_error_t **error) {
  pp_object_ref_t target;
  pp_error_code_t reference_status = pp_object_ref_from_asset(*asset_id, &target, error);
  if (reference_status != PP_OK) return reference_status;
  pp_metadata_set_t *metadata = NULL;
  pp_error_code_t status =
      pp_production_metadata(production, &target, &metadata, error);
  for (uint64_t i = 0; status == PP_OK && i < pp_metadata_set_count(metadata);
       ++i) {
    pp_object_ref_t owner;
    const char *vocabulary, *property;
    const pp_metadata_value_t *value;
    status = pp_metadata_set_get(metadata, i, &owner, &vocabulary, &property,
                                 &value, error);
    if (status == PP_OK && strcmp(vocabulary, EDITORIAL) == 0) {
      printf("%s: ", property);
      status = print_value(value, out_seen_kinds, error);
      printf("\n");
    }
  }
  pp_metadata_set_release(metadata);
  return status;
}

static pp_error_code_t count_values_by_page(const pp_production_t *production,
                                            const char *property,
                                            uint64_t *out_count,
                                            pp_error_t **error) {
  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  pp_error_code_t status = PP_OK;
  *out_count = 0;
  while (status == PP_OK) {
    pp_metadata_set_t *page = NULL;
    /* A NULL exact value matches every value of the property. */
    status = pp_production_query_metadata(production, EDITORIAL, property, NULL,
                                          UINT32_C(1), cursor, &page, error);
    *out_count += status == PP_OK ? pp_metadata_set_count(page) : 0;
    const char *next =
        status == PP_OK ? pp_metadata_set_next_cursor(page) : NULL;
    const int copied =
        next != NULL
            ? snprintf(cursor_storage, sizeof cursor_storage, "%s", next)
            : 0;
    pp_metadata_set_release(page);
    if (status != PP_OK || copied == 0) {
      break;
    }
    if (copied < 0 || (size_t)copied >= sizeof cursor_storage) {
      status = PP_ERROR_INTERNAL;
    }
    cursor = cursor_storage;
  }
  return status;
}
/* [/typed-metadata] */

/* [remove-metadata] */
static pp_error_code_t remove_keywords(pp_production_t *production,
                                       const pp_asset_id_t *asset_id,
                                       uint64_t *out_remaining,
                                       pp_error_t **error) {
  pp_object_ref_t target;
  pp_error_code_t reference_status = pp_object_ref_from_asset(*asset_id, &target, error);
  if (reference_status != PP_OK) return reference_status;
  pp_transaction_t *transaction = NULL;
  pp_read_session_t *view = NULL;
  pp_metadata_set_t *remaining = NULL;
  pp_error_code_t status =
      pp_production_read_session(production, &view, error);
  if (status == PP_OK) {
    status = pp_read_session_begin_edit(view, &transaction, error);
  }
  pp_read_session_release(view);
  if (status == PP_OK) {
    /* Removes every value of the property on this target. */
    status = pp_transaction_remove_metadata_property(
        transaction, &target, EDITORIAL, "keywords", error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_find_metadata(production, EDITORIAL, "keywords",
                                         &remaining, error);
  }
  if (status == PP_OK) {
    *out_remaining = pp_metadata_set_count(remaining);
    printf("keywords left: %llu\n", (unsigned long long)*out_remaining);
  }
  pp_metadata_set_release(remaining);
  pp_transaction_release(transaction);
  return status;
}
/* [/remove-metadata] */

static void join(char *buffer, size_t size, const char *directory,
                 const char *name) {
  snprintf(buffer, size, "%s/%s", directory, name);
}

static pp_error_code_t create_production(const char *path, const char *media,
                                         pp_production_t **out_production,
                                         pp_asset_id_t *out_asset_id,
                                         pp_representation_id_t *out_representation_id,
                                         pp_error_t **error) {
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_media_source_t *camera = NULL;
  pp_representation_set_t *representations = NULL;
  pp_error_code_t status =
      pp_production_create(path, "Documentary", &production, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_media_source_create_file(media, &camera, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_import_media(transaction, camera, "Camera A",
                                         out_asset_id, error);
  }
  pp_media_source_release(camera);
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_representations(production, *out_asset_id,
                                           &representations, error);
  }
  if (status == PP_OK) {
    pp_asset_id_t owner;
    pp_representation_kind_t kind;
    pp_content_structure_kind_t structure;
    uint64_t members, resources, fingerprints;
    status = pp_representation_set_get(
        representations, 0, out_representation_id, &owner, &kind, &structure,
        &members, &resources, &fingerprints, error);
  }
  if (status == PP_OK) {
    *out_production = production;
    production = NULL;
  }
  pp_representation_set_release(representations);
  pp_transaction_release(transaction);
  pp_production_release(production);
  return status;
}

/* Records one "department" value on two objects so a query spans pages. */
static pp_error_code_t add_departments(pp_production_t *production,
                                       const pp_asset_id_t *asset_id,
                                       const pp_representation_id_t *representation_id,
                                       pp_error_t **error) {
  pp_object_ref_t targets[2];
  pp_error_code_t reference_status = pp_object_ref_from_asset(*asset_id, &targets[0], error);
  if (reference_status != PP_OK) return reference_status;
  reference_status = pp_object_ref_from_representation(*representation_id, &targets[1], error);
  if (reference_status != PP_OK) return reference_status;
  pp_metadata_input_t *department = NULL;
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status =
      pp_metadata_input_create_string("editorial", NULL, &department, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  for (int i = 0; status == PP_OK && i < 2; ++i) {
    status = pp_transaction_add_metadata_value(
        transaction, &targets[i], EDITORIAL, "department", department, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  pp_metadata_input_release(department);
  return status;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: postproject-c-knowledge WORK_DIRECTORY\n");
    return 2;
  }
  const char *work = argv[1];
  char production_path[4096];
  char media[4096];
  join(production_path, sizeof production_path, work, "knowledge.pproj");
  join(media, sizeof media, work, "rushes/A001.mov");

  pp_production_t *production = NULL;
  pp_error_t *error = NULL;
  pp_asset_id_t asset_id;
  pp_representation_id_t representation_id;
  uint64_t count = 0;
  uint32_t seen_kinds = 0;

  pp_error_code_t status =
      create_production(production_path, media, &production, &asset_id,
                        &representation_id, &error);
  if (status == PP_OK) {
    status = find_known_media(production, media, &asset_id, &error);
  }
  if (status == PP_OK) {
    status = replace_identifiers(production, &asset_id, &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(1)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status =
        add_typed_values(production, &asset_id, &representation_id, &error);
  }
  if (status == PP_OK) {
    status = print_typed_values(production, &asset_id, &seen_kinds, &error);
  }
  /* Every kind from PP_METADATA_STRING (1) to PP_METADATA_REFERENCE (13). */
  if (status == PP_OK && seen_kinds != UINT32_C(0x3ffe)) {
    fprintf(stderr, "seen kinds: %#x\n", (unsigned)seen_kinds);
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = add_departments(production, &asset_id, &representation_id, &error);
  }
  if (status == PP_OK) {
    status = count_values_by_page(production, "department", &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(2)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = remove_keywords(production, &asset_id, &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(0)) {
    status = PP_ERROR_INTERNAL;
  }

  if (status != PP_OK) {
    fprintf(stderr, "operation failed (%u): %s\n", status,
            error != NULL ? pp_error_message(error) : "no details");
  }
  pp_error_release(error);
  pp_production_release(production);
  return status == PP_OK ? EXIT_SUCCESS : EXIT_FAILURE;
}
