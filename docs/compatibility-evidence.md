# Compatibility evidence

PostProject derives pre-1.0 compatibility candidates from maintained host
scenarios. Availability in a header, a dynamic symbol table, or a binding is
not use evidence.

## Record ABI use

Set `POSTPROJECT_ABI_TRACE` to a process-specific output path before starting a
host scenario:

```sh
POSTPROJECT_ABI_TRACE=target/evidence/blender-abi.txt blender --background …
```

The file contains each exported C operation reached at least once, sorted by
symbol name. Tracing is disabled by default. It does not change return values;
an unwritable evidence path is ignored. Give concurrent processes different
paths and pass all of them explicitly to the report tool.

Projection-only constructs have no separate C call. Record their compile or
source evidence as newline-delimited operation names matching
`docs/compatibility-families.toml`, for example:

```text
Result<T>
POSTPROJECT_TRY
POSTPROJECT_TRY_ASSIGN
```

## Generate the matrix

The report tool reads only local files:

```sh
python tools/compatibility_usage.py \
  --trace kdenlive=target/evidence/kdenlive-abi.txt \
  --trace blender=target/evidence/blender-abi.txt \
  --projection kdenlive=target/evidence/kdenlive-cpp.txt \
  --propose external-identifiers --propose resolution \
  --output target/evidence/compatibility.md
```

An ABI-backed family counts for a host only when that trace contains every C
operation in the family. A projection-only family counts when the supplied
projection evidence names every required operation of its language projection.
A C++ host need not use the Python projection, and a C host need not use either.
Projection evidence for ABI-backed families is reviewed separately from ABI
reachability. The generated eligibility
column also checks that the family existed at the baseline and did not change
after it.

The manifest records required family dependencies. The report validates the
graph, rejects missing names and cycles, and shows the sorted transitive
closure of each proposed subset. A mechanically eligible family remains
ineligible with dependencies when any required family fails the same checks.
Ownership and error contracts need review even when they have no separate
manifest family. `--propose` prepares evidence; it does not approve a promise.

The manifest and report are evidence structures, not stability promises. A
release names a compatibility subset only through the ABI policy and ADR 0020.
