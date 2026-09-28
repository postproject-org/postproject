/* Runs the C listings for representations, locators, and media roots.
 *
 * Each "[name]" ... "[/name]" region is included verbatim by the documentation
 * build, so keep regions self-contained and readable. Usage:
 *   postproject-c-media WORK_DIRECTORY
 * The work directory is prepared by prepare-workdir.cmake.
 */
#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif

#include <postproject/postproject.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <direct.h>
#define make_directory(path) _mkdir(path)
#else
#include <sys/stat.h>
#define make_directory(path) mkdir(path, 0777)
#endif

/* [import-sequence] */
static pp_error_code_t import_image_strip(pp_production_t *production,
                                          const char *directory,
                                          pp_uuid_t *out_asset_id,
                                          pp_content_structure_kind_t *out_kind,
                                          pp_error_t **error) {
  const int64_t missing_frames[] = {1003};
  pp_media_source_t *strip = NULL;
  pp_transaction_t *transaction = NULL;
  pp_representation_set_t *representations = NULL;
  /* The sequence becomes the new asset's only original representation. */
  pp_error_code_t status = pp_media_source_create_image_sequence(
      directory, "shot010.", ".exr", 4, 1001, 1004, 1, 24, 1, missing_frames,
      1, &strip, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_import_media(transaction, strip, "shot010 strip",
                                         out_asset_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  pp_media_source_release(strip);
  if (status == PP_OK) {
    status = pp_production_representations(production, out_asset_id,
                                           &representations, error);
  }
  if (status == PP_OK) {
    pp_uuid_t representation_id, owner;
    pp_representation_kind_t kind;
    uint64_t members, resources, fingerprints;
    status = pp_representation_set_get(representations, 0, &representation_id,
                                       &owner, &kind, out_kind, &members,
                                       &resources, &fingerprints, error);
    printf("%llu representation, structure %u\n",
           (unsigned long long)pp_representation_set_count(representations),
           status == PP_OK ? *out_kind : 0);
  }
  pp_representation_set_release(representations);
  return status;
}
/* [/import-sequence] */

/* [add-representation] */
static pp_error_code_t add_proxy(pp_production_t *production,
                                 const pp_uuid_t *asset_id,
                                 const char *proxy_path,
                                 pp_uuid_t *out_proxy_id, pp_error_t **error) {
  pp_media_source_t *proxy = NULL;
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status = pp_media_source_create_file(proxy_path, &proxy, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    /* The file is fingerprinted and recorded as one more representation of
     * the same asset; it never becomes an unrelated asset. */
    status = pp_transaction_add_representation(
        transaction, asset_id, PP_REPRESENTATION_PROXY, proxy, out_proxy_id,
        error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  pp_media_source_release(proxy);
  return status;
}
/* [/add-representation] */

/* [ordered-parts] */
static pp_error_code_t
add_spanned_clip(pp_production_t *production, const pp_uuid_t *asset_id,
                 const char *first_part, const char *second_part,
                 pp_uuid_t *out_representation_id, pp_error_t **error) {
  /* Order is significant, and every ordered part must be required. */
  const pp_file_resource_input_t parts[] = {
      {first_part, "org.postproject:span-part", UINT8_C(1)},
      {second_part, "org.postproject:span-part", UINT8_C(1)},
  };
  pp_media_source_t *clip = NULL;
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status =
      pp_media_source_create_ordered_parts(parts, 2, &clip, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_representation(
        transaction, asset_id, PP_REPRESENTATION_OPTIMIZED, clip,
        out_representation_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  pp_media_source_release(clip);
  return status;
}
/* [/ordered-parts] */

/* [package-representation] */
static pp_error_code_t add_package(pp_production_t *production,
                                   const pp_uuid_t *asset_id,
                                   const char *essence, const char *sidecar,
                                   pp_uuid_t *out_representation_id,
                                   pp_error_t **error) {
  /* A package needs at least one required member; an optional sidecar that
   * goes missing produces an issue but does not reduce availability. */
  const pp_file_resource_input_t members[] = {
      {essence, "org.postproject:essence", UINT8_C(1)},
      {sidecar, "org.postproject:sidecar", UINT8_C(0)},
  };
  pp_media_source_t *package = NULL;
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status =
      pp_media_source_create_package(members, 2, &package, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_representation(
        transaction, asset_id, PP_REPRESENTATION_DERIVED, package,
        out_representation_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  pp_media_source_release(package);
  return status;
}
/* [/package-representation] */

/* [representation-structure] */
static void print_hex(const char *label, const char *algorithm,
                      uint16_t version, const uint8_t *value, uint64_t length) {
  printf("    %s %s v%u: ", label, algorithm, version);
  for (uint64_t i = 0; i < length && i < 8; ++i) {
    printf("%02x", value[i]);
  }
  printf("%s\n", length > 8 ? "..." : "");
}

static pp_error_code_t print_resource(const pp_representation_set_t *set,
                                      uint64_t r, uint64_t s,
                                      pp_error_t **error) {
  pp_uuid_t resource_id;
  uint8_t has_file_facts = 0;
  uint64_t size = 0;
  uint8_t has_modified_at = 0;
  int64_t modified_at = 0;
  uint64_t locator_count = 0;
  uint64_t fingerprint_count = 0;
  pp_error_code_t status = pp_representation_set_get_resource(
      set, r, s, &resource_id, &has_file_facts, &size, &has_modified_at,
      &modified_at, &locator_count, &fingerprint_count, error);
  if (status == PP_OK && has_file_facts != 0) {
    printf("  resource %llu: %llu bytes\n", (unsigned long long)s,
           (unsigned long long)size);
  }
  for (uint64_t f = 0; status == PP_OK && f < fingerprint_count; ++f) {
    const char *algorithm = NULL;
    uint16_t version = 0;
    const uint8_t *value = NULL;
    uint64_t length = 0;
    status = pp_representation_set_get_resource_fingerprint(
        set, r, s, f, &algorithm, &version, &value, &length, error);
    if (status == PP_OK) {
      print_hex("resource fingerprint", algorithm, version, value, length);
    }
  }
  for (uint64_t l = 0; status == PP_OK && l < locator_count; ++l) {
    pp_uuid_t locator_id;
    const char *uri = NULL;
    pp_locator_availability_t availability;
    uint8_t has_last_seen = 0;
    int64_t last_seen = 0;
    status = pp_representation_set_get_locator(set, r, s, l, &locator_id, &uri,
                                               &availability, &has_last_seen,
                                               &last_seen, error);
    if (status == PP_OK) {
      printf("    locator: %s (availability %u)\n", uri, availability);
    }
  }
  return status;
}

static pp_error_code_t print_representation(const pp_representation_set_t *set,
                                            uint64_t r,
                                            int64_t *out_missing_frame,
                                            pp_error_t **error) {
  pp_uuid_t id;
  pp_uuid_t asset_id;
  pp_representation_kind_t kind;
  pp_content_structure_kind_t structure;
  uint64_t members = 0;
  uint64_t resources = 0;
  uint64_t fingerprints = 0;
  pp_error_code_t status =
      pp_representation_set_get(set, r, &id, &asset_id, &kind, &structure,
                                &members, &resources, &fingerprints, error);
  if (status == PP_OK) {
    printf("representation kind %u, structure %u\n", kind, structure);
  }
  /* Members are in structural order; single files and sequences have no role.
   */
  for (uint64_t m = 0; status == PP_OK && m < members; ++m) {
    pp_uuid_t resource_id;
    const char *role = NULL;
    uint8_t required = 0;
    status = pp_representation_set_get_member(set, r, m, &resource_id, &role,
                                              &required, error);
    if (status == PP_OK) {
      printf("  member %llu: %s%s\n", (unsigned long long)m,
             role != NULL ? role : "(no role)", required ? "" : " (optional)");
    }
  }
  for (uint64_t s = 0; status == PP_OK && s < resources; ++s) {
    status = print_resource(set, r, s, error);
  }
  /* Representation fingerprints are distinct from resource fingerprints. */
  for (uint64_t f = 0; status == PP_OK && f < fingerprints; ++f) {
    const char *algorithm = NULL;
    uint16_t version = 0;
    const uint8_t *value = NULL;
    uint64_t length = 0;
    status = pp_representation_set_get_fingerprint(
        set, r, f, &algorithm, &version, &value, &length, error);
    if (status == PP_OK) {
      print_hex("representation fingerprint", algorithm, version, value,
                length);
    }
  }
  if (status == PP_OK && structure == PP_CONTENT_IMAGE_SEQUENCE) {
    const char *prefix, *suffix;
    uint8_t padding = 0;
    int64_t start = 0, end = 0;
    uint32_t step = 0, rate_numerator = 0, rate_denominator = 0;
    uint64_t missing_count = 0;
    status = pp_representation_set_get_sequence(
        set, r, &prefix, &suffix, &padding, &start, &end, &step,
        &rate_numerator, &rate_denominator, &missing_count, error);
    for (uint64_t i = 0; status == PP_OK && i < missing_count; ++i) {
      status = pp_representation_set_get_sequence_missing_frame(
          set, r, i, out_missing_frame, error);
      if (status == PP_OK) {
        printf("  known missing frame: %lld\n", (long long)*out_missing_frame);
      }
    }
  }
  return status;
}

static pp_error_code_t print_asset_structure(const pp_production_t *production,
                                             const pp_uuid_t *asset_id,
                                             uint64_t *out_count,
                                             int64_t *out_missing_frame,
                                             pp_error_t **error) {
  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  pp_error_code_t status = PP_OK;
  *out_count = 0;
  while (status == PP_OK) {
    pp_representation_set_t *page = NULL;
    status = pp_production_representations_page(
        production, asset_id, UINT32_C(2), cursor, &page, error);
    const uint64_t count =
        status == PP_OK ? pp_representation_set_count(page) : 0;
    for (uint64_t r = 0; status == PP_OK && r < count; ++r) {
      status = print_representation(page, r, out_missing_frame, error);
    }
    *out_count += count;
    /* The cursor borrows the page; copy it before releasing the page. */
    const char *next =
        status == PP_OK ? pp_representation_set_next_cursor(page) : NULL;
    const int copied =
        next != NULL
            ? snprintf(cursor_storage, sizeof cursor_storage, "%s", next)
            : 0;
    pp_representation_set_release(page);
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
/* [/representation-structure] */

/* [media-root-lifecycle] */
static pp_error_code_t find_root(const pp_production_t *production,
                                 const char *wanted, pp_uuid_t *out_root_id,
                                 int *out_found, pp_error_t **error) {
  pp_media_root_set_t *roots = NULL;
  *out_found = 0;
  pp_error_code_t status = pp_production_media_roots(production, &roots, error);
  /* Roots are ordered by resolver priority, then stable identity. */
  for (uint64_t i = 0; status == PP_OK && i < pp_media_root_set_count(roots);
       ++i) {
    pp_uuid_t id;
    const char *name, *label, *legacy_uri;
    int32_t priority = 0;
    uint8_t enabled = 0;
    status = pp_media_root_set_get(roots, i, &id, &name, &label, &legacy_uri,
                                   &priority, &enabled, error);
    if (status == PP_OK) {
      printf("root %s (priority %d, %s)\n", name, (int)priority,
             enabled ? "enabled" : "disabled");
    }
    if (status == PP_OK && strcmp(name, wanted) == 0) {
      *out_root_id = id;
      *out_found = 1;
    }
  }
  pp_media_root_set_release(roots);
  return status;
}

static pp_error_code_t apply_root_change(pp_production_t *production,
                                         const pp_uuid_t *root_id, int change,
                                         pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK && change == 0) {
    /* A disabled root keeps its locators but is skipped by resolution. */
    status =
        pp_transaction_set_media_root_enabled(transaction, root_id, 0, error);
  } else if (status == PP_OK && change == 1) {
    status =
        pp_transaction_set_media_root_enabled(transaction, root_id, 1, error);
  } else if (status == PP_OK) {
    status = pp_transaction_remove_media_root(transaction, root_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  return status;
}

static pp_error_code_t cycle_media_root(pp_production_t *production,
                                        const char *name, pp_error_t **error) {
  pp_uuid_t root_id;
  int found = 0;
  pp_error_code_t status = find_root(production, name, &root_id, &found, error);
  if (status == PP_OK && !found) {
    status = PP_ERROR_NOT_FOUND;
  }
  /* Disable, enable again, then remove: each change is its own revision. */
  for (int change = 0; status == PP_OK && change < 3; ++change) {
    status = apply_root_change(production, &root_id, change, error);
  }
  return status;
}
/* [/media-root-lifecycle] */

/* [locator-uri] */
static pp_error_code_t is_recorded_locator(const char *path,
                                           const char *recorded_uri,
                                           int *out_same, pp_error_t **error) {
  /* Spell the path as PostProject spells locators instead of building a URI
   * with the host's own URL type, then compare the strings exactly. */
  char *uri = NULL;
  char *native_path = NULL;
  pp_error_code_t status = pp_file_path_to_locator(path, &uri, error);
  if (status == PP_OK) {
    *out_same = strcmp(uri, recorded_uri) == 0;
    status = pp_locator_to_file_path(recorded_uri, &native_path, error);
  }
  if (status == PP_OK) {
    printf("recorded locator %s is %s\n", recorded_uri, native_path);
  }
  pp_string_release(native_path);
  pp_string_release(uri);
  return status;
}
/* [/locator-uri] */

/* [content-fingerprint] */
static pp_error_code_t print_file_fingerprint(const char *path,
                                              pp_error_t **error) {
  pp_fingerprint_t *fingerprint = NULL;
  pp_error_code_t status = pp_fingerprint_file(path, &fingerprint, error);
  const char *algorithm = NULL;
  uint16_t version = 0;
  const uint8_t *value = NULL;
  uint64_t length = 0;
  if (status == PP_OK) {
    status = pp_fingerprint_get(fingerprint, &algorithm, &version, &value,
                                &length, error);
  }
  if (status == PP_OK) {
    printf("%s v%u: %llu bytes, first %02x\n", algorithm, version,
           (unsigned long long)length, value[0]);
  }
  pp_fingerprint_release(fingerprint);
  return status;
}
/* [/content-fingerprint] */

/* [fingerprint-observation] */
static pp_error_code_t observe_changed_file(pp_production_t *production,
                                            const pp_uuid_t *resource_id,
                                            const char *path,
                                            pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_revision_set_t *latest = NULL;
  pp_revision_event_set_t *events = NULL;
  pp_content_verification_t verification = 0;
  pp_content_observation_t outcome = 0;

  /* Verification only reads: it compares the file with the stored value. */
  pp_error_code_t status = pp_production_verify_resource(
      production, resource_id, path, &verification, error);
  if (status == PP_OK && verification != PP_CONTENT_DIFFERS) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    /* Stages the new resource fingerprint and every representation
     * fingerprint recomputed from it; commit records both in one revision. */
    status = pp_transaction_observe_resource_content(transaction, resource_id,
                                                     path, &outcome, error);
  }
  if (status == PP_OK && outcome != PP_OBSERVATION_CHANGED) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  /* Observing the same content again changes nothing and records nothing. */
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_observe_resource_content(transaction, resource_id,
                                                     path, &outcome, error);
  }
  if (status == PP_OK && outcome != PP_OBSERVATION_UNCHANGED) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_latest_revision(production, &latest, error);
  }
  if (status == PP_OK) {
    pp_uuid_t revision_id, transaction_id;
    uint64_t sequence = 0;
    int64_t committed_at = 0;
    const char *origin_name, *origin_version, *origin_uri, *message;
    status = pp_revision_set_get(latest, 0, &revision_id, &sequence,
                                 &transaction_id, &committed_at, &origin_name,
                                 &origin_version, &origin_uri, &message, error);
    if (status == PP_OK) {
      status = pp_production_revision_events(production, &revision_id, &events,
                                             error);
    }
  }
  for (uint64_t i = 0;
       status == PP_OK && i < pp_revision_event_set_count(events); ++i) {
    pp_revision_event_t event;
    status = pp_revision_event_set_get(events, i, &event, error);
    if (status == PP_OK) {
      printf("event kind %u: %s v%u\n", event.kind, event.fingerprint_algorithm,
             event.fingerprint_version);
    }
  }

  pp_revision_event_set_release(events);
  pp_revision_set_release(latest);
  pp_transaction_release(transaction);
  return status;
}
/* [/fingerprint-observation] */

/* [retire-locator] */
static pp_error_code_t retire_superseded(pp_production_t *production,
                                         const pp_uuid_t *resource_id,
                                         const pp_uuid_t *old_locator_id,
                                         uint64_t *out_remaining,
                                         pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    /* Retire only a locator already superseded by a confirmed one. */
    status = pp_transaction_retire_locator(transaction, old_locator_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);

  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  *out_remaining = 0;
  while (status == PP_OK) {
    pp_locator_query_set_t *page = NULL;
    status = pp_production_locators_page(production, resource_id, UINT32_C(1),
                                         cursor, &page, error);
    for (uint64_t i = 0;
         status == PP_OK && i < pp_locator_query_set_count(page); ++i) {
      pp_uuid_t locator_id, owner_id;
      const char *uri, *media_root;
      pp_locator_availability_t availability;
      uint8_t has_last_seen = 0;
      int64_t last_seen = 0;
      status = pp_locator_query_set_get(page, i, &locator_id, &owner_id, &uri,
                                        &availability, &has_last_seen,
                                        &last_seen, &media_root, error);
      if (status == PP_OK) {
        printf("current locator: %s\n", uri);
        ++*out_remaining;
      }
    }
    const char *next =
        status == PP_OK ? pp_locator_query_set_next_cursor(page) : NULL;
    const int copied =
        next != NULL
            ? snprintf(cursor_storage, sizeof cursor_storage, "%s", next)
            : 0;
    pp_locator_query_set_release(page);
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
/* [/retire-locator] */

/* [resolution-issues] */
static pp_error_code_t
print_resolution_issues(const pp_production_t *production,
                        const pp_uuid_t *asset_id, int64_t *out_missing_frame,
                        uint64_t *out_evidence_count, pp_error_t **error) {
  pp_resolution_set_t *resolutions = NULL;
  /* Resolution is read-only; it never changes the production. */
  pp_error_code_t status = pp_production_resolve_assets(
      production, asset_id, 1, NULL, &resolutions, error);
  const uint64_t count =
      status == PP_OK ? pp_resolution_set_representation_count(resolutions) : 0;
  for (uint64_t r = 0; status == PP_OK && r < count; ++r) {
    pp_uuid_t resolved_asset_id;
    pp_uuid_t representation_id;
    pp_representation_availability_t availability;
    uint64_t resource_count = 0;
    uint64_t issue_count = 0;
    status = pp_resolution_set_get_representation(
        resolutions, r, &resolved_asset_id, &representation_id, &availability,
        &resource_count, &issue_count, error);
    for (uint64_t i = 0; status == PP_OK && i < issue_count; ++i) {
      pp_uuid_t resource_id;
      uint8_t required = 0;
      pp_availability_issue_kind_t kind;
      uint64_t frame_count = 0;
      status =
          pp_resolution_set_get_issue(resolutions, r, i, &resource_id,
                                      &required, &kind, &frame_count, error);
      if (status == PP_OK) {
        printf("issue kind %u (%s)\n", kind,
               required ? "required" : "optional");
      }
      /* Missing frames are sorted individual frame numbers. */
      for (uint64_t f = 0; status == PP_OK && f < frame_count; ++f) {
        status = pp_resolution_set_get_issue_frame(resolutions, r, i, f,
                                                   out_missing_frame, error);
        if (status == PP_OK) {
          printf("  missing frame %lld\n", (long long)*out_missing_frame);
        }
      }
    }
    for (uint64_t s = 0; status == PP_OK && s < resource_count; ++s) {
      pp_uuid_t resource_id;
      pp_resource_resolution_state_t state;
      uint64_t candidate_count = 0;
      uint64_t evidence_count = 0;
      status = pp_resolution_set_get_resource(resolutions, r, s, &resource_id,
                                              &state, &candidate_count,
                                              &evidence_count, error);
      for (uint64_t e = 0; status == PP_OK && e < evidence_count; ++e) {
        pp_evidence_kind_t kind;
        const char *detail = NULL;
        status = pp_resolution_set_get_resource_evidence(resolutions, r, s, e,
                                                         &kind, &detail, error);
        if (status == PP_OK) {
          printf("  resource %llu (state %u) evidence %u: %s\n",
                 (unsigned long long)s, state, kind,
                 detail != NULL ? detail : "-");
          ++*out_evidence_count;
        }
      }
    }
  }
  pp_resolution_set_release(resolutions);
  return status;
}
/* [/resolution-issues] */

static void join(char *buffer, size_t size, const char *directory,
                 const char *name) {
  snprintf(buffer, size, "%s/%s", directory, name);
}

static int write_file(const char *path, const char *contents) {
  FILE *file = fopen(path, "wb");
  if (file == NULL) {
    return 0;
  }
  const size_t length = strlen(contents);
  const int ok = fwrite(contents, 1, length, file) == length;
  return fclose(file) == 0 && ok;
}

static pp_error_code_t create_production(const char *path, const char *media,
                                         const char *renders,
                                         pp_production_t **out_production,
                                         pp_uuid_t *out_asset_id,
                                         pp_error_t **error) {
  const int64_t missing_frames[] = {1003};
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_media_source_t *camera = NULL;
  pp_media_source_t *sequence = NULL;
  pp_uuid_t root_id;
  pp_uuid_t sequence_id;
  pp_error_code_t status =
      pp_production_create(path, "Documentary", &production, error);
  if (status == PP_OK) {
    status = pp_media_source_create_file(media, &camera, error);
  }
  if (status == PP_OK) {
    status = pp_media_source_create_image_sequence(
        renders, "shot010.", ".exr", 4, 1001, 1004, 1, 24000, 1001,
        missing_frames, 1, &sequence, error);
  }
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_import_media(transaction, camera, "Camera A",
                                         out_asset_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_representation(
        transaction, out_asset_id, PP_REPRESENTATION_DERIVED, sequence,
        &sequence_id, error);
  }
  pp_media_source_release(camera);
  pp_media_source_release(sequence);
  if (status == PP_OK) {
    status = pp_transaction_add_media_root(transaction, "proxies",
                                           "Proxy storage", 5, &root_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    *out_production = production;
    production = NULL;
  }
  pp_transaction_release(transaction);
  pp_production_release(production);
  return status;
}

/* Finds the imported original's resource and its current locator. */
static pp_error_code_t
original_resource(const pp_production_t *production, const pp_uuid_t *asset_id,
                  pp_uuid_t *out_representation_id, pp_uuid_t *out_resource_id,
                  pp_uuid_t *out_locator_id, char *out_uri, size_t uri_size,
                  pp_error_t **error) {
  pp_representation_set_t *set = NULL;
  pp_error_code_t status =
      pp_production_representations(production, asset_id, &set, error);
  for (uint64_t r = 0; status == PP_OK && r < pp_representation_set_count(set);
       ++r) {
    pp_uuid_t owner;
    pp_representation_kind_t kind;
    pp_content_structure_kind_t structure;
    uint64_t members, resources, fingerprints;
    status = pp_representation_set_get(set, r, out_representation_id, &owner,
                                       &kind, &structure, &members, &resources,
                                       &fingerprints, error);
    if (status != PP_OK || kind != PP_REPRESENTATION_ORIGINAL) {
      continue;
    }
    uint8_t has_file_facts, has_modified_at, has_last_seen;
    uint64_t size, locator_count, fingerprint_count;
    int64_t modified_at, last_seen;
    const char *uri = NULL;
    pp_locator_availability_t availability;
    status = pp_representation_set_get_resource(
        set, r, 0, out_resource_id, &has_file_facts, &size, &has_modified_at,
        &modified_at, &locator_count, &fingerprint_count, error);
    if (status == PP_OK) {
      status = pp_representation_set_get_locator(
          set, r, 0, locator_count - 1, out_locator_id, &uri, &availability,
          &has_last_seen, &last_seen, error);
    }
    if (status == PP_OK) {
      snprintf(out_uri, uri_size, "%s", uri);
    }
    break;
  }
  pp_representation_set_release(set);
  return status;
}

/* [verify-resolution] */
static pp_error_code_t verify_contents(const pp_production_t *production,
                                       const pp_uuid_t *asset_id,
                                       uint64_t *out_verified,
                                       pp_error_t **error) {
  pp_resolution_options_t *options = NULL;
  pp_resolution_set_t *resolutions = NULL;
  /* Content mode re-fingerprints files at known locators instead of trusting
   * their presence. */
  pp_error_code_t status = pp_resolution_options_create(&options, error);
  if (status == PP_OK) {
    status = pp_resolution_options_set_verification(options, PP_VERIFY_CONTENT,
                                                    error);
  }
  if (status == PP_OK) {
    status = pp_production_resolve_assets(production, asset_id, 1, options,
                                          &resolutions, error);
  }
  pp_resolution_options_release(options);
  const uint64_t count =
      status == PP_OK ? pp_resolution_set_representation_count(resolutions) : 0;
  for (uint64_t r = 0; status == PP_OK && r < count; ++r) {
    pp_uuid_t resolved_asset_id, representation_id;
    pp_representation_availability_t availability;
    uint64_t resource_count = 0, issue_count = 0;
    status = pp_resolution_set_get_representation(
        resolutions, r, &resolved_asset_id, &representation_id, &availability,
        &resource_count, &issue_count, error);
    for (uint64_t s = 0; status == PP_OK && s < resource_count; ++s) {
      pp_uuid_t resource_id;
      pp_resource_resolution_state_t state;
      uint64_t candidate_count = 0, evidence_count = 0;
      status = pp_resolution_set_get_resource(resolutions, r, s, &resource_id,
                                              &state, &candidate_count,
                                              &evidence_count, error);
      if (status == PP_OK && state == PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR) {
        ++*out_verified;
      }
      if (status == PP_OK && state == PP_RESOURCE_RESOLUTION_ERROR) {
        /* PP_EVIDENCE_FINGERPRINT_MISMATCH: the content was replaced. */
        printf("resource %llu content differs\n", (unsigned long long)s);
      }
    }
  }
  pp_resolution_set_release(resolutions);
  return status;
}
/* [/verify-resolution] */

/* [resolve-scope] */
static pp_error_code_t find_nearby(const pp_production_t *production,
                                   const pp_uuid_t *asset_ids,
                                   uint64_t asset_count, const char *directory,
                                   const pp_cancel_token_t *cancel_token,
                                   char *out_uri, size_t out_uri_size,
                                   pp_error_t **error) {
  pp_resolution_options_t *options = NULL;
  pp_resolution_set_t *resolutions = NULL;
  /* A search directory is an unnamed, machine-local place such as the
   * project folder or where the media used to be; it is never recorded. */
  pp_error_code_t status = pp_resolution_options_create(&options, error);
  if (status == PP_OK) {
    status =
        pp_resolution_options_add_search_directory(options, directory, error);
  }
  if (status == PP_OK) {
    status = pp_resolution_options_set_verification(options,
                                                    PP_VERIFY_PRESENCE, error);
  }
  if (status == PP_OK) {
    /* Each searched directory has its own budget; an oversized one is
     * searched partially and reported as PP_EVIDENCE_SEARCH_TRUNCATED. */
    status = pp_resolution_options_set_limits(options, 16, 50000, error);
  }
  if (status == PP_OK) {
    /* Another thread may call pp_cancel_token_cancel() to stop the scan. */
    status =
        pp_resolution_options_set_cancel_token(options, cancel_token, error);
  }
  if (status == PP_OK) {
    /* All assets are resolved together; each directory is scanned once. */
    status = pp_production_resolve_assets(production, asset_ids, asset_count,
                                          options, &resolutions, error);
  }
  pp_resolution_options_release(options);

  out_uri[0] = '\0';
  const uint64_t count =
      status == PP_OK ? pp_resolution_set_representation_count(resolutions) : 0;
  for (uint64_t r = 0; status == PP_OK && r < count; ++r) {
    pp_uuid_t asset_id, representation_id;
    pp_representation_availability_t availability;
    uint64_t resource_count = 0, issue_count = 0;
    status = pp_resolution_set_get_representation(
        resolutions, r, &asset_id, &representation_id, &availability,
        &resource_count, &issue_count, error);
    for (uint64_t s = 0; status == PP_OK && s < resource_count; ++s) {
      pp_uuid_t resource_id;
      pp_resource_resolution_state_t state;
      uint64_t candidate_count = 0, evidence_count = 0;
      status = pp_resolution_set_get_resource(resolutions, r, s, &resource_id,
                                              &state, &candidate_count,
                                              &evidence_count, error);
      const char *uri = NULL;
      const char *root = NULL;
      uint16_t confidence = 0;
      if (status == PP_OK && candidate_count == 1) {
        status = pp_resolution_set_get_candidate(resolutions, r, s, 0, &uri,
                                                 &confidence, &root,
                                                 &evidence_count, error);
      }
      for (uint64_t e = 0; status == PP_OK && uri != NULL && e < evidence_count;
           ++e) {
        pp_evidence_kind_t kind;
        const char *detail = NULL;
        status = pp_resolution_set_get_candidate_evidence(
            resolutions, r, s, 0, e, &kind, &detail, error);
        if (status == PP_OK) {
          printf("%s evidence %u: %s\n", uri, kind,
                 detail != NULL ? detail : "-");
        }
      }
      /* Only discovered candidates need confirmation; one found in a search
       * directory has no root (root == NULL). */
      const int discovered = state == PP_RESOURCE_RESOLVED_EXACT ||
                             state == PP_RESOURCE_RESOLVED_PROBABLE;
      if (status == PP_OK && discovered && uri != NULL && out_uri[0] == '\0' &&
          root == NULL) {
        snprintf(out_uri, out_uri_size, "%s", uri);
      }
    }
  }
  pp_resolution_set_release(resolutions);
  return status;
}
/* [/resolve-scope] */

static pp_error_code_t confirm_moved(pp_production_t *production,
                                     const pp_uuid_t *asset_id,
                                     const pp_uuid_t *resource_id,
                                     const char *moved_directory,
                                     pp_error_t **error) {
  /* Find the moved file near where it went and confirm what was found. */
  char new_uri[4096];
  pp_cancel_token_t *cancel_token = NULL;
  pp_error_code_t status = pp_cancel_token_create(&cancel_token, error);
  if (status == PP_OK) {
    status = find_nearby(production, asset_id, 1, moved_directory, cancel_token,
                         new_uri, sizeof new_uri, error);
  }
  if (status == PP_OK && new_uri[0] == '\0') {
    status = PP_ERROR_NOT_FOUND;
  }
  /* A cancelled token stops the next resolution with PP_ERROR_CANCELLED. */
  char ignored[16];
  pp_error_t *cancelled = NULL;
  pp_cancel_token_cancel(cancel_token);
  if (status == PP_OK &&
      find_nearby(production, asset_id, 1, moved_directory, cancel_token,
                  ignored, sizeof ignored, &cancelled) != PP_ERROR_CANCELLED) {
    status = PP_ERROR_INTERNAL;
  }
  pp_error_release(cancelled);
  pp_cancel_token_release(cancel_token);

  pp_transaction_t *transaction = NULL;
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_confirm_locator(transaction, resource_id, new_uri,
                                            error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  return status;
}

static pp_error_code_t add_unmapped_root(pp_production_t *production,
                                         pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_uuid_t root_id;
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_add_media_root(
        transaction, "rushes", "Camera originals", 0, &root_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  return status;
}

static pp_error_code_t count_events(const pp_production_t *production,
                                    uint64_t after,
                                    pp_revision_event_kind_t kind,
                                    uint64_t *out_count, pp_error_t **error) {
  pp_revision_set_t *revisions = NULL;
  *out_count = 0;
  pp_error_code_t status =
      pp_production_changes_since(production, after, 1000, &revisions, error);
  for (uint64_t i = 0; status == PP_OK && i < pp_revision_set_count(revisions);
       ++i) {
    pp_uuid_t revision_id, transaction_id;
    uint64_t sequence = 0;
    int64_t committed_at = 0;
    const char *origin_name, *origin_version, *origin_uri, *message;
    pp_revision_event_set_t *events = NULL;
    status = pp_revision_set_get(revisions, i, &revision_id, &sequence,
                                 &transaction_id, &committed_at, &origin_name,
                                 &origin_version, &origin_uri, &message, error);
    if (status == PP_OK) {
      status = pp_production_revision_events(production, &revision_id, &events,
                                             error);
    }
    for (uint64_t j = 0;
         status == PP_OK && j < pp_revision_event_set_count(events); ++j) {
      pp_revision_event_t event;
      status = pp_revision_event_set_get(events, j, &event, error);
      if (status == PP_OK && event.kind == kind) {
        ++*out_count;
      }
    }
    pp_revision_event_set_release(events);
  }
  pp_revision_set_release(revisions);
  return status;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: postproject-c-media WORK_DIRECTORY\n");
    return 2;
  }
  const char *work = argv[1];
  char production_path[4096], media[4096], moved_media[4096], renders[4096];
  char directory[4096], proxy[4096], part1[4096], part2[4096];
  char essence[4096], sidecar[4096];
  join(production_path, sizeof production_path, work, "media.pproj");
  join(media, sizeof media, work, "rushes/A001.mov");
  join(moved_media, sizeof moved_media, work, "moved/A001.mov");
  join(renders, sizeof renders, work, "renders/shot010");
  join(proxy, sizeof proxy, work, "proxies/A001_proxy.mov");
  join(part1, sizeof part1, work, "spanned/CLIP0001.MTS");
  join(part2, sizeof part2, work, "spanned/CLIP0002.MTS");
  join(essence, sizeof essence, work, "package/clip.mxf");
  join(sidecar, sizeof sidecar, work, "package/clip.xml");

  int files_ok = 1;
  const char *const directories[] = {"proxies", "spanned", "package"};
  for (size_t i = 0; i < sizeof directories / sizeof directories[0]; ++i) {
    join(directory, sizeof directory, work, directories[i]);
    make_directory(directory);
  }
  files_ok = write_file(proxy, "proxy essence\n") &&
             write_file(part1, "span one\n") &&
             write_file(part2, "span two\n") &&
             write_file(essence, "package essence\n") &&
             write_file(sidecar, "<clip/>\n");

  pp_production_t *production = NULL;
  pp_error_t *error = NULL;
  pp_uuid_t asset_id, proxy_id, spanned_id, package_id;
  pp_uuid_t original_id, resource_id, old_locator_id;
  char old_uri[4096] = {0};
  uint64_t count = 0;
  uint64_t checkpoint = 0;
  int64_t missing_frame = 0;
  int found = 0;

  pp_error_code_t status = files_ok ? PP_OK : PP_ERROR_IO;
  if (status == PP_OK) {
    status = create_production(production_path, media, renders, &production,
                               &asset_id, &error);
  }
  if (status == PP_OK) {
    status = add_proxy(production, &asset_id, proxy, &proxy_id, &error);
  }
  if (status == PP_OK) {
    status = add_spanned_clip(production, &asset_id, part1, part2, &spanned_id,
                              &error);
  }
  if (status == PP_OK) {
    status = add_package(production, &asset_id, essence, sidecar, &package_id,
                         &error);
  }
  if (status == PP_OK) {
    status = print_asset_structure(production, &asset_id, &count,
                                   &missing_frame, &error);
  }
  /* Original, sequence, proxy, spanned clip, and package. */
  if (status == PP_OK && (count != UINT64_C(5) || missing_frame != 1003)) {
    status = PP_ERROR_INTERNAL;
  }

  if (status == PP_OK) {
    pp_revision_set_t *latest = NULL;
    pp_uuid_t revision_id, transaction_id;
    int64_t committed_at;
    const char *a, *b, *c, *d;
    status = pp_production_latest_revision(production, &latest, &error);
    if (status == PP_OK) {
      status = pp_revision_set_get(latest, 0, &revision_id, &checkpoint,
                                   &transaction_id, &committed_at, &a, &b, &c,
                                   &d, &error);
    }
    pp_revision_set_release(latest);
  }
  if (status == PP_OK) {
    status = cycle_media_root(production, "proxies", &error);
  }
  if (status == PP_OK) {
    status =
        count_events(production, checkpoint,
                     PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED, &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(2)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = count_events(production, checkpoint,
                          PP_REVISION_MEDIA_ROOT_REMOVED, &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(1)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    pp_uuid_t unused;
    status = find_root(production, "proxies", &unused, &found, &error);
  }
  if (status == PP_OK && found) {
    status = PP_ERROR_INTERNAL;
  }

  if (status == PP_OK) {
    status =
        original_resource(production, &asset_id, &original_id, &resource_id,
                          &old_locator_id, old_uri, sizeof old_uri, &error);
  }
  if (status == PP_OK) {
    int same = 0;
    status = is_recorded_locator(media, old_uri, &same, &error);
    if (status == PP_OK && !same) {
      status = PP_ERROR_INTERNAL;
    }
  }
  /* The camera original is re-rendered in place with new bytes. */
  if (status == PP_OK && !write_file(media, "re-rendered camera original\n")) {
    status = PP_ERROR_IO;
  }
  if (status == PP_OK) {
    status = print_file_fingerprint(media, &error);
  }
  if (status == PP_OK) {
    status = observe_changed_file(production, &resource_id, media, &error);
  }
  if (status == PP_OK) {
    uint64_t resource_events = 0;
    status = count_events(production, checkpoint,
                          PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED,
                          &resource_events, &error);
    if (status == PP_OK) {
      status = count_events(production, checkpoint,
                            PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED,
                            &count, &error);
    }
    if (status == PP_OK && (resource_events != 1 || count != 1)) {
      status = PP_ERROR_INTERNAL;
    }
  }

  if (status == PP_OK && rename(media, moved_media) != 0) {
    perror("move media");
    status = PP_ERROR_IO;
  }
  if (status == PP_OK) {
    char moved_directory[4096];
    join(moved_directory, sizeof moved_directory, work, "moved");
    status = confirm_moved(production, &asset_id, &resource_id,
                           moved_directory, &error);
  }
  if (status == PP_OK) {
    status = retire_superseded(production, &resource_id, &old_locator_id,
                               &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(1)) {
    status = PP_ERROR_INTERNAL;
  }

  /* The optional sidecar goes missing, and a "rushes" root has no mapping. */
  if (status == PP_OK && remove(sidecar) != 0) {
    status = PP_ERROR_IO;
  }
  if (status == PP_OK) {
    status = add_unmapped_root(production, &error);
  }
  uint64_t verified = 0;
  if (status == PP_OK) {
    status = verify_contents(production, &asset_id, &verified, &error);
  }
  if (status == PP_OK && verified == 0) {
    status = PP_ERROR_INTERNAL;
  }
  missing_frame = 0;
  count = 0;
  if (status == PP_OK) {
    status = print_resolution_issues(production, &asset_id, &missing_frame,
                                     &count, &error);
  }
  if (status == PP_OK && (missing_frame != 1003 || count == 0)) {
    status = PP_ERROR_INTERNAL;
  }

  pp_uuid_t strip_id;
  pp_content_structure_kind_t strip_kind = 0;
  if (status == PP_OK) {
    status = import_image_strip(production, renders, &strip_id, &strip_kind,
                                &error);
  }
  if (status == PP_OK && strip_kind != PP_CONTENT_IMAGE_SEQUENCE) {
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
