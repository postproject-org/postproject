# Exchange vectors

`canonical-json.json` tests the exact JSON profile. `proposal-vectors.json` binds
the files under `proposals/` to fixed canonical bytes and request digests.
These cover all 24 command alternatives, including prepared media, exact access
facts, destructive metadata edits and the complete worker lifecycle. They check
decoding and feature declarations; individual proposals are not execution scripts.

Expected bytes were authored with Python's standard-library JSON encoder, without
the PostProject codec or bindings. Digests used `blake3==1.0.10`, checked against
[upstream BLAKE3 vectors](https://github.com/BLAKE3-team/BLAKE3/blob/master/test_vectors/test_vectors.json).
The hash binding uses the official Rust primitive; the independent implementation
here is the wire encoding. Rust checks the fixed results, rather than generating
its own expectations. `stream-vectors.json` binds independently authored metadata
and media records, split length/UTF-8 framing, genesis anchors and both record/manifest
digests. They preserve exact extreme values, repeated extensions, original media
identities and observations; storage applies the fixed records and tests duplicate
replay and reopen. `checkpoint-vectors.json` binds a complete
genesis checkpoint with all 19 sections and split length/UTF-8 framing. It imports
as an ordinary passive store. Nonempty checkpoint and remaining record families
remain incomplete.

Run `python3 tools/check_exchange_vectors.py` for canonical checks, or
`uv run --no-project --with blake3==1.0.10 python3 tools/check_exchange_vectors.py --digests`
for both bytes and digests. Review changes to fixed expectations explicitly.
