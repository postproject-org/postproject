"""Independently check canonical JSON with Python's standard library only.

This checker never imports the PostProject codec or bindings. Canonical checks
use Python's standard library; optional digest checks use a separately verified
BLAKE3 binding (uv run --no-project --with blake3==1.0.10 ... --digests).
"""

from __future__ import annotations

import argparse
import base64
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate key")
        result[key] = value
    return result


def reject_number(_text: str) -> None:
    raise ValueError("exact numbers require strings")


def canonical(input_text: str) -> bytes:
    value = json.loads(
        input_text,
        object_pairs_hook=unique_object,
        parse_int=reject_number,
        parse_float=reject_number,
        parse_constant=reject_number,
    )
    if not isinstance(value, dict):
        raise TypeError("document must be an object")
    # UTF-8 ordering agrees with scalar order, unlike UTF-16/JCS ordering.
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8", errors="strict")


def check_stream(vector: dict, stream: dict, digests: bool = False) -> None:
    def encoded(value: object) -> bytes:
        return canonical(json.dumps(value, ensure_ascii=False))

    manifest = stream["manifest"]
    assert encoded(manifest) == vector["canonical_manifest"].encode("utf-8")
    assert manifest["predecessor"] == stream["floor"]
    assert stream["head"]["digest"] == vector["record_digest"]
    assert manifest["record_digest"] == vector["record_digest"]
    assert manifest["digest"] == vector["manifest_digest"]
    previous = None
    payload = bytearray()
    for index, chunk in enumerate(stream["chunks"]):
        assert encoded(chunk) == vector["canonical_chunks"][index].encode("utf-8")
        assert chunk["index"] == str(index) and chunk["previous"] == previous
        assert chunk["production"] == stream["floor"]["production"]
        assert chunk["history"] == stream["floor"]["history"]
        assert chunk["revision"] == manifest["revision"]["id"]
        assert chunk["digest"] == vector["chunk_digests"][index]
        raw = base64.b64decode(chunk["payload"], validate=True)
        assert base64.b64encode(raw).decode("ascii") == chunk["payload"]
        payload.extend(raw)
        previous = chunk["digest"]
    assert manifest["chunks"] == {
        "count": str(len(stream["chunks"])),
        "payload_bytes": str(len(payload)),
        "last_digest": previous,
    }
    expected = b"".join(
        len(encoded(item)).to_bytes(8, "big") + encoded(item) for item in stream["body"]
    )
    assert payload == expected
    if digests:
        from blake3 import blake3

        def digest(value: dict, domain: str) -> str:
            return blake3(
                encoded(value), derive_key_context=f"postproject.exchange.v1.{domain}"
            ).hexdigest()

        assert digest(stream["anchor"], "anchor") == vector["anchor_digest"]
        for chunk in stream["chunks"]:
            assert (
                digest({k: v for k, v in chunk.items() if k != "digest"}, "chunk")
                == chunk["digest"]
            )
        unsigned = {k: v for k, v in manifest.items() if k != "digest"}
        assert digest(unsigned, "manifest") == vector["manifest_digest"]
        del unsigned["record_digest"]
        assert digest(unsigned, "record") == vector["record_digest"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--digests", action="store_true", help="also verify BLAKE3 digests"
    )
    options = parser.parse_args()
    vectors = json.loads(
        (ROOT / "tests/fixtures/exchange/canonical-json.json").read_text()
    )
    for vector in vectors:
        assert canonical(vector["input"]) == vector["canonical"].encode("utf-8")
    proposals = json.loads(
        (ROOT / "tests/fixtures/exchange/proposal-vectors.json").read_text()
    )
    for vector in proposals:
        path = ROOT / "tests/fixtures/exchange" / vector["file"]
        assert canonical(path.read_text()) == vector["canonical"].encode("utf-8")
    streams = json.loads(
        (ROOT / "tests/fixtures/exchange/stream-vectors.json").read_text()
    )
    for vector in streams:
        path = ROOT / "tests/fixtures/exchange" / vector["file"]
        check_stream(vector, json.loads(path.read_text()), options.digests)
    if options.digests:
        from blake3 import blake3

        context = "BLAKE3 2019-12-27 16:29:52 test vectors context"
        for raw, expected in [
            (b"", "2cc39783c223154fea8dfb7c1b1660f2ac2dcbd1c1de8277b0b0dd39b7e50d7d"),
            (b"\0", "b3e2e340a117a499c6cf2398a19ee0d29cca2bb7404c73063382693bf66cb06c"),
        ]:
            assert blake3(raw, derive_key_context=context).hexdigest() == expected
        for vector in proposals:
            digest = blake3(
                vector["canonical"].encode("utf-8"),
                derive_key_context="postproject.exchange.v1.request",
            ).hexdigest()
            assert digest == vector["request_digest"]
    for invalid in [
        '{"a":null,"a":true}',
        '{"a":1}',
        '{"a":1.0}',
        '{"a":"\\ud800"}',
        "{}{}",
    ]:
        try:
            canonical(invalid)
        except (ValueError, UnicodeError):
            continue
        raise AssertionError("negative vector accepted")
    print(
        f"{len(vectors)} framing, {len(proposals)} proposal and {len(streams)} stream vectors checked"
    )
    if options.digests:
        print(
            f"{len(proposals)} proposal and {len(streams)} stream digests plus upstream BLAKE3 vectors checked"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
