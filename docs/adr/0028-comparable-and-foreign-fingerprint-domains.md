# ADR 0028: Comparable and foreign fingerprint domains in resolution

- Status: Accepted
- Date: 2026-09-27

## Context

ADR 0021 makes fingerprint algorithms open-world: a resource may carry an
observation in any algorithm/version domain, and PostProject stores its bytes
opaquely. Hosts use this to preserve content evidence that they already own.
Examples are an editor's head-and-tail MD5, a MAM checksum, a camera clip hash,
or a facility xxHash.

Resolution treated every stored fingerprint as if the resolver could check it.
Once any fingerprint existed, discovery stopped requiring a filename match and
accepted every file of the stored size. Verification then discarded each
candidate for which no stored value shared the resolver's algorithm. As a
result, a resource whose only evidence was a host's fingerprint was never
resolved, not even to a file with the same name and size. Content verification
at a known locator reported such a resource as a fingerprint *mismatch*, which
claims that the content differs when nothing was compared. In short, preserving
a host's evidence verbatim, as the design rules require, disabled resolution for
that resource.

In the first real-host integration, a pilot in a non-linear editor, clips saved
before the integration recorded PostProject fingerprints could not be relinked.
The same failure applies to any host, manager, or ingest tool that records its
own content hashes.

## Decision

The resolver classifies each stored fingerprint domain by whether it can
compute that domain itself:

- A **comparable** domain is one whose algorithm and version the media crate
  implements for the resource's structure. These are the BLAKE3 full-file and
  sampled-region domains for a file, and the sequence collection domain for an
  image sequence.
- A **foreign** domain is any other domain. It is kept exactly as recorded.

Only comparable domains take part in discovery and verification:

- Discovery accepts files of the stored size regardless of name only when a
  comparable domain exists. Otherwise a filename match is required, exactly as
  for a resource with no fingerprint.
- Without a comparable domain, candidates are scored by filename, size,
  relative path, and technical inspection. Content is never hashed for a
  comparison that cannot succeed.
- A candidate scored this way whose resource has foreign domains carries
  `FingerprintNotVerified` evidence. Its detail lists every foreign domain as
  `<algorithm> version <n>`. Such a candidate is never `ResolvedExact`.
- Content verification at a present known locator with no comparable domain
  reports `OnlineAtKnownLocator`, with `FingerprintNotVerified` evidence in
  place of an error. `FingerprintMismatch` is reserved for comparable content
  that differs.
- A candidate that cannot be read or hashed adds `DiscoveryError` evidence to
  the resource and is dropped. Other candidates are still returned.

The host checks foreign domains. Resolution proposes a candidate annotated with
the domains it did not check. The host verifies the file with its own
algorithm, then confirms the locator in a transaction. This is the existing
confirm-before-commit flow. No verification callback crosses the ABI.

## Alternatives considered

- **Treat foreign-only resources as having no fingerprint.** This restores
  filename discovery, but it hides from the caller that evidence exists which
  it could check. The new evidence kind costs one enum value and makes the
  caller's next step explicit.
- **Host-supplied verifier callbacks.** The C ABI deliberately has no callbacks
  on synchronous query paths: re-entrancy, thread affinity, unwinding, and
  Python's GIL would all become part of the contract. The two-phase flow gives
  the same power without them.
- **Built-in implementations of common host hashes.** Host hashes are rarely
  plain digests. The pilot's MD5 covers only head and tail regions, and their
  exact definitions belong to their hosts. Implementing them would make
  PostProject responsible for other applications' formats.

## Standards impact

None. Fingerprint domains remain PostProject-defined, open-world identifiers.
No external standard defines content-identity algorithms for relinking.
Foreign values are still preserved byte for byte.

## Consequences

Recording a host's own content evidence is now safe, and a host can use that
evidence to confirm a candidate. Resources that only ever had foreign evidence
resolve like fingerprint-less resources, and a caller can see which evidence is
left for it to check. Callers that previously treated a content-verification
error as the signal for "cannot verify" must now read `FingerprintNotVerified`
evidence instead. One unreadable file in a searched directory no longer turns
the resource's result into an error when another candidate is usable.
