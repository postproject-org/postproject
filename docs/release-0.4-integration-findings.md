# Release 0.4 integration findings

Release 0.4 adds one maintained integration with a real application, the
[Kdenlive pilot](https://github.com/postproject-org/postproject-kdenlive). The
pilot is a patch series on Kdenlive `v26.08.1` that links only the installed
CMake package. Its [brief](https://github.com/postproject-org/postproject-kdenlive/blob/main/BRIEF.md)
records Kdenlive's behavior, checked against source on 2026-09-27. This page
records what the pilot showed about PostProject. The pilot itself is kept only
to produce these findings.

## What was built

A resolver experiment: Kdenlive keeps its project file and behavior, and
PostProject only assists relinking. On save, Kdenlive records each
file-backed bin clip in a sidecar production next to the project: an imported
asset keyed by Kdenlive's `kdenlive:control_uuid`, which is stored as an
application external identifier. When a project opens with a missing clip,
Kdenlive resolves that asset under the project folder and the clip's former
folder. It offers the one candidate in its own relink dialog only if Kdenlive's
own MD5 of the file agrees. Kdenlive's own tests cover a renamed clip, a clip
outside the project folder, two identical copies, a candidate that Kdenlive's
hash rejects, no sidecar, and an unreadable sidecar. The pilot changes about
30 lines of Kdenlive, all behind a CMake option, plus one adapter file of
about 290 lines.

The experiment met its success criteria. A renamed clip is relinked by content,
duplicate copies stay with the user, and Kdenlive without PostProject or
without a sidecar behaves as upstream. Recording Kdenlive's proxies as managed
artifacts was not started. Its prerequisites are below.

## Findings

**The C++ wrapper assumes exceptions.** KDE's compiler settings build
application code with `-fno-exceptions`, and Kdenlive keeps that default.
Every wrapper operation reports failure by throwing `postproject::Error`, so
the pilot compiled its one adapter file with `kde_source_files_enable_exceptions()`
and caught everything there. That works, but a KDE maintainer would ask about
it first. The C ABI is exception-free, yet it is too verbose to be the offer
for a C++ host. A non-throwing variant of the wrapper, or result-returning
overloads, would remove the question.

**A host's own content hash cannot take part in resolution.** Kdenlive already
stores an MD5 of each clip, over head and tail regions of 1,000,000 bytes. The
resolver verifies candidates only against algorithms it can compute, so that
hash is useless to it. The pilot re-imports every clip to obtain PostProject's
own fingerprint and uses Kdenlive's hash only as a second check on
PostProject's answer. A clip saved before the pilot was installed therefore
cannot be relinked. Reading `resolver.rs` shows something worse. A resource
whose only fingerprint uses a foreign algorithm is never resolved at all.
Discovery stops requiring a filename match once any fingerprint exists, and
verification then rejects every candidate for lack of a comparable algorithm,
so even a same-name, same-size file is not offered. Preserving a host's
evidence verbatim, which the design rules require, is exactly what disables
resolution for that resource.

**Hosts cannot compute or verify PostProject fingerprints through the C ABI.**
`pp_transaction_record_resource_fingerprint` records a value, but no C, C++,
or Python function computes one for a path. Content verification is available
only in Rust and the CLI. Import is the only way a native host obtains a
fingerprint. That was enough for relinking, but it blocks managed proxies: to mark
a proxy stale after its source is replaced, Kdenlive must record the source's
new fingerprint, and it has no supported way to compute it.

**Logical roots do not map onto an editor without roots.** Kdenlive has a
project folder and absolute clip paths, and no notion of named media roots.
The pilot invented roots: one for the project folder, mapped to its current
location on every call, and one per clip folder outside it, named from a
digest of the path and labelled with the path. The label is effectively
machine-local data stored in the production. Resolution also fails as a whole
when a mapped root exceeds the entry limit, so the pilot maps only the roots
relevant to one missing clip. A per-call search directory that needs no
registered root would express "look near where it was" directly.

**Looking up an object by host identity takes two calls.**
`find_by_external_identifier` matches scheme and value but not the qualifier.
Under the shared application scheme, the pilot has to read each match's
identifiers again to confirm the `org.kde.kdenlive:control_uuid` qualifier.

**Hosts compare locators by path, not by URI.** Kdenlive spells file URIs with
`QUrl`, and PostProject canonicalizes with Rust's `url` crate. The two can
percent-encode differently, so the pilot converts every locator back to a local
path before comparing. No public function turns a path into the canonical
locator URI that import would record.

**Resolution is one asset at a time.** A project opened with many missing clips
rescans the same roots once per clip. The pilot did not measure the cost.

## Changes made in response

Release 0.4 addresses each finding in general terms, for every host rather than
for Kdenlive alone. Each change is recorded in an ADR and documented for
integrators.

- **C++ without exceptions (ADR 0032).** Every fallible C++ operation returns
  `postproject::Result<T>`. The header compiles with `-fno-exceptions`, and
  `value()` throws `postproject::Exception` only where exceptions are enabled.
  See the {doc}`/src/integrators/cpp-quickstart`.
- **Host content hashes (ADR 0028, C ABI 28).** A fingerprint in a domain that
  PostProject cannot compute no longer disables resolution. Candidates are
  found by name and size and carry `fingerprint_not_verified` evidence that
  lists the host's domains, so the host checks them with its own hash. Content
  verification reports such a resource as not verified, not as a mismatch.
- **Content fingerprints for hosts (ADR 0029, C ABI 29).** Hosts can compute
  PostProject's fingerprint of a file and verify a resource against a path. A
  transaction can observe a resource's present content, which also recomputes
  every representation that uses the resource and records the file's size and
  modification time. This was the prerequisite for managed proxies.
- **Qualified identifier lookup (ADR 0002, C ABI 30).** Lookup by external
  identifier can require an exact qualifier.
- **Canonical locators (ADR 0030, C ABI 31).** Hosts obtain the locator URI
  that import records for a path, and a local path for a locator, instead of
  spelling URIs themselves.
- **Search scope, budgets, batches, and cancellation (ADR 0031, C ABI 32).**
  - Resolution accepts unnamed search directories, so hosts without logical
    roots no longer invent them.
  - It resolves many assets with one scan of each directory.
  - It budgets entries per directory: an oversized directory is searched
    partially and reported, and the resolution does not fail.
  - A cancellation token can stop it.
  - A candidate names the logical root it was found under.

## For managed proxies

Proxies as managed artifacts needed fingerprint computation, which release 0.4
now provides through content observation. The rest already existed.
Kdenlive's proxy task can act as the job worker: it claims a proxy job, renders with its own `melt` or `ffmpeg` settings, and
completes with the proxy as a derived representation. Artifact evaluation
would then report a proxy as stale where Kdenlive, which names proxies by the
source's MD5 and never checks a proxy's content, reports nothing.

## Packaging

Kdenlive ships on Flathub with the KDE 6.10 runtime. PostProject now publishes
a tested Flatpak module that builds offline from the release source archive
with the Rust SDK extension, together with its generated Cargo sources. See
{doc}`/src/integrators/flatpak`. The Rust extension is a build-only dependency,
and the resulting application ships only the 4.5 MiB shared library.
