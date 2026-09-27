/* Runs every C listing included in the PostProject integrator guides.
 *
 * Each "[name]" ... "[/name]" region is included verbatim by the documentation
 * build, so keep regions self-contained and readable. Usage:
 *   postproject-c-guides WORK_DIRECTORY
 * The work directory is prepared by prepare-workdir.cmake.
 */
#include <postproject/postproject.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* [create-production] */
static pp_error_code_t create_production(const char *path, const char *media,
                                         pp_production_t **out_production,
                                         pp_uuid_t *out_asset_id,
                                         pp_error_t **error) {
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_representation_set_t *representations = NULL;

  pp_error_code_t status =
      pp_production_create(path, "Documentary", &production, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_set_revision_context(
        transaction, "com.example.editor", "0.4.0", NULL,
        "Import camera original", error);
  }
  if (status == PP_OK) {
    status = pp_transaction_import_media(transaction, media, "Camera A",
                                         out_asset_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_representations(production, out_asset_id,
                                           &representations, error);
  }
  if (status == PP_OK) {
    printf("representations: %llu\n",
           (unsigned long long)pp_representation_set_count(representations));
    *out_production = production;
    production = NULL;
  }

  pp_representation_set_release(representations);
  pp_transaction_release(transaction);
  pp_production_release(production);
  return status;
}
/* [/create-production] */

/* [external-identifiers] */
static pp_error_code_t tag_camera_serial(pp_production_t *production,
                                         const pp_uuid_t *asset_id,
                                         pp_error_t **error) {
  const pp_object_ref_t target = {PP_OBJECT_ASSET, *asset_id};
  pp_transaction_t *transaction = NULL;
  pp_external_identifier_set_t *attached = NULL;
  pp_object_ref_set_t *matches = NULL;

  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_add_external_identifier(
        transaction, &target, "com.example.camera.serial", "A-0007", NULL,
        error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_external_identifiers(production, &target,
                                                &attached, error);
  }
  if (status == PP_OK) {
    status = pp_production_find_by_external_identifier(
        production, "com.example.camera.serial", "A-0007", NULL, &matches,
        error);
  }
  if (status == PP_OK) {
    printf("identifiers: %llu, matching objects: %llu\n",
           (unsigned long long)pp_external_identifier_set_count(attached),
           (unsigned long long)pp_object_ref_set_count(matches));
  }

  pp_object_ref_set_release(matches);
  pp_external_identifier_set_release(attached);
  pp_transaction_release(transaction);
  return status;
}
/* [/external-identifiers] */

/* [metadata] */
static const char *const IPTC_VIDEO_METADATA_HUB =
    "https://iptc.org/std/videometadatahub/recommendation/"
    "iptc-vmhub-1.7-schema.json";

static pp_error_code_t add_title(pp_production_t *production,
                                 const pp_uuid_t *asset_id,
                                 pp_error_t **error) {
  const pp_object_ref_t target = {PP_OBJECT_ASSET, *asset_id};
  pp_metadata_input_t *title = NULL;
  pp_transaction_t *transaction = NULL;
  pp_metadata_set_t *on_asset = NULL;
  pp_metadata_set_t *everywhere = NULL;

  pp_error_code_t status =
      pp_metadata_input_create_string("Interview", "en-US", &title, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_metadata_value(
        transaction, &target, IPTC_VIDEO_METADATA_HUB, "title", title, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_metadata(production, &target, &on_asset, error);
  }
  if (status == PP_OK) {
    status = pp_production_find_metadata(production, IPTC_VIDEO_METADATA_HUB,
                                         "title", &everywhere, error);
  }
  for (uint64_t index = 0;
       status == PP_OK && index < pp_metadata_set_count(on_asset); ++index) {
    pp_object_ref_t owner;
    const char *vocabulary = NULL;
    const char *property = NULL;
    const pp_metadata_value_t *value = NULL;
    const char *text = NULL;
    const char *language = NULL;
    status = pp_metadata_set_get(on_asset, index, &owner, &vocabulary,
                                 &property, &value, error);
    if (status == PP_OK &&
        pp_metadata_value_kind(value) == PP_METADATA_LANG_STRING) {
      status = pp_metadata_value_get_string(value, &text, &language, error);
      if (status == PP_OK) {
        printf("%s: %s [%s]\n", property, text, language);
      }
    }
  }

  pp_metadata_set_release(everywhere);
  pp_metadata_set_release(on_asset);
  pp_transaction_release(transaction);
  pp_metadata_input_release(title);
  return status;
}
/* [/metadata] */

/* [media-root] */
static pp_error_code_t add_rushes_root(pp_production_t *production,
                                       pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_uuid_t root_id;

  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_add_media_root(transaction, "rushes",
                                           "Camera originals", 0, &root_id,
                                           error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }

  pp_transaction_release(transaction);
  return status;
}
/* [/media-root] */

/* [resolve-asset] */
static pp_error_code_t resolve_asset(const pp_production_t *production,
                                     const pp_uuid_t *asset_id,
                                     const char *rushes_directory,
                                     pp_resolution_set_t **out_resolutions,
                                     pp_error_t **error) {
  pp_resolution_options_t *options = NULL;
  pp_resolution_set_t *resolutions = NULL;

  /* The mapping locates the logical root on this machine for this call only. */
  pp_error_code_t status = pp_resolution_options_create(&options, error);
  if (status == PP_OK) {
    status = pp_resolution_options_add_root_mapping(options, "rushes",
                                                    rushes_directory, error);
  }
  if (status == PP_OK) {
    status = pp_production_resolve_assets(production, asset_id, 1, options,
                                          &resolutions, error);
  }
  pp_resolution_options_release(options);
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
    if (status == PP_OK) {
      printf("availability: %u\n", availability);
    }
    for (uint64_t s = 0; status == PP_OK && s < resource_count; ++s) {
      pp_uuid_t resource_id;
      pp_resource_resolution_state_t state;
      uint64_t candidate_count = 0;
      uint64_t evidence_count = 0;
      status = pp_resolution_set_get_resource(resolutions, r, s, &resource_id,
                                              &state, &candidate_count,
                                              &evidence_count, error);
      for (uint64_t c = 0; status == PP_OK && c < candidate_count; ++c) {
        const char *uri = NULL;
        uint16_t confidence = 0;
        const char *root = NULL;
        status = pp_resolution_set_get_candidate(resolutions, r, s, c, &uri,
                                                 &confidence, &root,
                                                 &evidence_count, error);
        if (status == PP_OK) {
          printf("candidate: %s (%u/10000) under %s\n", uri, confidence,
                 root != NULL ? root : "no root");
        }
      }
    }
  }

  if (status == PP_OK) {
    *out_resolutions = resolutions;
  } else {
    pp_resolution_set_release(resolutions);
  }
  return status;
}
/* [/resolve-asset] */

/* [confirm-locator] */
static pp_error_code_t
confirm_unique_candidates(pp_production_t *production,
                          const pp_resolution_set_t *resolutions,
                          pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);

  const uint64_t count = pp_resolution_set_representation_count(resolutions);
  for (uint64_t r = 0; status == PP_OK && r < count; ++r) {
    pp_uuid_t asset_id;
    pp_uuid_t representation_id;
    pp_representation_availability_t availability;
    uint64_t resource_count = 0;
    uint64_t issue_count = 0;
    status = pp_resolution_set_get_representation(
        resolutions, r, &asset_id, &representation_id, &availability,
        &resource_count, &issue_count, error);
    for (uint64_t s = 0; status == PP_OK && s < resource_count; ++s) {
      pp_uuid_t resource_id;
      pp_resource_resolution_state_t state;
      uint64_t candidate_count = 0;
      uint64_t evidence_count = 0;
      const char *uri = NULL;
      uint16_t confidence = 0;
      /* The logical root the candidate was found under, if any. */
      const char *root = NULL;
      status = pp_resolution_set_get_resource(resolutions, r, s, &resource_id,
                                              &state, &candidate_count,
                                              &evidence_count, error);
      /* Several candidates need a person to choose; never pick one here. */
      if (status == PP_OK && candidate_count == 1) {
        status = pp_resolution_set_get_candidate(resolutions, r, s, 0, &uri,
                                                 &confidence, &root,
                                                 &evidence_count, error);
      }
      if (status == PP_OK && uri != NULL && root != NULL) {
        status = pp_transaction_confirm_locator_under_root(
            transaction, &resource_id, uri, root, error);
      } else if (status == PP_OK && uri != NULL) {
        status = pp_transaction_confirm_locator(transaction, &resource_id, uri,
                                                error);
      }
    }
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }

  pp_transaction_release(transaction);
  return status;
}
/* [/confirm-locator] */

/* [image-sequence] */
static pp_error_code_t add_render_sequence(pp_production_t *production,
                                           const pp_uuid_t *asset_id,
                                           const char *directory,
                                           pp_uuid_t *out_sequence_id,
                                           pp_error_t **error) {
  const int64_t missing_frames[] = {1003};
  pp_transaction_t *transaction = NULL;
  pp_representation_set_t *representations = NULL;

  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_add_image_sequence_representation(
        transaction, asset_id, PP_REPRESENTATION_DERIVED, directory,
        "shot010.", ".exr", 4, 1001, 1004, 1, 24000, 1001, missing_frames, 1,
        out_sequence_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_representations(production, asset_id,
                                           &representations, error);
  }
  for (uint64_t index = 0;
       status == PP_OK && index < pp_representation_set_count(representations);
       ++index) {
    pp_uuid_t id;
    pp_uuid_t owner;
    pp_representation_kind_t kind;
    pp_content_structure_kind_t structure;
    uint64_t members = 0;
    uint64_t resources = 0;
    uint64_t fingerprints = 0;
    status = pp_representation_set_get(representations, index, &id, &owner,
                                       &kind, &structure, &members, &resources,
                                       &fingerprints, error);
    if (status == PP_OK && structure == PP_CONTENT_IMAGE_SEQUENCE) {
      const char *prefix = NULL;
      const char *suffix = NULL;
      uint8_t padding = 0;
      int64_t start = 0;
      int64_t end = 0;
      uint32_t step = 0;
      uint32_t rate_numerator = 0;
      uint32_t rate_denominator = 0;
      uint64_t missing_count = 0;
      status = pp_representation_set_get_sequence(
          representations, index, &prefix, &suffix, &padding, &start, &end,
          &step, &rate_numerator, &rate_denominator, &missing_count, error);
      if (status == PP_OK) {
        printf("%s#%s frames %lld-%lld, %llu known missing\n", prefix, suffix,
               (long long)start, (long long)end,
               (unsigned long long)missing_count);
      }
    }
  }

  pp_representation_set_release(representations);
  pp_transaction_release(transaction);
  return status;
}
/* [/image-sequence] */

/* [provenance] */
static pp_error_code_t record_render(pp_production_t *production,
                                     const pp_uuid_t *source_id,
                                     const pp_uuid_t *render_id,
                                     pp_error_t **error) {
  const pp_activity_edge_t inputs[] = {{*source_id, "org.postproject:primary"}};
  const pp_activity_edge_t outputs[] = {{*render_id, NULL}};
  pp_transaction_t *transaction = NULL;
  pp_activity_set_t *producers = NULL;
  pp_object_ref_set_t *ancestors = NULL;
  pp_uuid_t activity_id;

  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_create_activity(
        transaction, "org.postproject:render", inputs, 1, outputs, 1, NULL,
        NULL, "Example Renderer", "2.1", "https://example.com/renderer", NULL,
        NULL, NULL, NULL, &activity_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_activities_producing(production, render_id,
                                                &producers, error);
  }
  if (status == PP_OK) {
    status = pp_production_provenance_ancestors(production, render_id,
                                                &ancestors, error);
  }
  if (status == PP_OK) {
    printf("producers: %llu, ancestors: %llu\n",
           (unsigned long long)pp_activity_set_count(producers),
           (unsigned long long)pp_object_ref_set_count(ancestors));
  }

  pp_object_ref_set_release(ancestors);
  pp_activity_set_release(producers);
  pp_transaction_release(transaction);
  return status;
}
/* [/provenance] */

/* [artifact-knowledge] */
static pp_error_code_t inspect_artifact(const pp_production_t *production,
                                        const pp_uuid_t *artifact_id,
                                        pp_error_t **error) {
  pp_artifact_evaluation_t *evaluation = NULL;
  pp_artifact_reproducibility_t *report = NULL;
  pp_uuid_t evaluated_id;
  pp_artifact_knowledge_state_t state = 0;
  uint32_t visited = 0;
  uint8_t truncated = 0;
  uint64_t reason_count = 0;
  pp_error_code_t status = pp_production_evaluate_artifact(
      production, artifact_id, UINT32_C(64), UINT32_C(1000), &evaluation,
      error);
  if (status == PP_OK) {
    status = pp_artifact_evaluation_get(
        evaluation, &evaluated_id, &state, &visited, &truncated, &reason_count,
        error);
  }
  if (status == PP_OK) {
    printf("artifact state: %u, reasons: %llu\n", state,
           (unsigned long long)reason_count);
    status = pp_production_artifact_reproducibility(
        production, artifact_id, &report, error);
  }
  if (status == PP_OK) {
    uint8_t reproducible = 0;
    uint8_t has_activity = 0;
    pp_uuid_t activity_id;
    const char *activity_kind = NULL;
    uint64_t issue_count = 0;
    status = pp_artifact_reproducibility_get(
        report, &evaluated_id, &reproducible, &has_activity, &activity_id,
        &activity_kind, &issue_count, error);
    if (status == PP_OK) {
      printf("reproducible: %u, missing conditions: %llu\n", reproducible,
             (unsigned long long)issue_count);
    }
  }

  pp_artifact_reproducibility_release(report);
  pp_artifact_evaluation_release(evaluation);
  return status;
}
/* [/artifact-knowledge] */

/* [dependency-queries] */
static pp_error_code_t
record_and_query_dependencies(pp_production_t *production,
                              const pp_uuid_t *source_id,
                              const pp_uuid_t *target_asset_id,
                              const pp_uuid_t *resolved_id,
                              pp_error_t **error) {
  pp_dependency_t dependency = {0};
  dependency.kind = "org.example:character-reference";
  dependency.target.kind = PP_OBJECT_ASSET;
  dependency.target.id = *target_asset_id;
  dependency.has_resolved_representation = UINT8_C(1);
  dependency.resolved_representation_id = *resolved_id;
  dependency.required = UINT8_C(1);
  dependency.authored_reference = "characters/lead.usd";

  pp_transaction_t *transaction = NULL;
  pp_dependency_query_set_t *dependencies = NULL;
  pp_dependency_query_set_t *dependents = NULL;
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_record_dependency_set(
        transaction, source_id, &dependency, UINT64_C(1), error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_dependencies(production, source_id, UINT32_C(4),
                                        UINT32_C(1000), UINT32_C(100), NULL,
                                        &dependencies, error);
  }
  for (uint64_t i = 0;
       status == PP_OK && i < pp_dependency_query_set_count(dependencies);
       ++i) {
    pp_dependency_match_t match;
    status = pp_dependency_query_set_get(dependencies, i, &match, error);
    if (status == PP_OK) {
      printf("dependency at depth %u\n", match.depth);
    }
  }
  const pp_object_ref_t target = {PP_OBJECT_ASSET, *target_asset_id};
  if (status == PP_OK) {
    status = pp_production_dependents(production, &target, UINT32_C(4),
                                      UINT32_C(1000), UINT32_C(100), NULL,
                                      &dependents, error);
  }
  if (status == PP_OK &&
      (pp_dependency_query_set_traversal_truncated(dependencies) != 0 ||
       pp_dependency_query_set_count(dependents) != UINT64_C(1))) {
    status = PP_ERROR_INTERNAL;
  }

  pp_dependency_query_set_release(dependents);
  pp_dependency_query_set_release(dependencies);
  pp_transaction_release(transaction);
  return status;
}
/* [/dependency-queries] */

/* [job-query-pages] */
static pp_error_code_t request_and_page_jobs(pp_production_t *production,
                                             const pp_uuid_t *input_id,
                                             const pp_uuid_t *output_asset_id,
                                             pp_error_t **error) {
  const char *kind = "org.example:generate-proxy";
  pp_transaction_t *transaction = NULL;
  pp_uuid_t job_ids[2];
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  for (uint32_t i = 0; status == PP_OK && i < UINT32_C(2); ++i) {
    status = pp_transaction_request_job(
        transaction, kind, input_id, UINT64_C(1), output_asset_id,
        PP_REPRESENTATION_PROXY, NULL, &job_ids[i], error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_job_set_t *first_job = NULL;
  pp_job_t job = {0};
  if (status == PP_OK) {
    status = pp_production_job(production, &job_ids[0], &first_job, error);
  }
  if (status == PP_OK) {
    status = pp_job_set_get(first_job, UINT64_C(0), &job, error);
  }
  if (status == PP_OK && job.state != PP_JOB_REQUESTED) {
    status = PP_ERROR_INTERNAL;
  }
  pp_job_set_release(first_job);

  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  uint64_t count = 0;
  while (status == PP_OK) {
    pp_job_set_t *page = NULL;
    status = pp_production_jobs(production, PP_JOB_REQUESTED, kind,
                                UINT32_C(1), cursor, &page, error);
    count += status == PP_OK ? pp_job_set_count(page) : 0;
    const char *next =
        status == PP_OK ? pp_job_set_next_cursor(page) : NULL;
    if (next == NULL) {
      pp_job_set_release(page);
      break;
    }
    const int copied = snprintf(cursor_storage, sizeof cursor_storage, "%s", next);
    pp_job_set_release(page);
    if (copied < 0 || (size_t)copied >= sizeof cursor_storage) {
      status = PP_ERROR_INTERNAL;
      break;
    }
    cursor = cursor_storage;
  }
  if (status == PP_OK && count != UINT64_C(2)) {
    status = PP_ERROR_INTERNAL;
  }

  pp_transaction_release(transaction);
  return status;
}
/* [/job-query-pages] */

/* [media-structure-pages] */
static pp_error_code_t
print_resource_locators(const pp_production_t *production,
                        const pp_uuid_t *resource_id,
                        pp_error_t **error) {
  pp_locator_query_set_t *locators = NULL;
  pp_error_code_t status = pp_production_locators_page(
      production, resource_id, UINT32_C(100), NULL, &locators, error);
  for (uint64_t i = 0;
       status == PP_OK && i < pp_locator_query_set_count(locators); ++i) {
    pp_uuid_t locator_id;
    pp_uuid_t owner_id;
    const char *uri = NULL;
    pp_locator_availability_t availability;
    uint8_t has_last_seen = 0;
    int64_t last_seen = 0;
    const char *media_root = NULL;
    status = pp_locator_query_set_get(locators, i, &locator_id, &owner_id, &uri,
                                      &availability, &has_last_seen,
                                      &last_seen, &media_root, error);
    if (status == PP_OK) {
      printf("%s (root: %s)\n", uri, media_root != NULL ? media_root : "-");
    }
  }
  pp_locator_query_set_release(locators);
  return status;
}

static pp_error_code_t print_asset_locators(const pp_production_t *production,
                                            const pp_uuid_t *asset_id,
                                            pp_error_t **error) {
  /* Follow each nested cursor the same way in large productions. */
  pp_representation_set_t *representations = NULL;
  pp_error_code_t status = pp_production_representations_page(
      production, asset_id, UINT32_C(100), NULL, &representations, error);
  for (uint64_t r = 0;
       status == PP_OK && r < pp_representation_set_count(representations);
       ++r) {
    pp_uuid_t representation_id;
    pp_uuid_t owner;
    pp_representation_kind_t kind;
    pp_content_structure_kind_t structure;
    uint64_t members, resources, fingerprints;
    pp_object_query_set_t *resource_page = NULL;
    status = pp_representation_set_get(representations, r, &representation_id,
                                       &owner, &kind, &structure, &members,
                                       &resources, &fingerprints, error);
    if (status == PP_OK) {
      status = pp_production_resources_page(production, &representation_id,
                                            UINT32_C(100), NULL,
                                            &resource_page, error);
    }
    for (uint64_t s = 0;
         status == PP_OK && s < pp_object_query_set_count(resource_page); ++s) {
      pp_object_ref_t resource;
      uint32_t depth = 0;
      status =
          pp_object_query_set_get(resource_page, s, &resource, &depth, error);
      if (status == PP_OK) {
        status = print_resource_locators(production, &resource.id, error);
      }
    }
    pp_object_query_set_release(resource_page);
  }
  pp_representation_set_release(representations);
  return status;
}

static pp_error_code_t
print_recorded_locators(const pp_production_t *production,
                        pp_error_t **error) {
  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  pp_error_code_t status = PP_OK;
  while (status == PP_OK) {
    pp_asset_set_t *assets = NULL;
    status = pp_production_assets_page(production, UINT32_C(100), cursor,
                                       &assets, error);
    for (uint64_t i = 0; status == PP_OK && i < pp_asset_set_count(assets);
         ++i) {
      pp_uuid_t asset_id;
      int64_t created_at = 0;
      const char *name = NULL;
      const char *import_source = NULL;
      status = pp_asset_set_get(assets, i, &asset_id, &created_at, &name,
                                &import_source, error);
      if (status == PP_OK) {
        status = print_asset_locators(production, &asset_id, error);
      }
    }
    /* The cursor borrows the page; copy it before releasing the page. */
    const char *next =
        status == PP_OK ? pp_asset_set_next_cursor(assets) : NULL;
    const int copied =
        next != NULL
            ? snprintf(cursor_storage, sizeof cursor_storage, "%s", next)
            : 0;
    pp_asset_set_release(assets);
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
/* [/media-structure-pages] */

/* [knowledge-only-media] */
static pp_error_code_t list_media_knowledge(const pp_production_t *production,
                                            uint64_t *out_under_rushes,
                                            pp_error_t **error) {
  /* Both queries read recorded knowledge; neither touches the filesystem. */
  pp_object_query_set_t *unresolved = NULL;
  pp_representation_set_t *under_rushes = NULL;
  pp_error_code_t status = pp_production_unresolved_media(
      production, UINT32_C(100), NULL, &unresolved, error);
  if (status == PP_OK) {
    printf("representations without a recorded locator: %llu\n",
           (unsigned long long)pp_object_query_set_count(unresolved));
    status = pp_production_representations_under_media_root(
        production, "rushes", UINT32_C(100), NULL, &under_rushes, error);
  }
  if (status == PP_OK) {
    *out_under_rushes = pp_representation_set_count(under_rushes);
  }

  pp_representation_set_release(under_rushes);
  pp_object_query_set_release(unresolved);
  return status;
}
/* [/knowledge-only-media] */

/* [point-reads] */
static pp_error_code_t read_known_objects(const pp_production_t *production,
                                          const pp_uuid_t *asset_id,
                                          const pp_uuid_t *representation_id,
                                          pp_error_t **error) {
  /* A host reference names one object; read it without scanning the
   * production. Point reads return one-element sets. */
  pp_asset_set_t *asset = NULL;
  pp_representation_set_t *representation = NULL;
  pp_representation_set_t *users = NULL;
  pp_uuid_t id;
  int64_t created_at = 0;
  const char *name = NULL;
  const char *source = NULL;
  pp_uuid_t resource_id;
  const char *role = NULL;
  uint8_t required = 0;
  pp_error_code_t status =
      pp_production_asset(production, asset_id, &asset, error);
  if (status == PP_OK) {
    status = pp_asset_set_get(asset, UINT64_C(0), &id, &created_at, &name,
                              &source, error);
  }
  if (status == PP_OK) {
    printf("asset: %s\n", name != NULL ? name : "unnamed");
    status = pp_production_representation(production, representation_id,
                                          &representation, error);
  }
  if (status == PP_OK) {
    status = pp_representation_set_get_member(representation, UINT64_C(0),
                                              UINT64_C(0), &resource_id, &role,
                                              &required, error);
  }
  if (status == PP_OK) {
    status = pp_production_representations_using_resource(
        production, &resource_id, UINT32_C(100), NULL, &users, error);
  }
  if (status == PP_OK) {
    printf("representations using the resource: %llu\n",
           (unsigned long long)pp_representation_set_count(users));
  }
  pp_representation_set_release(users);
  pp_representation_set_release(representation);
  pp_asset_set_release(asset);
  return status;
}
/* [/point-reads] */

/* [metadata-query-pages] */
static pp_error_code_t find_interview_titles(const pp_production_t *production,
                                             uint64_t *out_count,
                                             pp_error_t **error) {
  pp_metadata_input_t *interview = NULL;
  pp_metadata_set_t *page = NULL;
  pp_error_code_t status =
      pp_metadata_input_create_string("Interview", "en-US", &interview, error);
  if (status == PP_OK) {
    /* Pass NULL instead of an exact value to match every value. */
    status = pp_production_query_metadata(
        production,
        "https://iptc.org/std/videometadatahub/recommendation/"
        "iptc-vmhub-1.7-schema.json",
        "title", interview, UINT32_C(100), NULL, &page, error);
  }
  if (status == PP_OK) {
    *out_count = pp_metadata_set_count(page);
    printf("exact title matches: %llu\n", (unsigned long long)*out_count);
  }

  pp_metadata_set_release(page);
  pp_metadata_input_release(interview);
  return status;
}
/* [/metadata-query-pages] */

/* [provenance-query-pages] */
static pp_error_code_t query_render_lineage(const pp_production_t *production,
                                            const pp_uuid_t *source_id,
                                            const pp_uuid_t *render_id,
                                            pp_error_t **error) {
  pp_activity_set_t *producing = NULL;
  pp_activity_set_t *consuming = NULL;
  pp_object_query_set_t *by_kind = NULL;
  pp_object_query_set_t *by_tool = NULL;
  pp_object_query_set_t *ancestors = NULL;
  pp_error_code_t status = pp_production_activities_producing_page(
      production, render_id, UINT32_C(100), NULL, &producing, error);
  if (status == PP_OK) {
    status = pp_production_activities_consuming_page(
        production, source_id, UINT32_C(100), NULL, &consuming, error);
  }
  if (status == PP_OK) {
    status = pp_production_outputs_by_activity_kind(
        production, "org.postproject:render", UINT32_C(100), NULL, &by_kind,
        error);
  }
  if (status == PP_OK) {
    /* The tool identity matches exactly; NULL matches an absent field. */
    status = pp_production_outputs_by_tool(
        production, "Example Renderer", "2.1", "https://example.com/renderer",
        UINT32_C(100), NULL, &by_tool, error);
  }
  if (status == PP_OK) {
    status = pp_production_provenance_ancestors_page(
        production, render_id, UINT32_C(8), UINT32_C(1000), UINT32_C(100), NULL,
        &ancestors, error);
  }
  for (uint64_t i = 0;
       status == PP_OK && i < pp_object_query_set_count(ancestors); ++i) {
    pp_object_ref_t ancestor;
    uint32_t depth = 0;
    status = pp_object_query_set_get(ancestors, i, &ancestor, &depth, error);
    if (status == PP_OK) {
      printf("ancestor at depth %u\n", depth);
    }
  }
  if (status == PP_OK &&
      (pp_activity_set_count(producing) != UINT64_C(1) ||
       pp_activity_set_count(consuming) != UINT64_C(1) ||
       pp_object_query_set_count(by_kind) != UINT64_C(1) ||
       pp_object_query_set_count(by_tool) != UINT64_C(1) ||
       pp_object_query_set_traversal_truncated(ancestors) != 0)) {
    status = PP_ERROR_INTERNAL;
  }

  pp_object_query_set_release(ancestors);
  pp_object_query_set_release(by_tool);
  pp_object_query_set_release(by_kind);
  pp_activity_set_release(consuming);
  pp_activity_set_release(producing);
  return status;
}
/* [/provenance-query-pages] */

/* [stale-artifact-pages] */
static pp_error_code_t
count_stale_descendants(const pp_production_t *production,
                        const pp_uuid_t *source_id,
                        uint64_t *out_count,
                        pp_error_t **error) {
  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  pp_error_code_t status = PP_OK;
  *out_count = 0;
  while (status == PP_OK) {
    pp_object_query_set_t *page = NULL;
    status = pp_production_stale_artifacts(production, source_id, UINT32_C(64),
                                           UINT32_C(1000), UINT32_C(100),
                                           cursor, &page, error);
    /* A page bounds the candidates examined, so it may hold fewer stale
     * results, or none, and still carry a continuation. */
    *out_count += status == PP_OK ? pp_object_query_set_count(page) : 0;
    const char *next =
        status == PP_OK ? pp_object_query_set_next_cursor(page) : NULL;
    const int copied =
        next != NULL
            ? snprintf(cursor_storage, sizeof cursor_storage, "%s", next)
            : 0;
    pp_object_query_set_release(page);
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
/* [/stale-artifact-pages] */

/* [changed-objects] */
static pp_error_code_t
object_changed_after(const pp_production_t *production, uint64_t sequence,
                     const pp_object_ref_t *object, uint8_t *out_changed,
                     pp_error_t **error) {
  char cursor_storage[2049] = {0};
  const char *cursor = NULL;
  pp_error_code_t status = PP_OK;
  *out_changed = 0;
  while (status == PP_OK) {
    pp_object_query_set_t *page = NULL;
    status = pp_production_objects_changed_since(
        production, sequence, UINT32_C(100), cursor, &page, error);
    for (uint64_t i = 0;
         status == PP_OK && i < pp_object_query_set_count(page); ++i) {
      pp_object_ref_t changed;
      uint32_t depth = 0;
      status = pp_object_query_set_get(page, i, &changed, &depth, error);
      if (status == PP_OK && changed.kind == object->kind &&
          memcmp(&changed.id, &object->id, sizeof changed.id) == 0) {
        *out_changed = 1;
      }
    }
    const char *next =
        status == PP_OK ? pp_object_query_set_next_cursor(page) : NULL;
    const int copied =
        next != NULL
            ? snprintf(cursor_storage, sizeof cursor_storage, "%s", next)
            : 0;
    pp_object_query_set_release(page);
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
/* [/changed-objects] */

static void handle_event(const pp_revision_event_t *event) {
  printf("event %u: kind %u\n", event->position, event->kind);
}

/* [revision-feed] */
static pp_error_code_t process_changes(const pp_production_t *production,
                                       uint64_t *cursor, pp_error_t **error) {
  const uint32_t limit = 100;
  pp_error_code_t status = PP_OK;
  uint64_t page_size = limit;

  while (status == PP_OK && page_size == limit) {
    pp_revision_set_t *page = NULL;
    status = pp_production_changes_since(production, *cursor, limit, &page,
                                         error);
    page_size = status == PP_OK ? pp_revision_set_count(page) : 0;
    for (uint64_t i = 0; status == PP_OK && i < page_size; ++i) {
      pp_uuid_t revision_id;
      pp_uuid_t transaction_id;
      uint64_t sequence = 0;
      int64_t committed_at = 0;
      const char *origin_name, *origin_version, *origin_uri, *message;
      pp_revision_event_set_t *events = NULL;
      status = pp_revision_set_get(page, i, &revision_id, &sequence,
                                   &transaction_id, &committed_at,
                                   &origin_name, &origin_version, &origin_uri,
                                   &message, error);
      if (status == PP_OK) {
        status = pp_production_revision_events(production, &revision_id,
                                               &events, error);
      }
      for (uint64_t j = 0;
           status == PP_OK && j < pp_revision_event_set_count(events); ++j) {
        pp_revision_event_t event;
        status = pp_revision_event_set_get(events, j, &event, error);
        if (status == PP_OK) {
          /* Switch on event.kind; unused fields are zero or NULL. */
          handle_event(&event);
        }
      }
      pp_revision_event_set_release(events);
      if (status == PP_OK) {
        /* Persist the cursor only after the whole revision is processed. */
        *cursor = sequence;
      }
    }
    pp_revision_set_release(page);
  }
  return status;
}
/* [/revision-feed] */

/* [revision-filter] */
static pp_error_code_t new_media_revisions(const pp_production_t *production,
                                           uint64_t *cursor,
                                           uint64_t *matched,
                                           pp_error_t **error) {
  const pp_revision_event_kind_t kinds[] = {PP_REVISION_REPRESENTATION_ADDED,
                                            PP_REVISION_JOB_SUCCEEDED};
  pp_revision_set_t *page = NULL;
  uint64_t through = 0;
  pp_error_code_t status = pp_production_changes_since_filtered(
      production, *cursor, kinds, 2, 100, &page, &through, error);
  if (status == PP_OK) {
    *matched = pp_revision_set_count(page);
    /* Continue from the through sequence, which skips unrelated revisions. */
    *cursor = through;
  }
  pp_revision_set_release(page);
  return status;
}
/* [/revision-filter] */

/* [revision-wait] */
static pp_error_code_t wait_for_changes(const pp_production_t *production,
                                        uint64_t cursor, uint64_t *count,
                                        pp_error_t **error) {
  pp_revision_waiter_t *waiter = NULL;
  pp_revision_set_t *revisions = NULL;
  pp_revision_wait_result_t result = 0;
  pp_error_code_t status = pp_revision_waiter_create(production, &waiter, error);
  if (status == PP_OK) {
    /* Another thread may call pp_revision_waiter_cancel() to stop the wait. */
    status = pp_revision_waiter_wait(waiter, cursor, 100, 5000, &result,
                                     &revisions, error);
  }
  if (status == PP_OK) {
    /* Timed out, closed, and cancelled results return an empty set. */
    *count = result == PP_REVISION_WAIT_REVISIONS
                 ? pp_revision_set_count(revisions)
                 : 0;
  }
  pp_revision_set_release(revisions);
  pp_revision_waiter_release(waiter);
  return status;
}
/* [/revision-wait] */

/* [host-binding] */
static pp_error_code_t bind_representation(const pp_uuid_t *production_id,
                                           const pp_uuid_t *representation_id,
                                           pp_error_t **error) {
  const pp_object_ref_t object = {PP_OBJECT_REPRESENTATION,
                                  *representation_id};
  char *stored = NULL;
  pp_uuid_t parsed_production = {{0}};
  pp_object_ref_t parsed_object = {0, {{0}}};

  pp_error_code_t status =
      pp_host_binding_format(production_id, &object, &stored, error);
  if (status == PP_OK) {
    printf("binding: %s\n", stored);
    status = pp_host_binding_parse(stored, &parsed_production, &parsed_object,
                                   error);
  }
  if (status == PP_OK &&
      (memcmp(&parsed_production, production_id, sizeof *production_id) != 0 ||
       parsed_object.kind != object.kind ||
       memcmp(&parsed_object.id, &object.id, sizeof object.id) != 0)) {
    status = PP_ERROR_INTERNAL;
  }

  pp_string_release(stored);
  return status;
}
/* [/host-binding] */

static void join(char *buffer, size_t size, const char *directory,
                 const char *name) {
  snprintf(buffer, size, "%s/%s", directory, name);
}

static pp_error_code_t
original_representation(const pp_production_t *production,
                        const pp_uuid_t *asset_id, pp_uuid_t *out_id,
                        pp_error_t **error) {
  pp_representation_set_t *representations = NULL;
  pp_error_code_t status =
      pp_production_representations(production, asset_id, &representations,
                                    error);
  if (status == PP_OK) {
    pp_uuid_t owner;
    pp_representation_kind_t kind;
    pp_content_structure_kind_t structure;
    uint64_t members, resources, fingerprints;
    status = pp_representation_set_get(representations, 0, out_id, &owner,
                                       &kind, &structure, &members, &resources,
                                       &fingerprints, error);
  }
  pp_representation_set_release(representations);
  return status;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: postproject-c-guides WORK_DIRECTORY\n");
    return 2;
  }
  const char *work = argv[1];
  char production_path[4096];
  char media[4096];
  char moved_media[4096];
  char moved[4096];
  char renders[4096];
  join(production_path, sizeof production_path, work, "production.pproj");
  join(media, sizeof media, work, "rushes/A001.mov");
  join(moved_media, sizeof moved_media, work, "moved/A001.mov");
  join(moved, sizeof moved, work, "moved");
  join(renders, sizeof renders, work, "renders/shot010");

  pp_production_t *production = NULL;
  pp_resolution_set_t *resolutions = NULL;
  pp_error_t *error = NULL;
  pp_uuid_t asset_id;
  pp_uuid_t production_id;
  pp_uuid_t original_id;
  pp_uuid_t sequence_id = {{0}};
  uint64_t cursor = 0;
  uint64_t before_render = 0;
  uint64_t count = 0;
  uint8_t changed = 0;

  pp_error_code_t status = create_production(production_path, media,
                                             &production, &asset_id, &error);
  if (status == PP_OK) {
    status = original_representation(production, &asset_id, &original_id,
                                     &error);
  }
  if (status == PP_OK) {
    status = tag_camera_serial(production, &asset_id, &error);
  }
  if (status == PP_OK) {
    status = add_title(production, &asset_id, &error);
  }
  if (status == PP_OK) {
    status = add_rushes_root(production, &error);
  }
  if (status == PP_OK && rename(media, moved_media) != 0) {
    perror("move media");
    status = PP_ERROR_IO;
  }
  if (status == PP_OK) {
    status = resolve_asset(production, &asset_id, moved, &resolutions, &error);
  }
  if (status == PP_OK) {
    status = confirm_unique_candidates(production, resolutions, &error);
  }
  if (status == PP_OK) {
    status = process_changes(production, &before_render, &error);
  }
  if (status == PP_OK) {
    status = add_render_sequence(production, &asset_id, renders, &sequence_id,
                                 &error);
  }
  if (status == PP_OK) {
    status = record_render(production, &original_id, &sequence_id, &error);
  }
  if (status == PP_OK) {
    status = inspect_artifact(production, &sequence_id, &error);
  }
  if (status == PP_OK) {
    status = record_and_query_dependencies(
        production, &sequence_id, &asset_id, &original_id, &error);
  }
  if (status == PP_OK) {
    status = request_and_page_jobs(production, &original_id, &asset_id, &error);
  }
  if (status == PP_OK) {
    status = print_recorded_locators(production, &error);
  }
  if (status == PP_OK) {
    status = list_media_knowledge(production, &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(1)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = read_known_objects(production, &asset_id, &original_id, &error);
  }
  if (status == PP_OK) {
    status = find_interview_titles(production, &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(1)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = query_render_lineage(production, &original_id, &sequence_id,
                                  &error);
  }
  if (status == PP_OK) {
    status = count_stale_descendants(production, &original_id, &count, &error);
  }
  if (status == PP_OK && count != UINT64_C(0)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    const pp_object_ref_t sequence = {PP_OBJECT_REPRESENTATION, sequence_id};
    status = object_changed_after(production, before_render, &sequence,
                                  &changed, &error);
  }
  if (status == PP_OK && changed == 0) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = process_changes(production, &cursor, &error);
  }
  if (status == PP_OK && cursor == 0) {
    status = PP_ERROR_INTERNAL;
  }
  uint64_t filtered_cursor = 0;
  uint64_t matched = 0;
  if (status == PP_OK) {
    status = new_media_revisions(production, &filtered_cursor, &matched, &error);
  }
  if (status == PP_OK && (matched == 0 || filtered_cursor != cursor)) {
    status = PP_ERROR_INTERNAL;
  }
  uint64_t waited = 0;
  if (status == PP_OK) {
    status = wait_for_changes(production, 0, &waited, &error);
  }
  if (status == PP_OK && waited == 0) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = pp_production_id(production, &production_id, &error);
  }
  if (status == PP_OK) {
    status = bind_representation(&production_id, &sequence_id, &error);
  }

  if (status != PP_OK) {
    fprintf(stderr, "operation failed (%u): %s\n", status,
            error != NULL ? pp_error_message(error) : "no details");
  }
  pp_error_release(error);
  pp_resolution_set_release(resolutions);
  pp_production_release(production);
  return status == PP_OK ? EXIT_SUCCESS : EXIT_FAILURE;
}
