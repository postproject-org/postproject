# 0081: Stage produced media and its actual activity together

Status: accepted for development C ABI 56.

Render callers currently prepare representations, construct trivial activity
edges, create an activity and attach its parameters in separate calls. These
facts describe one completed operation and must publish together.

Add a canonical output-registration task accepting output sources, input IDs
with optional roles, actual tool/agent attribution, timing and repeated typed
parameters. Each output explicitly chooses an existing asset or a new original
asset. Return every asset/representation pair and the producing activity ID.
Single files take paths; compound sources retain their existing structure.

Native staging validates and prepares all values before appending mutations.
Failure returns cleared outputs and leaves the edit unchanged. Commit remains
explicit. Rust preparation performs media I/O before opening its transaction;
its owned staging helper rolls back on failure and returns the edit on success.
Keep lower-level creation for activities describing existing representations.
Consolidate native activity inputs into the same borrowed recipe structure.

Bare representation IDs serve ordinary edges. Standard pairs carry explicit
roles. Read edges retain their captured snapshots; they are not write builders.
Keep zero-input generation and multiple inputs/outputs, all graph checks,
actual parameter attribution and automatic complete effect capture/replay.

Registration creates no job or authored dependency observation. When a real
job exists, compose guarded completion in the same edit and retain its lease
through commit. Rust's guarded path preserves its backend completion contract.
Recorded recipe completeness remains a read, separate from execution.

ABI signatures/exports and loader guards change together. Schema 27, exchange
protocol 1 and CLI JSON 1 remain unchanged; the CLI adds a task operation.
Standards-policy review: no identifier, vocabulary or metadata mapping changes.
