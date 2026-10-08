/* Runs the C listings for provenance, artifact knowledge, and dependencies.
 *
 * Each "[name]" ... "[/name]" region is included verbatim by the documentation
 * build, so keep regions self-contained and readable. Usage:
 *   postproject-c-provenance WORK_DIRECTORY
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

/* Caller-owned fingerprint algorithms computed by this host (see below). */
#define HOST_RESOURCE_ALGORITHM "example-fnv1a-64"
#define HOST_REPRESENTATION_ALGORITHM "example-fnv1a-64-representation"

/* A host-defined content hash: 64-bit FNV-1a over the file bytes. Hosts
 * usually use a stronger digest; the ABI accepts any caller-owned algorithm. */
static int host_fingerprint(const char *path, uint8_t out_value[8]) {
  FILE *file = fopen(path, "rb");
  if (file == NULL) {
    return 0;
  }
  uint64_t hash = UINT64_C(14695981039346656037);
  int byte;
  while ((byte = fgetc(file)) != EOF) {
    hash ^= (uint64_t)(unsigned char)byte;
    hash *= UINT64_C(1099511628211);
  }
  const int ok = ferror(file) == 0;
  fclose(file);
  for (int i = 0; i < 8; ++i) {
    out_value[i] = (uint8_t)(hash >> (56 - 8 * i));
  }
  return ok;
}

/* Records the host fingerprint of a single-file representation. */
static pp_error_code_t observe_file(pp_production_t *production,
                                    const pp_resource_id_t *resource_id,
                                    const pp_representation_id_t *representation_id,
                                    const char *path, pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_read_session_t *view = NULL;
  uint8_t value[8];
  pp_error_code_t status =
      pp_production_read_session(production, &view, error);
  if (status == PP_OK) {
    status = pp_read_session_begin_edit(view, &transaction, error);
  }
  pp_read_session_release(view);
  view = NULL;
  if (status == PP_OK && !host_fingerprint(path, value)) {
    status = PP_ERROR_IO;
  }
  if (status == PP_OK) {
    status = pp_transaction_record_resource_fingerprint(
        transaction, *resource_id, HOST_RESOURCE_ALGORITHM, 1, value,
        sizeof value, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_record_representation_fingerprint(
        transaction, *representation_id, HOST_REPRESENTATION_ALGORITHM, 1, value,
        sizeof value, error);
  }
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit_with_receipt(transaction, &commit_receipt, error);
  }
  pp_transaction_release(transaction);
  return status;
}

/* [activity-snapshots] */
static pp_error_code_t print_edge_snapshot(const pp_activity_set_t *set,
                                           uint64_t a, uint64_t e, int input,
                                           pp_error_t **error) {
  uint8_t has_snapshot = 0;
  uint64_t revision = 0;
  uint64_t count = 0;
  /* Snapshots are captured at commit; a migrated edge may have none. */
  pp_error_code_t status =
      input ? pp_activity_set_get_input_snapshot(set, a, e, &has_snapshot,
                                                 &revision, &count, error)
            : pp_activity_set_get_output_snapshot(set, a, e, &has_snapshot,
                                                  &revision, &count, error);
  if (status == PP_OK && has_snapshot) {
    printf("    snapshot at revision %llu\n", (unsigned long long)revision);
  }
  for (uint64_t f = 0; status == PP_OK && has_snapshot && f < count; ++f) {
    const char *algorithm;
    uint16_t version = 0;
    const uint8_t *value;
    uint64_t length = 0;
    uint8_t has_observed = 0;
    uint64_t observed = 0;
    status = input ? pp_activity_set_get_input_snapshot_fingerprint(
                         set, a, e, f, &algorithm, &version, &value, &length,
                         &has_observed, &observed, error)
                   : pp_activity_set_get_output_snapshot_fingerprint(
                         set, a, e, f, &algorithm, &version, &value, &length,
                         &has_observed, &observed, error);
    if (status == PP_OK) {
      printf("      %s v%u (%llu bytes)\n", algorithm, version,
             (unsigned long long)length);
    }
  }
  return status;
}

static pp_error_code_t print_activity(const pp_activity_set_t *set, uint64_t a,
                                      pp_error_t **error) {
  pp_activity_id_t id;
  const char *kind;
  uint8_t has_started = 0, has_finished = 0;
  int64_t started = 0, finished = 0;
  uint64_t input_count = 0, output_count = 0;
  const char *tool = NULL, *tool_version = NULL, *tool_uri = NULL;
  const char *agent = NULL, *agent_scheme = NULL, *agent_value = NULL;
  const char *agent_qualifier = NULL;
  pp_error_code_t status = pp_activity_set_get(
      set, a, &id, &kind, &has_started, &started, &has_finished, &finished,
      &input_count, &output_count, error);
  if (status == PP_OK) {
    status = pp_activity_set_get_tool(set, a, &tool, &tool_version, &tool_uri,
                                      error);
  }
  if (status == PP_OK) {
    status = pp_activity_set_get_agent(set, a, &agent, &agent_scheme,
                                       &agent_value, &agent_qualifier, error);
  }
  if (status == PP_OK) {
    printf("%s by %s %s, run by %s (%s:%s)\n", kind, tool != NULL ? tool : "-",
           tool_version != NULL ? tool_version : "",
           agent != NULL ? agent : "-",
           agent_scheme != NULL ? agent_scheme : "",
           agent_value != NULL ? agent_value : "");
  }
  for (uint64_t e = 0; status == PP_OK && e < input_count; ++e) {
    pp_representation_id_t representation_id;
    const char *role;
    status =
        pp_activity_set_get_input(set, a, e, &representation_id, &role, error);
    if (status == PP_OK) {
      printf("  input role %s\n", role != NULL ? role : "-");
      status = print_edge_snapshot(set, a, e, 1, error);
    }
  }
  for (uint64_t e = 0; status == PP_OK && e < output_count; ++e) {
    pp_representation_id_t representation_id;
    const char *role;
    status =
        pp_activity_set_get_output(set, a, e, &representation_id, &role, error);
    if (status == PP_OK) {
      printf("  output role %s\n", role != NULL ? role : "-");
      status = print_edge_snapshot(set, a, e, 0, error);
    }
  }
  return status;
}

static pp_error_code_t record_transcode(pp_production_t *production,
                                        const pp_representation_id_t *original_id,
                                        const pp_representation_id_t *proxy_id,
                                        uint64_t counts[4],
                                        pp_error_t **error) {
  const pp_activity_edge_t inputs[] = {
      {*original_id, "org.postproject:input.primary-video"}};
  const pp_activity_edge_t outputs[] = {
      {*proxy_id, "org.postproject:output.proxy"}};
  const int64_t started = INT64_C(1767225600000000);
  const int64_t finished = INT64_C(1767225660000000);
  pp_transaction_t *transaction = NULL;
  pp_activity_set_t *activities = NULL;
  pp_activity_set_t *consuming = NULL;
  pp_object_ref_set_t *descendants = NULL;
  pp_object_query_set_t *descendant_page = NULL;
  pp_activity_id_t activity_id = {0};

  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    /* Tool and agent identity are independent, optional attributions. */
    status = pp_transaction_create_activity(
        transaction, "org.postproject:transcode", inputs, 1, outputs, 1,
        &started, &finished, "Example Transcoder", "3.2",
        "https://example.com/transcoder", "Assistant editor",
        "com.example.staff", "ae-17", NULL, &activity_id, error);
  }
  if (status == PP_OK) {
    char *saved = NULL;
    pp_activity_id_t parsed = {0};
    pp_object_ref_t target;
    status = pp_activity_id_format(activity_id, &saved, error);
    if (status == PP_OK) status = pp_activity_id_parse(saved, &parsed, error);
    pp_string_release(saved);
    if (status == PP_OK) status = pp_object_ref_from_activity(parsed, &target, error);
    if (status == PP_OK) status = pp_object_ref_get_activity(&target, &parsed, error);
    if (status == PP_OK && memcmp(parsed.bytes, activity_id.bytes, 16))
      status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    char *saved = NULL;
    pp_representation_id_t parsed = {0};
    pp_object_ref_t target;
    status = pp_representation_id_format(*original_id, &saved, error);
    if (status == PP_OK) status = pp_representation_id_parse(saved, &parsed, error);
    pp_string_release(saved);
    if (status == PP_OK) status = pp_object_ref_from_representation(parsed, &target, error);
    if (status == PP_OK) status = pp_object_ref_get_representation(&target, &parsed, error);
    if (status == PP_OK && memcmp(parsed.bytes, original_id->bytes, 16))
      status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit_with_receipt(transaction, &commit_receipt, error);
  }
  if (status == PP_OK) {
    status = pp_production_activities(production, &activities, error);
  }
  for (uint64_t a = 0; status == PP_OK && a < pp_activity_set_count(activities);
       ++a) {
    status = print_activity(activities, a, error);
  }
  if (status == PP_OK) {
    status = pp_production_activities_consuming(production, *original_id,
                                                &consuming, error);
  }
  if (status == PP_OK) {
    status = pp_production_provenance_descendants(production, *original_id,
                                                  &descendants, error);
  }
  if (status == PP_OK) {
    /* The page form bounds depth and visited representations. */
    status = pp_production_provenance_descendants_page(
        production, *original_id, UINT32_C(8), UINT32_C(1000), UINT32_C(100),
        NULL, &descendant_page, error);
  }
  for (uint64_t i = 0;
       status == PP_OK && i < pp_object_query_set_count(descendant_page); ++i) {
    pp_object_ref_t descendant;
    uint32_t depth = 0;
    status =
        pp_object_query_set_get(descendant_page, i, &descendant, &depth, error);
    if (status == PP_OK) {
      printf("descendant at depth %u%s\n", depth,
             pp_object_query_set_next_cursor(descendant_page) != NULL
                 ? " (more pages follow)"
                 : "");
    }
  }
  if (status == PP_OK) {
    counts[0] = pp_activity_set_count(activities);
    counts[1] = pp_activity_set_count(consuming);
    counts[2] = pp_object_ref_set_count(descendants);
    counts[3] = pp_object_query_set_count(descendant_page);
  }

  pp_object_query_set_release(descendant_page);
  pp_object_ref_set_release(descendants);
  pp_activity_set_release(consuming);
  pp_activity_set_release(activities);
  pp_transaction_release(transaction);
  return status;
}

static pp_error_code_t
count_consumers_by_page(const pp_production_t *production,
                        const pp_representation_id_t *original_id, uint64_t *out_count,
                        pp_error_t **error) {
  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  pp_error_code_t status = PP_OK;
  *out_count = 0;
  while (status == PP_OK) {
    pp_activity_set_t *page = NULL;
    status = pp_production_activities_consuming_page(
        production, *original_id, UINT32_C(1), cursor, &page, error);
    *out_count += status == PP_OK ? pp_activity_set_count(page) : 0;
    /* The cursor borrows the page; copy it before releasing the page. */
    const char *next =
        status == PP_OK ? pp_activity_set_next_cursor(page) : NULL;
    const int copied =
        next != NULL
            ? snprintf(cursor_storage, sizeof cursor_storage, "%s", next)
            : 0;
    pp_activity_set_release(page);
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
/* [/activity-snapshots] */

/* [stale-after-change] */
static pp_error_code_t explain_stale_proxy(
    pp_production_t *production, const pp_resource_id_t *original_resource,
    const pp_representation_id_t *original_id, const char *original_path,
    const pp_representation_id_t *proxy_id, pp_artifact_knowledge_state_t *state,
    uint32_t *out_reason_kinds, uint32_t *out_issue_kinds, pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_read_session_t *view = NULL;
  pp_artifact_evaluation_t *evaluation = NULL;
  pp_artifact_reproducibility_t *report = NULL;
  pp_representation_id_t evaluated_id;
  uint32_t visited = 0;
  uint8_t truncated = 0;
  uint64_t reason_count = 0;
  uint8_t value[8];

  /* Record what the host observes now: the original's bytes changed. */
  pp_error_code_t status =
      pp_production_read_session(production, &view, error);
  if (status == PP_OK) {
    status = pp_read_session_begin_edit(view, &transaction, error);
  }
  pp_read_session_release(view);
  view = NULL;
  if (status == PP_OK && !host_fingerprint(original_path, value)) {
    status = PP_ERROR_IO;
  }
  if (status == PP_OK) {
    status = pp_transaction_record_resource_fingerprint(
        transaction, *original_resource, HOST_RESOURCE_ALGORITHM, 1, value,
        sizeof value, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_record_representation_fingerprint(
        transaction, *original_id, HOST_REPRESENTATION_ALGORITHM, 1, value,
        sizeof value, error);
  }
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit_with_receipt(transaction, &commit_receipt, error);
  }
  if (status == PP_OK) {
    status = pp_production_evaluate_artifact(
        production, *proxy_id, UINT32_C(64), UINT32_C(1000), &evaluation, error);
  }
  if (status == PP_OK) {
    status =
        pp_artifact_evaluation_get(evaluation, &evaluated_id, state, &visited,
                                   &truncated, &reason_count, error);
  }
  for (uint64_t i = 0; status == PP_OK && i < reason_count; ++i) {
    pp_artifact_reason_kind_t kind = 0;
    status = pp_artifact_evaluation_get_reason_kind(evaluation, i, &kind, error);
    pp_artifact_reason_t reason = {0};
    if (status == PP_OK) {
      status = pp_artifact_evaluation_get_reason(evaluation, i, kind, &reason, error);
    }
    if (status == PP_OK) {
      *out_reason_kinds |= reason.kind < 32 ? UINT32_C(1) << reason.kind : 0;
      printf("reason %u\n", kind);
      if (kind == PP_ARTIFACT_REASON_FINGERPRINT_CHANGED ||
          kind == PP_ARTIFACT_REASON_FINGERPRINT_EVIDENCE_MISSING) {
        printf("%s edge: %s v%u, snapshot %llu bytes, now %llu\n",
             reason.edge_kind == PP_ARTIFACT_EDGE_INPUT ? "input" : "output",
             reason.fingerprint_algorithm != NULL ? reason.fingerprint_algorithm
                                                  : "-",
             reason.fingerprint_version,
             (unsigned long long)reason.snapshot_value_length,
             (unsigned long long)reason.current_value_length);
      }
    }
  }
  if (status == PP_OK) {
    status = pp_production_artifact_reproducibility(production, *proxy_id,
                                                    &report, error);
  }
  if (status == PP_OK) {
    uint8_t reproducible = 0;
    uint8_t has_activity = 0;
    pp_activity_id_t activity_id = {0};
    const char *activity_kind;
    uint64_t issue_count = 0;
    status = pp_artifact_reproducibility_get(
        report, &evaluated_id, &reproducible, &has_activity, &activity_id,
        &activity_kind, &issue_count, error);
    /* Issues name what a regeneration would still be missing. */
    for (uint64_t i = 0; status == PP_OK && i < issue_count; ++i) {
      pp_artifact_reproducibility_issue_kind_t kind = 0;
      status = pp_artifact_reproducibility_get_issue_kind(report, i, &kind, error);
      pp_artifact_reproducibility_issue_t issue = {0};
      if (status == PP_OK) {
        status = pp_artifact_reproducibility_get_issue(report, i, kind, &issue, error);
      }
      if (status == PP_OK) {
        *out_issue_kinds |= issue.kind < 32 ? UINT32_C(1) << issue.kind : 0;
        printf("reproducibility issue %u\n", issue.kind);
      }
    }
  }

  pp_artifact_reproducibility_release(report);
  pp_artifact_evaluation_release(evaluation);
  pp_transaction_release(transaction);
  return status;
}
/* [/stale-after-change] */

/* [dependency-set] */
static pp_error_code_t read_dependency_set(
    const pp_production_t *production, const pp_representation_id_t *representation_id,
    pp_dependency_set_status_t *out_status, pp_error_t **error) {
  pp_dependency_set_t *set = NULL;
  uint8_t present = 0;
  pp_representation_id_t source_id;
  uint64_t recorded_at = 0;
  uint64_t count = 0;
  pp_error_code_t status =
      pp_production_dependency_set(production, *representation_id, &set, error);
  if (status == PP_OK) {
    status = pp_dependency_set_get(set, &present, &source_id, &recorded_at,
                                   out_status, &count, error);
  }
  /* Absent knowledge differs from a recorded empty set. */
  if (status == PP_OK && !present) {
    printf("no dependency set recorded\n");
  }
  for (uint64_t i = 0; status == PP_OK && present && i < count; ++i) {
    pp_dependency_t dependency;
    status = pp_dependency_set_get_dependency(set, i, &dependency, error);
    if (status == PP_OK) {
      printf("%s -> %s (%s)\n", dependency.kind,
             dependency.authored_reference != NULL
                 ? dependency.authored_reference
                 : "-",
             dependency.required ? "required" : "optional");
    }
  }
  if (status == PP_OK && present) {
    printf("status %u, recorded at revision %llu\n", *out_status,
           (unsigned long long)recorded_at);
  }
  pp_dependency_set_release(set);
  return status;
}

static pp_error_code_t record_dependencies(pp_production_t *production,
                                           const pp_representation_id_t *source_id,
                                           const pp_asset_id_t *original_asset,
                                           const pp_representation_id_t *original_id,
                                           pp_error_t **error) {
  pp_dependency_t dependencies[2];
  memset(dependencies, 0, sizeof dependencies);
  dependencies[0].kind = "org.example:source-media";
  pp_error_code_t reference_status = pp_object_ref_from_asset(*original_asset, &dependencies[0].target, error);
  if (reference_status != PP_OK) return reference_status;
  dependencies[0].has_resolved_representation = 1;
  dependencies[0].resolved_representation_id = *original_id;
  dependencies[0].required = 1;
  dependencies[0].authored_reference = "rushes/A001.mov";
  dependencies[1].kind = "org.example:timecode-reference";
  reference_status = pp_object_ref_from_representation(*original_id, &dependencies[1].target, error);
  if (reference_status != PP_OK) return reference_status;
  dependencies[1].required = 0;
  dependencies[1].authored_reference = "A001.mov#timecode";

  pp_read_session_t *view = NULL;
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status =
      pp_production_read_session(production, &view, error);
  if (status == PP_OK) {
    status = pp_read_session_begin_edit(view, &transaction, error);
  }
  pp_read_session_release(view);
  if (status == PP_OK) {
    /* The call replaces the complete, ordered dependency observation. */
    status = pp_transaction_record_dependency_set(transaction, *source_id,
                                                  dependencies, 2, error);
  }
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit_with_receipt(transaction, &commit_receipt, error);
  }
  pp_transaction_release(transaction);
  return status;
}

static pp_error_code_t observe_source(pp_production_t *production,
                                      const pp_representation_id_t *source_id,
                                      const uint8_t value[8],
                                      pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_read_session_t *view = NULL;
  pp_error_code_t status =
      pp_production_read_session(production, &view, error);
  if (status == PP_OK) {
    status = pp_read_session_begin_edit(view, &transaction, error);
  }
  pp_read_session_release(view);
  view = NULL;
  if (status == PP_OK) {
    /* A new source fingerprint marks its dependency set for re-extraction. */
    status = pp_transaction_record_representation_fingerprint(
        transaction, *source_id, HOST_REPRESENTATION_ALGORITHM, 1, value, 8,
        error);
  }
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit_with_receipt(transaction, &commit_receipt, error);
  }
  pp_transaction_release(transaction);
  return status;
}

static pp_error_code_t
count_dependencies_by_page(const pp_production_t *production,
                           const pp_representation_id_t *source_id, uint64_t *out_count,
                           pp_error_t **error) {
  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  pp_error_code_t status = PP_OK;
  *out_count = 0;
  while (status == PP_OK) {
    pp_dependency_query_set_t *page = NULL;
    status = pp_production_dependencies(production, *source_id, UINT32_C(4),
                                        UINT32_C(1000), UINT32_C(1), cursor,
                                        &page, error);
    *out_count += status == PP_OK ? pp_dependency_query_set_count(page) : 0;
    const char *next =
        status == PP_OK ? pp_dependency_query_set_next_cursor(page) : NULL;
    const int copied =
        next != NULL
            ? snprintf(cursor_storage, sizeof cursor_storage, "%s", next)
            : 0;
    pp_dependency_query_set_release(page);
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
/* [/dependency-set] */

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

/* Imports the original and adds the proxy, returning both IDs. */
static pp_error_code_t
create_production(const char *path, const char *media, const char *proxy,
                  pp_production_t **out_production, pp_asset_id_t *out_asset_id,
                  pp_representation_id_t *out_original_id, pp_resource_id_t *out_resource_id,
                  pp_representation_id_t *out_proxy_id, pp_error_t **error) {
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_media_source_t *camera = NULL;
  pp_representation_set_t *set = NULL;
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
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit_with_receipt(transaction, &commit_receipt, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  if (status == PP_OK) {
    status =
        pp_production_representations(production, *out_asset_id, &set, error);
  }
  if (status == PP_OK) {
    pp_asset_id_t owner;
    pp_representation_kind_t kind;
    pp_content_structure_kind_t structure;
    uint64_t members, resources, fingerprints;
    const char *role;
    uint8_t required;
    status = pp_representation_set_get(set, 0, out_original_id, &owner, &kind,
                                       &structure, &members, &resources,
                                       &fingerprints, error);
    if (status == PP_OK) {
      status = pp_representation_set_get_member(set, 0, 0, out_resource_id,
                                                &role, &required, error);
    }
  }
  pp_media_source_t *proxy_file = NULL;
  if (status == PP_OK) {
    status = pp_media_source_create_file(proxy, &proxy_file, error);
  }
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_representation(
        transaction, *out_asset_id, PP_REPRESENTATION_PROXY, proxy_file,
        out_proxy_id, error);
  }
  pp_media_source_release(proxy_file);
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit_with_receipt(transaction, &commit_receipt, error);
  }
  if (status == PP_OK) {
    *out_production = production;
    production = NULL;
  }
  pp_representation_set_release(set);
  pp_transaction_release(transaction);
  pp_production_release(production);
  return status;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: postproject-c-provenance WORK_DIRECTORY\n");
    return 2;
  }
  const char *work = argv[1];
  char production_path[4096], media[4096], proxies[4096], proxy[4096];
  join(production_path, sizeof production_path, work, "provenance.pproj");
  join(media, sizeof media, work, "rushes/A001.mov");
  join(proxies, sizeof proxies, work, "proxies");
  join(proxy, sizeof proxy, work, "proxies/A001_proxy.mov");
  make_directory(proxies);

  pp_production_t *production = NULL;
  pp_error_t *error = NULL;
  pp_asset_id_t asset_id;
  pp_representation_id_t original_id;
  pp_resource_id_t resource_id;
  pp_representation_id_t proxy_id;
  uint64_t counts[4] = {0, 0, 0, 0};
  uint64_t count = 0;
  pp_artifact_knowledge_state_t state = 0;
  uint32_t reason_kinds = 0;
  uint32_t issue_kinds = 0;
  pp_dependency_set_status_t dependency_status = 0;

  pp_error_code_t status =
      write_file(proxy, "proxy essence\n") ? PP_OK : PP_ERROR_IO;
  if (status == PP_OK) {
    status =
        create_production(production_path, media, proxy, &production, &asset_id,
                          &original_id, &resource_id, &proxy_id, &error);
  }
  /* Observe the original with the host algorithm before the activity, so its
   * input snapshot holds a value this host can recompute later. */
  if (status == PP_OK) {
    status =
        observe_file(production, &resource_id, &original_id, media, &error);
  }
  if (status == PP_OK) {
    status =
        record_transcode(production, &original_id, &proxy_id, counts, &error);
  }
  if (status == PP_OK &&
      (counts[0] != 1 || counts[1] != 1 || counts[2] != 1 || counts[3] != 1)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = count_consumers_by_page(production, &original_id, &count, &error);
  }
  if (status == PP_OK && count != 1) {
    status = PP_ERROR_INTERNAL;
  }

  /* The camera original is re-rendered in place with new bytes. */
  if (status == PP_OK && !write_file(media, "re-rendered camera original\n")) {
    status = PP_ERROR_IO;
  }
  if (status == PP_OK) {
    status = explain_stale_proxy(production, &resource_id, &original_id, media,
                                 &proxy_id, &state, &reason_kinds, &issue_kinds,
                                 &error);
  }
  if (status == PP_OK &&
      (state != PP_ARTIFACT_STALE ||
       (reason_kinds &
        (UINT32_C(1) << PP_ARTIFACT_REASON_FINGERPRINT_CHANGED)) == 0 ||
       (issue_kinds &
        (UINT32_C(1) << PP_ARTIFACT_REPRODUCIBILITY_PARAMETERS_MISSING)) ==
           0)) {
    status = PP_ERROR_INTERNAL;
  }

  if (status == PP_OK) {
    status = record_dependencies(production, &proxy_id, &asset_id, &original_id,
                                 &error);
  }
  if (status == PP_OK) {
    status =
        read_dependency_set(production, &proxy_id, &dependency_status, &error);
  }
  if (status == PP_OK && dependency_status != PP_DEPENDENCY_SET_CURRENT) {
    status = PP_ERROR_INTERNAL;
  }
  /* A new observation of the source means its dependencies need extraction. */
  uint8_t proxy_value[8];
  if (status == PP_OK && !host_fingerprint(proxy, proxy_value)) {
    status = PP_ERROR_IO;
  }
  if (status == PP_OK) {
    status = observe_source(production, &proxy_id, proxy_value, &error);
  }
  if (status == PP_OK) {
    status =
        read_dependency_set(production, &proxy_id, &dependency_status, &error);
  }
  if (status == PP_OK &&
      dependency_status != PP_DEPENDENCY_SET_NEEDS_EXTRACTION) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = count_dependencies_by_page(production, &proxy_id, &count, &error);
  }
  if (status == PP_OK && count < 2) {
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
