# PostProject 0.6 compatibility proposal

**Status: proposed, not maintainer-approved.** No new public compatibility
promise is enacted by this page. The released baseline is `v0.5.0-alpha.1`;
the candidate is `0.6.0-alpha.1`, C ABI 37 and schema 17. The
{doc}`release-0.6-compatibility-evidence` report evaluates that actual boundary.
No additional waiting release is imposed on unchanged families.

## Proposed source-level C++ subset

Propose `cpp-result-propagation`: `Result<T>` (including `Result<void>`),
`POSTPROJECT_TRY`, and `POSTPROJECT_TRY_ASSIGN`. Kdenlive and Natron compile and
execute all three through their distinct native adapters. The complete
projection check now requires every helper for each host; partial helper use
does not qualify. C has no requirement to use this C++ projection.

The authoritative C header, C++ wrapper and Python binding are unchanged from
the released baseline. The only native runtime change repairs destruction of
closed transaction handles. It does not alter the Result protocol or helper
evaluation/error propagation. Installed exception-enabled and no-exception
consumers pass. The proposed dependency closure contains this projection
family alone, with its owning Error/value and evaluation contracts reviewed.

This is useful as a consistent source-level propagation protocol for native
host adapters. It does not name a stable production operation, promise object
layout across compilers, or promise cross-series binary substitution. Native
operations themselves remain experimental. Consumers must still adapt when
an operation's signature or domain contract changes.

## Other family results and exclusions

Natron adds complete normal-path evidence for asset point reads, bounded
locator inspection, revision feed, explicit content verification and canonical
bindings. OBS adds direct-C production lifecycle and asset point-read evidence.
Its rollback/lost-acknowledgement contract tests remain separate from normal
recording evidence. Natron's partial, ambiguity, cancellation and conflict
traces also remain separate.

The mechanical matrix retains the historical OpenAssetIO Manager label once.
Its actual route is an independent OpenAssetIO test host, rather than another
desktop application's normal create/open action. The matrix counts supplied
labels; this proposal does not make a new positive independence determination
for that route. Consequently, it does not propose production lifecycle or
read families whose useful callable closure requires it. The complete native
Kdenlive and Natron Result use does not rely on that label.

Transaction lifecycle and base-revision conflict families conservatively have
last-change release 0.6 because the observed guard behavior changed. They
cannot qualify as unchanged across this boundary. External identifiers remain
mechanically eligible, but their dependency closure includes the changed
transaction family. Known-media lookup includes both locator and fingerprint
lookup with all owning-page operations; no supplied host completes that whole
family. Natron's naming-aware locator use is real partial evidence, not a
reason to split the family or insert unused calls.

Filtered feeds/waiters and job renew/fail/cancel do not acquire new family
definitions from unrelated host calls. The selected refresh uses the durable
unfiltered feed. Existing installed examples test filtered continuation
watermarks, including empty pages, cancellation and close.

## Draft ADR 0020 amendment

The maintainer can review this concrete text without changing current policy:

```text
Release 0.6 names cpp-result-propagation as a source-level C++17 compatibility
family: Result<T>, Result<void>, POSTPROJECT_TRY and POSTPROJECT_TRY_ASSIGN
retain their success/error ownership and propagation protocol within 0.6.x.
Diagnostic text and compiler-specific layouts are not promised. No C ABI
operation family or cross-series binary replacement promise is named.
Other APIs remain experimental with migration notes for maintained consumers.
```

The proposal is supported by complete Kdenlive and Natron projection use and
unchanged baseline declarations. ABI layout/symbol checks and recompilation
are separate evidence; no previously compiled cross-series substitution is
claimed, since the 0.5 series made no such promise. Released schemas continue
under the existing tested forward-migration policy.
