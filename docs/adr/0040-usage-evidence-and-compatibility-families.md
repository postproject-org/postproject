# ADR 0040: ABI usage evidence and compatibility families

- Status: Accepted
- Date: 2026-09-29

## Context

Exported symbols and source references show that an operation is available, not
that a host reaches it on a normal path. PostProject needs repeatable evidence
from maintained integrations before it can name another pre-1.0 compatibility
subset. Isolated functions are also the wrong unit: a useful workflow depends
on ownership, accessors, release functions, and the projections that expose
them together.

## Decision

Every exported C operation records its symbol name at the implementation
boundary when `POSTPROJECT_ABI_TRACE` names an output file. Tracing is disabled
when that variable is absent or empty. A process writes a sorted, deduplicated
newline-delimited file and rewrites it after the first reach of each operation.
Trace I/O failures are ignored, because evidence collection must not alter the
operation's result or production semantics. Acceptance jobs give each process a
distinct output file; the report tool combines explicit inputs rather than
making concurrent processes coordinate through one trace.

`docs/compatibility-families.toml` defines coherent evidence families. Each
family records all required C operations, included C++ and Python projection
operations, its introducing release, and its last-change release and ABI.
Projection-only helpers use source or compile-time evidence because no distinct
C symbol can observe them.

`tools/compatibility_usage.py` validates the manifest against the authoritative
C header and consumes explicit host trace and projection-evidence files. It
reports the family-to-host matrix, constituent operations, last-change data,
and mechanical eligibility. A family is mechanically eligible only when it
existed at the released baseline, did not change after that baseline, and has
complete evidence from at least two independent hosts. The report is evidence,
not a compatibility promise; naming a subset still requires the maintainer's
release decision and an amendment to ADR 0020.

## Alternatives considered

- Dynamic symbol inspection was rejected because loading or linking a symbol
  does not prove normal-path use.
- Wrapper-only instrumentation was rejected because it would miss direct C
  callers and produce incomparable evidence across projections.
- Manually maintained usage tables were rejected because they drift from host
  scenarios and cannot demonstrate that every family member ran.
- One shared multi-process trace file was rejected because locking and recovery
  would add production behavior solely for evidence collection.

## Standards impact

This decision changes no media, metadata, identifier, provenance, or
interoperability semantics. Trace files contain only PostProject C symbol names
and no production data. No external standard defines compatibility evidence for
this local ABI.

## Migration implications

There is no ABI or schema migration. Tracing adds no exported symbol and remains
off by default. CI and maintained pilots may opt in by setting the environment
variable and uploading the resulting file.

## Consequences

Use by C, C++, Python, and any future binding is observed at one boundary.
Evidence is deterministic and can be regenerated without network access.
Projection-only behavior remains a separate explicit input instead of being
misrepresented as an ABI call. A trace proves reachability on the executed
scenario; it does not prove semantic correctness, platform coverage, or
compatibility by itself.
