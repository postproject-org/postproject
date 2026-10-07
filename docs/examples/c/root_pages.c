#include <postproject/postproject.h>
#include <stdio.h>

/* [root-pages] */
static int root_pages(const char *path) {
  pp_production_t *production = NULL;
  pp_transaction_t *transaction = NULL;
  pp_read_session_t *view = NULL;
  pp_media_root_set_t *live = NULL, *first = NULL, *last = NULL;
  pp_error_t *error = NULL;
  pp_media_root_id_t root;
  int result = 1;
#define CHECK_ROOT(call) do { if ((call) != PP_OK) goto cleanup; } while (0)
  CHECK_ROOT(pp_production_create(path, "Root pages", &production, &error));
  CHECK_ROOT(pp_production_begin_transaction(production, &transaction, &error));
  CHECK_ROOT(pp_transaction_add_media_root(transaction, "first", NULL, -1, &root, &error));
  CHECK_ROOT(pp_transaction_add_media_root(transaction, "second", NULL, 0, &root, &error));
  pp_commit_receipt_t commit_receipt;
  CHECK_ROOT(pp_transaction_commit_with_receipt(transaction, &commit_receipt, &error));
  CHECK_ROOT(pp_production_media_roots_page(production, 1, NULL, &live, &error));
  if (pp_media_root_set_count(live) != 1 || !pp_media_root_set_next_cursor(live)) goto cleanup;
  CHECK_ROOT(pp_production_read_session(production, &view, &error));
  CHECK_ROOT(pp_read_session_media_roots_page(view, 1, NULL, &first, &error));
  const char *cursor = pp_media_root_set_next_cursor(first);
  if (pp_media_root_set_count(first) != 1 || !cursor) goto cleanup;
  /* Keep first alive while borrowing its cursor. The next set owns its rows. */
  CHECK_ROOT(pp_read_session_media_roots_page(view, 1, cursor, &last, &error));
  if (pp_media_root_set_count(last) != 1 || pp_media_root_set_next_cursor(last)) goto cleanup;
  pp_media_root_set_t *invalid = first;
  if (pp_production_media_roots_page(production, 0, NULL, &invalid, &error) != PP_ERROR_INVALID_ARGUMENT || invalid != NULL) goto cleanup;
  pp_error_release(error); error = NULL;
  result = 0;
cleanup:
  if (result && error) fprintf(stderr, "%s\n", pp_error_message(error));
  pp_error_release(error);
  pp_media_root_set_release(last);
  pp_media_root_set_release(first);
  pp_media_root_set_release(live);
  pp_read_session_release(view);
  pp_transaction_release(transaction);
  pp_production_release(production);
  return result;
#undef CHECK_ROOT
}
/* [/root-pages] */

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  char path[4096];
  if (snprintf(path, sizeof path, "%s/roots.pproj", argv[1]) >= (int)sizeof path) return 2;
  return root_pages(path);
}
