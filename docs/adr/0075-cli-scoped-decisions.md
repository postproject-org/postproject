# 0075: CLI decisions use one production-scoped token

Status: accepted for development.

## Decision

Remove `--base-revision`. Use `--decision-base TOKEN` from `inspect` for
writes that depend on observed knowledge. One token includes production,
revision and sequence; it also represents an empty production. Remove the
CLI-only choice between scoped tokens and bare revision IDs. Keep explicit
additive commands without a base and reject global bases for portable
exchange, whose proposal already supplies its scope.

Help leads with create, import, adopt, resolve, inspect, changes and handoff
tasks. The command hierarchy and JSON format 1 remain unchanged: this syntax
cleanup does not change machine-output shapes, values or exit categories.

## Migration and standards impact

Read `inspect --json` and pass its `decision_base` instead of querying
`revisions latest` and passing its ID. Semantic conflicts keep their exact
base and superseding revision details. No automatic retry is introduced.

Reviewed against the contributor standards policy: no domain, persistence,
protocol, identifier or standards mapping changes.
