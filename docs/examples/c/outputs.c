/* Register already-produced outputs; release all owners on either outcome. */
#include <postproject/postproject.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  char project[4096], render[4096], thumbnail[4096];
  snprintf(project, sizeof project, "%s/outputs.pproj", argv[1]);
  snprintf(render, sizeof render, "%s/render.dat", argv[1]);
  snprintf(thumbnail, sizeof thumbnail, "%s/thumbnail.dat", argv[1]);
  FILE *file = fopen(render, "wb"); if (!file) return 1;
  fputs("fixture render", file); fclose(file);
  file = fopen(thumbnail, "wb"); if (!file) return 1;
  fputs("fixture thumbnail", file); fclose(file);
  pp_production_t *production = NULL;
  pp_transaction_t *edit = NULL;
  pp_metadata_input_t *width = NULL;
  pp_error_t *error = NULL;
  pp_error_code_t status = pp_production_create(project, NULL, &production, &error);
  if (status == PP_OK) status = pp_metadata_input_create_i64(1920, &width, &error);
  if (status == PP_OK) status = pp_production_begin_edit(production, NULL, NULL, &edit, &error);
  if (status == PP_OK) {
    /* [register-outputs] */
    const pp_output_source_t outputs[] = {
        {NULL, PP_REPRESENTATION_ORIGINAL, render, NULL, NULL, NULL},
        {NULL, PP_REPRESENTATION_ORIGINAL, thumbnail, NULL, "example:thumbnail", NULL}};
    const pp_activity_parameter_t parameter = {"example:recipe", "width", width};
    const pp_activity_recipe_t recipe = {
        .kind = "example:generate", .tool_name = "Fixture generator",
        .tool_version = "1.0", .parameters = &parameter, .parameter_count = 1};
    pp_output_identity_t identities[2];
    pp_activity_id_t activity;
    status = pp_transaction_register_outputs(edit, outputs, 2, &recipe,
                                             identities, &activity, &error);
    pp_commit_receipt_t receipt;
    if (status == PP_OK) status = pp_transaction_commit(edit, &receipt, &error);
    /* All returned identities become durable together after explicit commit. */
    /* [/register-outputs] */
    if (status == PP_OK && (receipt.outcome != PP_COMMIT_REVISION_CREATED ||
        memcmp(identities[0].asset_id.bytes, identities[1].asset_id.bytes, 16) == 0))
      status = PP_ERROR_INTERNAL;
  }
  if (status != PP_OK) fprintf(stderr, "%s\n", error ? pp_error_message(error) : "output assertion failed");
  pp_metadata_input_release(width);
  pp_transaction_release(edit);
  pp_production_release(production);
  pp_error_release(error);
  return status == PP_OK ? 0 : 1;
}
