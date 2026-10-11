# 0073: Single files stage directly from native paths

Status: accepted for development C ABI 53.

## Decision

Use `pp_transaction_import_file` and
`pp_transaction_add_file_representation` for single files. Remove
`pp_media_source_create_file`; constructing and releasing a source owner
adds no useful information to this case. Compound-source operations keep
their explicit sequence, ordered-part and package inputs.

Both file and compound operations use the same preparation and staging
functions. Keep fingerprinting, stable IDs, automatic effect capture,
proposal preparation/replay, atomic commit and rollback behavior. Failed
staging clears the output ID, leaves the edit open and adds no mutation.
Paths are borrowed NUL-terminated UTF-8 during the C call.

Python accepts paths directly and removes the redundant `FileSource` class.
Its `MediaSource` annotation includes paths and compound inputs. C++ accepts
`std::filesystem::path` and existing UTF-8 path strings implicitly, and
removes the redundant `MediaSource::file` factory. File staging in both
bindings calls the native file operation without allocating a source owner.

## Migration and standards impact

Replace C source creation/import/release with one file operation. Pass paths
instead of Python `FileSource` or C++ `MediaSource::file` wrappers. ABI 53
now has 344 exports and 38 public layouts.

Reviewed against the contributor standards policy: representation meaning,
storage and exchange formats are unchanged. No file recognition, identifier
normalization or standards mapping is added.
