/* Runs the C listings for requested work, workers, and regeneration.
 *
 * Each "[name]" ... "[/name]" region is included verbatim by the documentation
 * build, so keep regions self-contained and readable. Usage:
 *   postproject-c-jobs WORK_DIRECTORY
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

/* Times are always supplied by the caller; these are fixed for the example. */
#define NOW INT64_C(1767225600000000) /* 2026-01-01T00:00:00Z */
#define MINUTE INT64_C(60000000)

static int write_file(const char *path, const char *contents) {
  FILE *file = fopen(path, "wb");
  if (file == NULL) {
    return 0;
  }
  const size_t length = strlen(contents);
  const int ok = fwrite(contents, 1, length, file) == length;
  return fclose(file) == 0 && ok;
}

/* Stands in for the real transcoder a worker would run. */
static int run_transcoder(const char *output_path) {
  return write_file(output_path, "proxy essence\n");
}

/* Rebuilds an input from a read value for the kinds this example uses. */
static pp_error_code_t
metadata_input_from_value(const pp_metadata_value_t *value,
                          pp_metadata_input_t **out_input, pp_error_t **error) {
  switch (pp_metadata_value_kind(value)) {
  case PP_METADATA_STRING:
  case PP_METADATA_LANG_STRING: {
    const char *text, *language;
    pp_error_code_t status =
        pp_metadata_value_get_string(value, &text, &language, error);
    return status == PP_OK ? pp_metadata_input_create_string(text, language,
                                                             out_input, error)
                           : status;
  }
  case PP_METADATA_U64: {
    uint64_t number = 0;
    pp_error_code_t status = pp_metadata_value_get_u64(value, &number, error);
    return status == PP_OK
               ? pp_metadata_input_create_u64(number, out_input, error)
               : status;
  }
  default:
    return PP_ERROR_UNSUPPORTED;
  }
}

/* [request-job] */
static const char *const PROXY_JOB = "org.postproject:generate-proxy";
static const char *const PROXY_PARAMETERS = "https://example.com/ns/proxy/1";

/* Parameters are ordinary typed metadata whose target is the job. */
static pp_error_code_t stage_proxy_parameters(pp_transaction_t *transaction,
                                              const pp_object_ref_t *target,
                                              pp_error_t **error) {
  pp_metadata_input_t *height = NULL;
  pp_metadata_input_t *codec = NULL;
  pp_error_code_t status = pp_metadata_input_create_u64(540, &height, error);
  if (status == PP_OK) {
    status =
        pp_metadata_input_create_string("prores-proxy", NULL, &codec, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_metadata_value(
        transaction, target, PROXY_PARAMETERS, "height", height, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_metadata_value(
        transaction, target, PROXY_PARAMETERS, "codec", codec, error);
  }
  pp_metadata_input_release(codec);
  pp_metadata_input_release(height);
  return status;
}

static pp_error_code_t request_proxy(pp_production_t *production,
                                     const pp_uuid_t *input_id,
                                     const pp_uuid_t *asset_id,
                                     pp_uuid_t *out_job_id,
                                     pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_job_set_t *jobs = NULL;
  /* Request the job and its parameters in one transaction. */
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_request_job(transaction, PROXY_JOB, input_id, 1,
                                        asset_id, PP_REPRESENTATION_PROXY,
                                        "proxies", out_job_id, error);
  }
  if (status == PP_OK) {
    const pp_object_ref_t job = {PP_OBJECT_JOB, *out_job_id};
    status = stage_proxy_parameters(transaction, &job, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status = pp_production_jobs(production, PP_JOB_REQUESTED, PROXY_JOB,
                                UINT32_C(100), NULL, &jobs, error);
  }
  for (uint64_t j = 0; status == PP_OK && j < pp_job_set_count(jobs); ++j) {
    pp_job_t job;
    status = pp_job_set_get(jobs, j, &job, error);
    if (status == PP_OK) {
      printf("%s job, state %u, output kind %u under root %s\n", job.kind,
             job.state, job.output_representation_kind,
             job.target_root != NULL ? job.target_root : "-");
    }
    for (uint64_t i = 0; status == PP_OK && i < job.input_count; ++i) {
      pp_uuid_t input;
      status = pp_job_set_get_input(jobs, j, i, &input, error);
    }
  }
  pp_job_set_release(jobs);
  pp_transaction_release(transaction);
  return status;
}
/* [/request-job] */

/* [claim-job] */
static pp_error_code_t claim_renew_release(pp_production_t *production,
                                           const pp_uuid_t *job_id,
                                           pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_uuid_t claim_id;
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    /* A five-minute lease; the token is usable only after commit. */
    status = pp_transaction_claim_job(
        transaction, job_id, "Example Transcoder", "3.2",
        "https://example.com/transcoder", "render-node-04", "com.example.host",
        "node-04", NULL, NOW, NOW + 5 * MINUTE, &claim_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  /* Renewal and release both require the claim token. */
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_renew_job_claim(transaction, job_id, &claim_id,
                                            NOW + 4 * MINUTE, NOW + 9 * MINUTE,
                                            error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  /* Releasing abandons the work without recording a failure. */
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status =
        pp_transaction_release_job_claim(transaction, job_id, &claim_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  return status;
}
/* [/claim-job] */

/* [complete-job] */
static pp_error_code_t
complete_proxy(pp_production_t *production, const pp_uuid_t *job_id,
               const pp_uuid_t *input_id, const pp_uuid_t *asset_id,
               const char *output_path, pp_uuid_t *out_proxy_id,
               pp_error_t **error) {
  const int64_t started = NOW + 10 * MINUTE;
  const int64_t finished = NOW + 12 * MINUTE;
  pp_transaction_t *transaction = NULL;
  pp_uuid_t claim_id;
  pp_uuid_t activity_id = {{0}};
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_claim_job(transaction, job_id, "Example Transcoder",
                                      "3.2", "https://example.com/transcoder",
                                      NULL, NULL, NULL, NULL, started,
                                      started + 5 * MINUTE, &claim_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;
  if (status == PP_OK && !run_transcoder(output_path)) {
    status = PP_ERROR_IO;
  }

  /* Stage the output and the activity, then complete in the same
   * transaction; never commit them earlier. */
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_single_file_representation(
        transaction, asset_id, PP_REPRESENTATION_PROXY, output_path,
        out_proxy_id, error);
  }
  if (status == PP_OK) {
    const pp_activity_edge_t inputs[] = {{*input_id, NULL}};
    const pp_activity_edge_t outputs[] = {{*out_proxy_id, NULL}};
    status = pp_transaction_create_activity(
        transaction, PROXY_JOB, inputs, 1, outputs, 1, &started, &finished,
        "Example Transcoder", "3.2", "https://example.com/transcoder", NULL,
        NULL, NULL, NULL, &activity_id, error);
  }
  if (status == PP_OK) {
    status =
        pp_transaction_complete_job(transaction, job_id, &claim_id, finished,
                                    out_proxy_id, &activity_id, error);
  }
  if (status == PP_OK) {
    /* Parameters on the activity make the artifact reproducible. */
    const pp_object_ref_t activity = {PP_OBJECT_ACTIVITY, activity_id};
    status = stage_proxy_parameters(transaction, &activity, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  return status;
}
/* [/complete-job] */

/* [fail-job] */
static pp_error_code_t fail_proxy(pp_production_t *production,
                                  const pp_uuid_t *job_id, pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_uuid_t claim_id;
  const int64_t now = NOW + 20 * MINUTE;
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_claim_job(transaction, job_id, "Example Transcoder",
                                      "3.2", NULL, NULL, NULL, NULL, NULL, now,
                                      now + 5 * MINUTE, &claim_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  /* The tool failed: record a bounded diagnostic and no output. */
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status =
        pp_transaction_fail_job(transaction, job_id, &claim_id, now + MINUTE,
                                "encoder exited with status 1", error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  return status;
}
/* [/fail-job] */

/* [cancel-job] */
static pp_error_code_t cancel(pp_production_t *production,
                              const pp_uuid_t *job_id, pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    /* Cancellation is administrative and needs no claim token. */
    status = pp_transaction_cancel_job(transaction, job_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  return status;
}
/* [/cancel-job] */

/* [plan-regeneration] */
static pp_error_code_t enqueue_regeneration(pp_production_t *production,
                                            const pp_uuid_t *artifact_id,
                                            pp_uuid_t *out_job_id,
                                            pp_error_t **error) {
  pp_regeneration_plan_set_t *plans = NULL;
  pp_job_set_t *planned = NULL;
  pp_metadata_set_t *parameters = NULL;
  pp_transaction_t *transaction = NULL;
  pp_uuid_t planned_artifact;
  pp_job_t job;
  pp_uuid_t inputs[16];
  memset(&job, 0, sizeof job);

  /* Planning is read-only; nothing is enqueued until you commit. */
  pp_error_code_t status = pp_production_plan_regeneration(
      production, artifact_id, 1, &plans, error);
  if (status == PP_OK && pp_regeneration_plan_set_count(plans) != 1) {
    status = PP_ERROR_NOT_FOUND;
  }
  if (status == PP_OK) {
    status = pp_regeneration_plan_set_get(plans, 0, &planned_artifact, &planned,
                                          &parameters, error);
  }
  if (status == PP_OK) {
    status = pp_job_set_get(planned, 0, &job, error);
  }
  if (status == PP_OK && job.input_count > 16) {
    status = PP_ERROR_UNSUPPORTED;
  }
  for (uint64_t i = 0; status == PP_OK && i < job.input_count; ++i) {
    status = pp_job_set_get_input(planned, 0, i, &inputs[i], error);
  }

  /* Enqueue the proposal explicitly, copying its parameters to the job. */
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_request_job(
        transaction, job.kind, inputs, job.input_count, &job.output_asset_id,
        job.output_representation_kind, job.target_root, out_job_id, error);
  }
  for (uint64_t i = 0; status == PP_OK && i < pp_metadata_set_count(parameters);
       ++i) {
    const pp_object_ref_t target = {PP_OBJECT_JOB, *out_job_id};
    pp_object_ref_t owner;
    const char *vocabulary, *property;
    const pp_metadata_value_t *value;
    pp_metadata_input_t *input = NULL;
    status = pp_metadata_set_get(parameters, i, &owner, &vocabulary, &property,
                                 &value, error);
    if (status == PP_OK) {
      status = metadata_input_from_value(value, &input, error);
    }
    if (status == PP_OK) {
      status = pp_transaction_add_metadata_value(
          transaction, &target, vocabulary, property, input, error);
    }
    pp_metadata_input_release(input);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }

  pp_transaction_release(transaction);
  pp_metadata_set_release(parameters);
  pp_job_set_release(planned);
  pp_regeneration_plan_set_release(plans);
  return status;
}
/* [/plan-regeneration] */

static void join(char *buffer, size_t size, const char *directory,
                 const char *name) {
  snprintf(buffer, size, "%s/%s", directory, name);
}

static pp_error_code_t create_production(const char *path, const char *media,
                                         pp_production_t **out_production,
                                         pp_uuid_t *out_asset_id,
                                         pp_uuid_t *out_original_id,
                                         pp_error_t **error) {
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_representation_set_t *set = NULL;
  pp_error_code_t status =
      pp_production_create(path, "Documentary", &production, error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_import_media(transaction, media, "Camera A",
                                         out_asset_id, error);
  }
  if (status == PP_OK) {
    /* A job's target root names an existing logical media root. */
    pp_uuid_t root_id;
    status = pp_transaction_add_media_root(transaction, "proxies",
                                           "Proxy storage", 0, &root_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  if (status == PP_OK) {
    status =
        pp_production_representations(production, out_asset_id, &set, error);
  }
  if (status == PP_OK) {
    pp_uuid_t owner;
    pp_representation_kind_t kind;
    pp_content_structure_kind_t structure;
    uint64_t members, resources, fingerprints;
    status = pp_representation_set_get(set, 0, out_original_id, &owner, &kind,
                                       &structure, &members, &resources,
                                       &fingerprints, error);
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

/* Reloads one job and checks its state and state-specific fields. */
static pp_error_code_t expect_job(const pp_production_t *production,
                                  const pp_uuid_t *job_id,
                                  pp_job_state_t expected,
                                  const pp_uuid_t *completion_representation,
                                  pp_error_t **error) {
  pp_job_set_t *jobs = NULL;
  int found = 0;
  pp_error_code_t status = pp_production_jobs(
      production, 0, NULL, UINT32_C(1000), NULL, &jobs, error);
  for (uint64_t i = 0; status == PP_OK && i < pp_job_set_count(jobs); ++i) {
    pp_job_t job;
    status = pp_job_set_get(jobs, i, &job, error);
    if (status != PP_OK || memcmp(&job.id, job_id, sizeof job.id) != 0) {
      continue;
    }
    found = job.state == expected;
    if (expected == PP_JOB_REQUESTED) {
      found = found && job.claim_tool_name == NULL;
    } else if (expected == PP_JOB_FAILED) {
      found =
          found && job.failure_diagnostic != NULL &&
          strcmp(job.failure_diagnostic, "encoder exited with status 1") == 0;
    } else if (expected == PP_JOB_SUCCEEDED) {
      const pp_uuid_t zero = {{0}};
      found =
          found &&
          memcmp(&job.completion_representation_id, completion_representation,
                 sizeof job.completion_representation_id) == 0 &&
          memcmp(&job.completion_activity_id, &zero, sizeof zero) != 0;
    }
  }
  pp_job_set_release(jobs);
  if (status == PP_OK && !found) {
    fprintf(stderr, "job is not in state %u\n", expected);
    status = PP_ERROR_INTERNAL;
  }
  return status;
}

static pp_error_code_t count_representations(const pp_production_t *production,
                                             const pp_uuid_t *asset_id,
                                             uint64_t *out_count,
                                             pp_error_t **error) {
  pp_representation_set_t *set = NULL;
  pp_error_code_t status =
      pp_production_representations(production, asset_id, &set, error);
  *out_count = status == PP_OK ? pp_representation_set_count(set) : 0;
  pp_representation_set_release(set);
  return status;
}

static pp_error_code_t count_job_parameters(const pp_production_t *production,
                                            const pp_uuid_t *job_id,
                                            uint64_t *out_count,
                                            pp_error_t **error) {
  const pp_object_ref_t target = {PP_OBJECT_JOB, *job_id};
  pp_metadata_set_t *metadata = NULL;
  pp_error_code_t status =
      pp_production_metadata(production, &target, &metadata, error);
  *out_count = status == PP_OK ? pp_metadata_set_count(metadata) : 0;
  pp_metadata_set_release(metadata);
  return status;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: postproject-c-jobs WORK_DIRECTORY\n");
    return 2;
  }
  const char *work = argv[1];
  char production_path[4096], media[4096], proxies[4096], output[4096];
  join(production_path, sizeof production_path, work, "jobs.pproj");
  join(media, sizeof media, work, "rushes/A001.mov");
  join(proxies, sizeof proxies, work, "proxies");
  join(output, sizeof output, work, "proxies/A001_proxy.mov");
  make_directory(proxies);

  pp_production_t *production = NULL;
  pp_error_t *error = NULL;
  pp_uuid_t asset_id, original_id, job_id, failed_id, cancelled_id;
  pp_uuid_t proxy_id, regeneration_id;
  uint64_t before = 0;
  uint64_t count = 0;

  pp_error_code_t status = create_production(
      production_path, media, &production, &asset_id, &original_id, &error);
  if (status == PP_OK) {
    status =
        request_proxy(production, &original_id, &asset_id, &job_id, &error);
  }
  if (status == PP_OK) {
    status = expect_job(production, &job_id, PP_JOB_REQUESTED, NULL, &error);
  }
  if (status == PP_OK) {
    status = count_job_parameters(production, &job_id, &count, &error);
  }
  if (status == PP_OK && count != 2) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = claim_renew_release(production, &job_id, &error);
  }
  if (status == PP_OK) {
    status = expect_job(production, &job_id, PP_JOB_REQUESTED, NULL, &error);
  }
  if (status == PP_OK) {
    status = complete_proxy(production, &job_id, &original_id, &asset_id,
                            output, &proxy_id, &error);
  }
  if (status == PP_OK) {
    status =
        expect_job(production, &job_id, PP_JOB_SUCCEEDED, &proxy_id, &error);
  }

  if (status == PP_OK) {
    status =
        request_proxy(production, &original_id, &asset_id, &failed_id, &error);
  }
  if (status == PP_OK) {
    status = count_representations(production, &asset_id, &before, &error);
  }
  if (status == PP_OK) {
    status = fail_proxy(production, &failed_id, &error);
  }
  if (status == PP_OK) {
    status = expect_job(production, &failed_id, PP_JOB_FAILED, NULL, &error);
  }
  if (status == PP_OK) {
    status = count_representations(production, &asset_id, &count, &error);
  }
  if (status == PP_OK && count != before) {
    status = PP_ERROR_INTERNAL;
  }

  if (status == PP_OK) {
    status = request_proxy(production, &original_id, &asset_id, &cancelled_id,
                           &error);
  }
  if (status == PP_OK) {
    status = cancel(production, &cancelled_id, &error);
  }
  if (status == PP_OK) {
    status =
        expect_job(production, &cancelled_id, PP_JOB_CANCELLED, NULL, &error);
  }

  if (status == PP_OK) {
    status =
        enqueue_regeneration(production, &proxy_id, &regeneration_id, &error);
  }
  if (status == PP_OK) {
    status = expect_job(production, &regeneration_id, PP_JOB_REQUESTED, NULL,
                        &error);
  }
  if (status == PP_OK) {
    status = count_job_parameters(production, &regeneration_id, &count, &error);
  }
  if (status == PP_OK && count != 2) {
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
