# Media resolution

## Fingerprints

Resource fingerprint format version 1 uses BLAKE3. Files up to and including
1 MiB store the standard 32-byte BLAKE3 hash of every content byte. Larger files
store a strategy-versioned digest over the file size and 64 KiB regions at
deterministic beginning, middle, and end offsets.

The sampled format is designed for relocation candidate verification, not as a
collision-proof or adversarial content identifier. The resolver must expose it as
partial-fingerprint evidence. A caller can later request a full-file hash when
stronger verification is required.

Fingerprinting rejects symbolic links and non-regular files. It compares file
size and modification time before and after reading, failing rather than
persisting a result when the file appears to change during calculation. These
are resource fingerprints; structure-aware representation fingerprints are a
separate domain and may use different strategies.

## Resolution policy

Resource resolution checks known locators first and scans only when necessary.
One call resolves many resources. It walks each mapped root and each unnamed
search directory at most once, into an index shared by every resource in the
call (ADR 0031). Traversal is deterministic and does not follow symlinks. Each
directory has its own depth limit, 64 by default, and entry limit, 100,000 by
default. A directory that exceeds its entry limit is searched partially and
reported with `SearchTruncated` evidence, and the other directories and the
candidates found so far are still used. A cancellation token stops the call.

Presence and verification remain separate as required by ADR 0015. Normal
resolution checks that a known locator and its declared members exist. Callers
may opt into content verification per call; that tier recomputes the stored file
or sampled sequence fingerprint. Present content that differs from its recorded
identity produces an error result with `FingerprintMismatch` evidence. It is
never silently accepted as a new version. A resource whose stored fingerprints
all lie in domains the resolver cannot compute is reported online with
`FingerprintNotVerified` evidence instead, because nothing was compared (ADR
0028).

Productions identify roots by logical name. Each machine maps those names to
local directories when resolving; migrated pre-schema-6 roots retain their old
absolute URI as a fallback. An unmapped root and a mapped-but-unavailable root
produce distinct evidence. Neither stops traversal of other roots, so reachable
results remain visible together with the configuration diagnostic.

Discovery, cheap file-size filtering, and fingerprint verification are separate
stages. Full hashes produce exact resolution; sampled fingerprints produce
probable resolution. Only fingerprints in a domain the resolver can compute
widen discovery beyond filename matches. If no such fingerprint exists, a
matching filename is required and file size strengthens the evidence; stored
foreign fingerprints are listed as `FingerprintNotVerified` evidence on each
candidate for the caller to check. A candidate that cannot be read adds
`DiscoveryError` evidence and is dropped without hiding other candidates. Equally credible candidates produce
`Ambiguous` and require explicit confirmation. Confirmation adds a new locator
for the selected resource inside a production transaction; the resolver itself
never mutates production state.

Candidate discovery also compares parent path components with the former
locator. A match adds weak `RelativePathSimilarity` evidence and a small
confidence increment, which deterministically orders otherwise filename-only
candidates. Relative paths alone never eliminate competing candidates or turn
an ambiguous result into an automatic choice.

At the opt-in verification tier, a single technical inspection stored on the
representation can also be compared with candidate inspection results. An
exact normalized profile match contributes partial-fingerprint evidence and a
larger confidence increment than relative-path similarity. It still does not
eliminate another credible candidate: the result remains `Ambiguous` until a
caller confirms one URI. Missing `ffprobe`, failed inspection, compound imports
with multiple unassociated profiles, and mismatches simply omit this evidence.

Representation availability is then aggregated from its content structure.
Every required member online is `Online`; a mix of online and offline required
members is `Partial`; no resolvable required members is `Offline`; and an
ambiguous required member makes the representation `Ambiguous`. Optional
package members produce diagnostics without reducing availability. Known
missing image-sequence frames make an otherwise online sequence `Partial`, with
the exact frames retained in the diagnostic.

The inventory service scans every usable configured root without mutating the
production and reports known-online, partial, missing, new, changed, duplicate,
ambiguous-relink, unmapped-root, and unavailable-root observations. Results and
work counters are deterministically ordered for host applications and tests.

An optional JSON sidecar caches file size, modification time, and computed
fingerprints. The cache is versioned, bounded to 64 MiB, tied to one production
identity and exact root mapping, and lives outside the `.pproj` file. Missing,
stale, incompatible, or corrupt caches are discarded and rebuilt; deleting the
cache cannot change inventory semantics. The cache is an acceleration structure,
not durable production data.

Rust callers receive `ResourceResolution` values and aggregate them into a
`RepresentationResolution`. The CLI emits one representation result containing
ordered resource results and availability issues. The C ABI exposes the same
nested shape through index-checked accessors, and the C++ wrapper copies it into
owned `RepresentationResolution` values. Native confirmation remains a separate
explicit transaction operation.

The demonstrator exposes the expensive tier as `media resolve --verify`, with
an optional `--ffprobe PATH` override for technical candidate evidence. Direct
verification is currently available through the Rust media adapter and CLI,
not as a separate C, C++, or Python operation; all native and Python surfaces
retain presence resolution and explicit confirmation unchanged.
