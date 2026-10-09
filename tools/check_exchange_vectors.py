"""Independently check canonical JSON with Python's standard library only.

This checker never imports the PostProject codec or bindings. Canonical checks
use Python's standard library; optional digest checks use a separately verified
BLAKE3 binding (uv run --no-project --with blake3==1.0.10 ... --digests).
"""

from __future__ import annotations

import argparse
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
        f"{len(vectors)} framing and {len(proposals)} independent proposal-byte vectors checked"
    )
    if options.digests:
        print(f"{len(proposals)} proposal digests and upstream BLAKE3 vectors checked")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
