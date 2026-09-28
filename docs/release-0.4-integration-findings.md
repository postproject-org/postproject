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
without a sidecar behaves as upstream. A second experiment, described
[below](#proxies-as-managed-artifacts), records Kdenlive's proxies as managed
artifacts.

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
  `Result` uses the member names of `std::expected`, and the public macros
  `POSTPROJECT_TRY` and `POSTPROJECT_TRY_ASSIGN` pass a failure on to the
  caller in one line (ADR 0033). See the {doc}`/src/integrators/cpp-quickstart`.
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

The pilot uses these changes. Its adapter builds without exceptions, passes
failures on with the propagation macros, and finds a clip with one qualified
lookup. It spells locators through PostProject and keeps Kdenlive's MD5 as a
host fingerprint. Opening a project resolves every missing clip in one call,
using search directories instead of invented roots. The adapter grew to about
375 lines, because it now records Kdenlive's hash and keeps the answers of the
batched resolution.

## Proxies as managed artifacts

Kdenlive keeps rendering proxies with its own settings, and the sidecar records
how each proxy was made. When Kdenlive renders a proxy for a clip the sidecar
knows, it requests an `org.kde.kdenlive:generate-proxy` job and claims it as
the worker. It renews the claim while `ffmpeg` or `melt` runs. One transaction
then records the proxy as a proxy representation of the clip's asset, records
the activity with the tool and its argument list, and completes the job. A
failed render fails the job with the end of its log. When a project opens,
PostProject evaluates each recorded proxy. A proxy whose source was replaced
is reported stale and offered for rebuilding in Kdenlive's relink dialog.
Kdenlive itself checks a proxied clip's hash only when the proxy is missing, so
it would keep playing the old proxy.

This needed no change to PostProject. Content observation from release 0.4
supplied the one missing piece. The pilot's Kdenlive changes grew to about
120 lines, and the adapter to about 690. Kdenlive's tests cover a recorded
render, a stale proxy offered for rebuilding, a proxy rendered again at its
old path, failed and abandoned renders, and proxies made before their clip was
recorded. In the running application under Xvfb on 2026-09-28:

- the first save of a new project recorded the proxy Kdenlive had already made;
- after the source was replaced, reopening offered the proxy for rebuilding;
- the rebuild was recorded as a succeeded job;
- PostProject then evaluated the old proxy as stale and the new one as current.

**Evaluation sees only observed content.** Artifact evaluation compares recorded
fingerprints and never reads a file, as designed. A host that evaluates without
observing reports a replaced source's proxy as current. To ask whether a proxy
is stale now, the first version of the adapter verified the source, observed it
in a write transaction only if it differed, and then evaluated. It did the same
before every render, so that the proxy is recorded against the content it was
made from. The verification was unnecessary: observing unchanged content already
recorded nothing and created no revision, but the observation did not say what
it had found.

**A C++ host cannot read metadata values.** The C++ wrapper returns metadata
values, including regeneration parameters, as opaque `MetadataInput` objects.
They can be passed to an exact-value query or copied to another target, but
not read. The C ABI has typed getters. Kdenlive records its render arguments as
an ordered list on the activity. It cannot read them back to render a proxy
again from a regeneration plan, although the CLI and Python can.

**A host needs a job kind of its own.** Kdenlive's arguments mean something
only to Kdenlive. A proxy job under the reference executor's
`org.postproject:generate-proxy` kind could be claimed by `postproject job
run`, and a regeneration plan derived from Kdenlive's activity would ask that
executor to interpret them. Kdenlive therefore uses its own kind. Requesting
and claiming in one transaction keeps other workers from seeing the job as
requested.

**Work done before a production existed can only be inferred.** Kdenlive
renders proxies as soon as clips are added, usually before the first save
creates the sidecar. On save, the pilot records such a proxy only when its file
name is the clip's present Kdenlive hash, which is how Kdenlive names the
proxies it renders. That activity names no tool and no parameters.
PostProject reports it as current but not reproducible, which is accurate.

**The wrapper warns under GCC 15.** Building the pilot at `-O2` with GCC 15.3,
`evaluateArtifact` in the installed C++ header raises `-Wmaybe-uninitialized`
for its optional fingerprint values. Both values are initialized before they
are moved, so the warning is a false positive. It still appears in every
consumer's build log.

### Changes made in response

Each change is recorded in an ADR and applies to every host.

- **Observation outcome (ADR 0035, C ABI 33).** Observing content reports
  whether it was unchanged, changed, or observed for the first time, and the
  guides recommend observing a source that may have changed outside the host,
  then evaluating. The adapter now observes before every render and on every
  open, without verifying first.
- **Readable C++ metadata values (ADR 0036).** `MetadataValue` replaces
  `MetadataInput` as one copyable type for writing and reading. The pilot's
  tests read the recorded argument list back and compare it with the expected
  value.
- **A warning-free C++ header (ADR 0034).** `evaluateArtifact` no longer
  triggers the GCC 15 false positive. CI compiles every C++ program against the
  header with the current GCC and Clang, as C++17 and C++20, with warnings as
  errors. The pilot's test build is free of the warning.

The job kind and the recording of work done before the sidecar existed needed
no change. The integrator guides describe both.

## Packaging

Kdenlive ships on Flathub with the KDE 6.10 runtime. PostProject now publishes
a tested Flatpak module that builds offline from the release source archive
with the Rust SDK extension, together with its generated Cargo sources. See
{doc}`/src/integrators/flatpak`. The Rust extension is a build-only dependency,
and the resulting application ships only the 4.5 MiB shared library.
