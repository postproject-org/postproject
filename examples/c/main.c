#include <postproject/postproject.h>

#include <stdio.h>

static void print_asset_id(const pp_asset_id_t *id) {
  for (size_t index = 0; index < sizeof(id->bytes); ++index) {
    printf("%02x", id->bytes[index]);
  }
  putchar('\n');
}

int main(int argc, char **argv) {
  if (argc != 3) {
    fprintf(stderr,
            "usage: postproject-c-example OUTPUT_PRODUCTION MEDIA_FILE\n");
    return 2;
  }

  /* Every handle starts as NULL and is released exactly once at the end.
   * Release functions accept NULL, so one cleanup path serves every outcome. */
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_media_source_t *media = NULL;
  pp_representation_set_t *representations = NULL;
  pp_error_t *error = NULL;
  pp_asset_id_t asset_id = {{0}};

  /* Each fallible call returns a status code. On failure it transfers a
   * caller-owned error handle, so the chain stops at the first failure and
   * never overwrites an error that still has to be released. */
  pp_error_code_t status =
      pp_production_create(argv[1], "C quickstart", &production, &error);
  if (status == PP_OK) {
    status = pp_production_begin_transaction(production, &transaction, &error);
  }
  /* A media source describes the file to import; the transaction borrows it
   * only for the call. */
  if (status == PP_OK) {
    status = pp_media_source_create_file(argv[2], &media, &error);
  }
  if (status == PP_OK) {
    status = pp_transaction_import_media(transaction, media, NULL, &asset_id,
                                         &error);
  }
  pp_media_source_release(media);
  /* Staged work becomes durable only on commit. Releasing a transaction that
   * was not committed discards everything staged in it. */
  if (status == PP_OK) {
    pp_commit_receipt_t commit_receipt;
    status = pp_transaction_commit_with_receipt(transaction, &commit_receipt, &error);
  }
  /* The result set is caller-owned; strings read from it borrow the set. */
  if (status == PP_OK) {
    status = pp_production_representations(production, asset_id,
                                           &representations, &error);
  }
  pp_locator_id_t locator_id = {{0}};
  const char *uri = NULL;
  pp_locator_availability_t availability = 0;
  uint8_t has_last_seen = 0;
  int64_t last_seen = 0;
  uint8_t has_sequence_naming = 0;
  pp_sequence_naming_t sequence_naming;
  if (status == PP_OK) {
    status = pp_representation_set_get_locator(
        representations, 0, 0, 0, &locator_id, &uri, &availability,
        &has_last_seen, &last_seen, &has_sequence_naming, &sequence_naming,
        &error);
  }

  if (status != PP_OK) {
    /* Branch on the code; the message is diagnostic text for people. The
     * message is borrowed from the error handle. */
    fprintf(stderr, "operation failed (%u): %s\n", status,
            error != NULL ? pp_error_message(error) : "no details");
  } else {
    print_asset_id(&asset_id);
    printf("representations: %llu\n",
           (unsigned long long)pp_representation_set_count(representations));
    /* Use borrowed strings before releasing the set that owns them. */
    printf("location: %s\n", uri);
  }
  pp_error_release(error);
  pp_representation_set_release(representations);
  pp_transaction_release(transaction);
  pp_production_release(production);
  return status == PP_OK ? 0 : 1;
}
