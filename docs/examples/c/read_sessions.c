/* Coherent reads, detached bases, explicit edits and atomic receipts. */
#include <postproject/postproject.h>
#include <stdio.h>
#include <string.h>

/* [coherent-reads] */
static int exercise(const char *path, const char *media) {
  pp_production_t *production = NULL;
  pp_read_session_t *empty = NULL, *view = NULL;
  pp_transaction_t *edit = NULL;
  pp_media_source_t *source = NULL;
  pp_asset_set_t *assets = NULL;
  pp_representation_set_t *representations = NULL;
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
  pp_read_session_release(view); view = NULL;
  /* Copied sets and a detached base survive closing the pinned view. */
  CHECK(pp_production_begin_edit(production, &base, &edit, &error));
  CHECK(pp_transaction_commit_with_receipt(edit, &receipt, &error));
  if (receipt.outcome != PP_COMMIT_NO_CHANGE || pp_asset_set_count(assets) != 1)
    goto cleanup;
  result = 0;
cleanup:
  if (result) fprintf(stderr, "%s\n", error ? pp_error_message(error) : "read-view assertion");
  pp_error_release(error);
  pp_asset_set_release(assets);
  pp_representation_set_release(representations);
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
