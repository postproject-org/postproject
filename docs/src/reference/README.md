# Reference

Reference pages answer **exact** questions: which function to call, which value a field takes, which term means what, and how a PostProject concept relates to an external standard. They assume you already know what you want to do. If you do not, start with the {doc}`../integrators/README` or the {doc}`../concepts/README`.

## API reference

Each public surface has a generated reference built from the same sources that are installed for consumers:

- {doc}`../../reference/c-api` — the hand-written C header, which is the authoritative native interface;
- {doc}`../../reference/cpp-api` — the header-only C++17 wrapper;
- {doc}`../../reference/python-api` — the typed Python binding over the C ABI;
- {doc}`../../rust-api` — version-matched rustdoc for contributors and adapter authors.
- {doc}`cli-output` — JSON format, errors and process exit categories for scripts.

The reference gives signatures and ownership rules. To see an operation *used*, look it up in the task guides: every public C function, C++ member function, and Python method appears in at least one tested example, and CI fails when one does not.

## Example programs

{doc}`../../reference/examples` lists the complete quickstart programs. The task guides show extracts of larger example programs, one per topic and surface, which CI compiles and runs against an installed package:

| Topic | Shows |
|---|---|
| guides | the end-to-end scenario: create, identify, describe, move and relink, add a sequence, record provenance, query, and follow the revision feed |
| lifecycle | reopening a production, explicit transactions, rollback, and errors |
| media | representation shapes, structure reads, media roots, locators, fingerprints, resolution issues, inventory, inspection, and recognition |
| knowledge | external identifiers and every typed metadata value |
| provenance | activities with snapshots, staleness after a change, and dependency sets |
| jobs | the complete job protocol and regeneration planning |

The programs live in `docs/examples/{c,cpp,python,cli}` and `docs/examples/rust/tests` in the source repository.

## Vocabulary and standards

- {doc}`terminology` — precise terms used throughout the project.
- {doc}`metadata-vocabularies` — registered metadata vocabularies and their properties.
- {doc}`standards-mapping-matrix` — how PostProject concepts map to external standards, and where they deliberately do not.

```{toctree}
:hidden:

/reference/c-api
/reference/cpp-api
/reference/python-api
/rust-api
cli-output
/reference/examples
terminology
metadata-vocabularies
standards-mapping-matrix
```
