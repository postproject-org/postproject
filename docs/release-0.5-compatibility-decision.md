# PostProject 0.5 compatibility decision

This decision applies ADR 0040 to the `0.5.0-alpha.1` release candidate. The
maintainer accepted the no-subset recommendation on 2026-10-01. The generated
{doc}`release-0.5-compatibility-evidence` report is the mechanical input; this
page records the semantic, projection, and usefulness review that the report
cannot decide.

## Decision

Release the 0.5 series with **no named compatibility subset**.

Two leaf families are mechanically eligible, but neither forms a useful
compatibility contract without foundational families that lack the required
two-host evidence:

```{mermaid}
flowchart LR
    production["production lifecycle<br/>one host"]
    transaction["transaction lifecycle<br/>one host"]
    identifiers["external identifiers<br/>two hosts"]
    resolution["resolution<br/>three hosts"]

    production --> identifiers
    transaction --> identifiers
    production --> resolution

    classDef eligible fill:#d8f3dc,stroke:#2d6a4f,color:#081c15
    classDef incomplete fill:#fff3bf,stroke:#e67700,color:#3d2800
    class identifiers,resolution eligible
    class production,transaction incomplete
```

Promising only the leaf operations would still require consumers to create,
open, and release experimental production handles. Identifier attachment would
also require an experimental transaction begin/commit/release contract. That
is a misleadingly tiny release subset, even though the leaf-family evidence is
real.

## Candidate review

| Criterion | External identifiers | Resolution |
|---|---|---|
| Present in `v0.4.0-alpha.1` | yes | yes |
| Complete normal-path host evidence | Blender, Kdenlive | Blender, Kdenlive, OpenAssetIO Manager |
| C declarations unchanged through the candidate | yes | yes |
| C++ projections unchanged through the candidate | yes | yes |
| Python projections unchanged through the candidate | yes | yes |
| Baseline-path semantics unchanged | yes | yes |
| Required family members qualify together | yes | yes |
| Useful dependency closure qualifies | no | no |

The declaration review compared the authoritative C header, header-only C++
wrapper, and Python binding at `v0.4.0-alpha.1` with the candidate. No included
declaration or projection changed. The resolution implementation changed only
to add behavior-neutral opt-in trace instrumentation.

External-identifier attachment gained semantic-conflict participation for a
transaction created through the new base-revision entry point. That entry point
did not exist in 0.4 and is not part of the external-identifier family. Calls
through the unchanged 0.4 transaction path retain their observable behavior;
the additive base-revision path does not revise the baseline contract.

Ardour is not counted as another matrix host. Its executable scenario runs the
same resolver policy compiled into the patched host and proves installed
`pkg-config`, exception-enabled C++, and audio-content behavior, but it does not
execute the complete operation family through a normal interactive Ardour
callback in this environment.

## ADR 0020 amendment

The accepted amendment records this release-series decision in ADR 0020:

```text
## Release 0.5 series

Release 0.5 names no compatibility subset. All APIs in the 0.5.x series remain
experimental and may change within the series, with migration notes for
operations used by maintained integrations.

ADR 0040 evidence found the external-identifier and resolution operation
families mechanically eligible across the 0.4-to-0.5 boundary. They are not
named as a compatibility promise because each depends on production lifecycle,
and external-identifier mutation also depends on transaction lifecycle; those
foundational families did not have complete normal-path evidence from two
independent hosts. Publishing only the eligible leaf families would therefore
be misleadingly narrow.

Released production schemas continue to receive tested forward migrations
under the repository's pre-1.0 migration policy. This decision creates no ABI
or schema compatibility promise beyond that policy.
```

The same outcome is stated in `docs/abi-policy.md` and the release notes. The
0.5 release report records it as part of the completed release policy.
