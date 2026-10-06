# 0053: Fresh bounded media-root reads

Status: accepted for development.

## Decision

Media roots are mutable production knowledge. The roots cached in a loaded
`Production` value cannot serve as the live query result after another
connection changes them.

`ProductionRead::media_roots_page` is the bounded read primitive. SQLite queries
the reader's connection in priority/identity order, decoding at most the page
limit plus one continuation row. Live pages observe current facts and can see
inserts or omissions between calls. Retained sessions query their pinned view.
Existing cursor rules bind each token to production, family, order and read
scope; a session token cannot be resumed through a live reader.

Migrate native getters, resolution, inventory and CLI callers through this
fallible storage seam. Do not secretly open a retained WAL view inside a live
getter. Convenience root reads, resolution and inventory use the current read
interface and fail above the shared 1000-item query cap. Larger root collections
remain available through pages. `Production` carries identity and header
metadata only; it has no root cache. Open, read-session creation and edit
creation do not load root collections. Root mutations and job target checks
use SQL point queries; CLI inspections page roots and report truncation.
CLI root edits search a retained view in bounded pages.

## Migration and standards impact

Rust callers replace `production().media_roots()` with the fallible reader's
`media_roots()` or `media_roots_page()`. The production value's root getter
and setter are removed. There is no schema or persisted-data change.
The unreleased ABI 47 gains three exports without changing existing signatures
or public layouts. Language projections expose the same bounds and read scope.
Reviewed against the standards policy: root ordering and cursor behavior change
no external identifier, vocabulary, timecode or interchange mapping.
