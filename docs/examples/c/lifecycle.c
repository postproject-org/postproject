/* Runs the C listings for the production and transaction lifecycle.
 *
 * Each "[name]" ... "[/name]" region is included verbatim by the documentation
 * build, so keep regions self-contained and readable. Usage:
 *   postproject-c-lifecycle WORK_DIRECTORY
 * The work directory is prepared by prepare-workdir.cmake.
 */
#include <postproject/postproject.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* [open-production] */
static pp_error_code_t open_production(const char *path,
                                       const pp_uuid_t *asset_id,
                                       pp_production_t **out_production,
                                       pp_error_t **error) {
  pp_production_t *production = NULL;
  pp_asset_set_t *assets = NULL;
  pp_revision_set_t *latest = NULL;
  pp_uuid_t production_id;
  pp_production_id_t parsed_id;
  char *identity_text = NULL;
  uint8_t exists = 0;

  pp_error_code_t status = pp_production_open(path, &production, error);
  if (status == PP_OK) {
    /* Typed parsing and formatting preserve ordinary UUID text. */
    status = pp_production_id_parse("00000000-0000-0000-0000-000000000001",
                                    &parsed_id, error);
  }
  if (status == PP_OK) {
    status = pp_production_id_format(parsed_id, &identity_text, error);
  }
  if (status == PP_OK &&
      strcmp(identity_text, "00000000-0000-0000-0000-000000000001") != 0) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    /* The production ID is stable for the lifetime of the file. */
    status = pp_production_id(production, &production_id, error);
  }
  if (status == PP_OK) {
    status = pp_production_asset_exists(production, asset_id, &exists, error);
  }
  if (status == PP_OK) {
    printf("asset recorded: %s\n", exists != 0 ? "yes" : "no");
    /* Whole-set enumeration; use pp_production_assets_page() when large. */
    status = pp_production_assets(production, &assets, error);
  }
  for (uint64_t i = 0; status == PP_OK && i < pp_asset_set_count(assets); ++i) {
    pp_uuid_t id;
    int64_t created_at = 0;
    const char *name = NULL;
    const char *import_source = NULL;
    status = pp_asset_set_get(assets, i, &id, &created_at, &name,
                              &import_source, error);
    if (status == PP_OK) {
      printf("asset: %s (from %s)\n", name != NULL ? name : "-",
             import_source != NULL ? import_source : "-");
    }
  }
  if (status == PP_OK) {
    status = pp_production_latest_revision(production, &latest, error);
  }
  if (status == PP_OK && pp_revision_set_count(latest) == 1) {
    pp_uuid_t revision_id;
    pp_uuid_t transaction_id;
    uint64_t sequence = 0;
    int64_t committed_at = 0;
    const char *origin_name, *origin_version, *origin_uri, *message;
    status = pp_revision_set_get(latest, 0, &revision_id, &sequence,
                                 &transaction_id, &committed_at, &origin_name,
                                 &origin_version, &origin_uri, &message, error);
    if (status == PP_OK) {
      printf("latest revision %llu: %s\n", (unsigned long long)sequence,
             message != NULL ? message : "-");
    }
  }
  if (status == PP_OK) {
    *out_production = production;
    production = NULL;
  }

  pp_revision_set_release(latest);
  pp_string_release(identity_text);
  pp_asset_set_release(assets);
  pp_production_release(production);
  return status;
}
/* [/open-production] */

/* [transaction-lifecycle] */
static pp_error_code_t commit_then_roll_back(pp_production_t *production,
                                             pp_error_t **error) {
  pp_transaction_t *transaction = NULL;
  pp_uuid_t root_id;

  /* Commit: every staged change becomes one durable revision. */
  pp_error_code_t status =
      pp_production_begin_transaction(production, &transaction, error);
  if (status == PP_OK) {
    status = pp_transaction_set_revision_context(
        transaction, "com.example.editor", "0.4.0", NULL, "Add archive root",
        error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_media_root(
        transaction, "archive", "Archive volume", 10, &root_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_commit(transaction, error);
  }
  pp_transaction_release(transaction);
  transaction = NULL;

  /* Roll back: staged changes are discarded and no revision is written. */
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_add_media_root(transaction, "scratch",
                                           "Scratch disk", 20, &root_id, error);
  }
  if (status == PP_OK) {
    status = pp_transaction_rollback(transaction, error);
  }

  pp_transaction_release(transaction);
  return status;
}
/* [/transaction-lifecycle] */

/* [error-handling] */
static int report_missing_production(const char *path) {
  pp_production_t *production = NULL;
  pp_error_t *error = NULL;

  const pp_error_code_t status = pp_production_open(path, &production, &error);
  /* The status and pp_error_code() agree; the message is for people. */
  const int not_found = status == PP_ERROR_NOT_FOUND && error != NULL &&
                        pp_error_code(error) == PP_ERROR_NOT_FOUND;
  if (error != NULL) {
    printf("open failed (%u): %s\n", pp_error_code(error),
           pp_error_message(error));
  }

  pp_error_release(error);
  pp_production_release(production);
  return not_found;
}
/* [/error-handling] */

static void join(char *buffer, size_t size, const char *directory,
                 const char *name) {
  snprintf(buffer, size, "%s/%s", directory, name);
}

static pp_error_code_t create_production(const char *path, const char *media,
                                         pp_uuid_t *out_asset_id,
                                         pp_error_t **error) {
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_media_source_t *camera = NULL;
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
  pp_transaction_release(transaction);
  pp_production_release(production);
  return status;
}

static pp_error_code_t latest_sequence(const pp_production_t *production,
                                       uint64_t *out_sequence,
                                       pp_error_t **error) {
  pp_revision_set_t *latest = NULL;
  pp_uuid_t revision_id;
  pp_uuid_t transaction_id;
  int64_t committed_at = 0;
  const char *origin_name, *origin_version, *origin_uri, *message;
  *out_sequence = 0;
  pp_error_code_t status =
      pp_production_latest_revision(production, &latest, error);
  if (status == PP_OK && pp_revision_set_count(latest) == 1) {
    status = pp_revision_set_get(latest, 0, &revision_id, out_sequence,
                                 &transaction_id, &committed_at, &origin_name,
                                 &origin_version, &origin_uri, &message, error);
  }
  pp_revision_set_release(latest);
  return status;
}

static pp_error_code_t media_root_names(const pp_production_t *production,
                                        int *out_has_archive,
                                        int *out_has_scratch,
                                        pp_error_t **error) {
  pp_media_root_set_t *roots = NULL;
  *out_has_archive = 0;
  *out_has_scratch = 0;
  pp_error_code_t status = pp_production_media_roots(production, &roots, error);
  for (uint64_t i = 0; status == PP_OK && i < pp_media_root_set_count(roots);
       ++i) {
    pp_uuid_t id;
    const char *name = NULL;
    const char *label = NULL;
    const char *legacy_uri = NULL;
    int32_t priority = 0;
    uint8_t enabled = 0;
    status = pp_media_root_set_get(roots, i, &id, &name, &label, &legacy_uri,
                                   &priority, &enabled, error);
    if (status == PP_OK && strcmp(name, "archive") == 0) {
      *out_has_archive = 1;
    }
    if (status == PP_OK && strcmp(name, "scratch") == 0) {
      *out_has_scratch = 1;
    }
  }
  pp_media_root_set_release(roots);
  return status;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    fprintf(stderr, "usage: postproject-c-lifecycle WORK_DIRECTORY\n");
    return 2;
  }
  const char *work = argv[1];
  char production_path[4096];
  char media[4096];
  char missing_path[4096];
  join(production_path, sizeof production_path, work, "lifecycle.pproj");
  join(media, sizeof media, work, "rushes/A001.mov");
  join(missing_path, sizeof missing_path, work, "missing.pproj");

  pp_production_t *production = NULL;
  pp_error_t *error = NULL;
  pp_uuid_t asset_id;
  uint64_t before = 0;
  uint64_t after = 0;
  int has_archive = 0;
  int has_scratch = 0;

  pp_error_code_t status =
      create_production(production_path, media, &asset_id, &error);
  if (status == PP_OK) {
    status = open_production(production_path, &asset_id, &production, &error);
  }
  if (status == PP_OK) {
    uint8_t exists = 0;
    pp_asset_set_t *assets = NULL;
    status = pp_production_asset_exists(production, &asset_id, &exists, &error);
    if (status == PP_OK) {
      status = pp_production_assets(production, &assets, &error);
    }
    if (status == PP_OK &&
        (exists != 1 || pp_asset_set_count(assets) != UINT64_C(1))) {
      status = PP_ERROR_INTERNAL;
    }
    pp_asset_set_release(assets);
  }
  if (status == PP_OK) {
    status = latest_sequence(production, &before, &error);
  }
  if (status == PP_OK && before == 0) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK) {
    status = commit_then_roll_back(production, &error);
  }
  if (status == PP_OK) {
    status = latest_sequence(production, &after, &error);
  }
  if (status == PP_OK) {
    status = media_root_names(production, &has_archive, &has_scratch, &error);
  }
  /* Exactly one revision was added: by the commit, not the rollback. */
  if (status == PP_OK &&
      (after != before + 1 || has_archive != 1 || has_scratch != 0)) {
    status = PP_ERROR_INTERNAL;
  }
  if (status == PP_OK && report_missing_production(missing_path) != 1) {
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
