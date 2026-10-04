#include <postproject/postproject.h>

#include <stdio.h>

/* Closed A may outlive open B, but must never release B's production guard. */
static int check_terminal_state(pp_production_t *production, int terminal) {
  pp_transaction_t *a = NULL;
  pp_transaction_t *b = NULL;
  pp_transaction_t *c = NULL;
  pp_error_t *error = NULL;
  int result = 1;
  if (pp_production_begin_transaction(production, &a, &error) != PP_OK)
    goto cleanup;

  if (terminal == 2) {
    const pp_uuid_t absent = {{1}};
    if (pp_transaction_confirm_locator(a, &absent, "file:///absent", NULL,
                                       NULL, &error) != PP_OK ||
        pp_transaction_commit(a, &error) == PP_OK)
      goto cleanup;
    pp_error_release(error);
    error = NULL;
  } else if ((terminal == 0 ? pp_transaction_commit(a, &error)
                            : pp_transaction_rollback(a, &error)) != PP_OK) {
    goto cleanup;
  }

  if (pp_production_begin_transaction(production, &b, &error) != PP_OK)
    goto cleanup;
  pp_transaction_release(a);
  a = NULL;
  if (pp_production_begin_transaction(production, &c, &error) !=
          PP_ERROR_CONFLICT ||
      c != NULL)
    goto cleanup;
  pp_error_release(error);
  error = NULL;
  pp_transaction_release(b);
  b = NULL;
  if (pp_production_begin_transaction(production, &c, &error) != PP_OK)
    goto cleanup;
  result = 0;

cleanup:
  if (result != 0)
    fprintf(stderr, "terminal state %d: %s\n", terminal,
            error == NULL ? "guard unexpectedly released" : pp_error_message(error));
  pp_error_release(error);
  pp_transaction_release(a);
  pp_transaction_release(b);
  pp_transaction_release(c);
  return result;
}

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
  remove(argv[1]);
  pp_production_t *production = NULL;
  pp_error_t *error = NULL;
  if (pp_production_create(argv[1], "Transaction lifetime", &production,
                           &error) != PP_OK) {
    pp_error_release(error);
    return 3;
  }
  int result = 0;
  for (int terminal = 0; terminal != 3; ++terminal)
    result |= check_terminal_state(production, terminal);
  pp_production_release(production);
  return result;
}
