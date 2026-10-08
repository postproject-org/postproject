"""Independently check canonical JSON with Python's standard library only.

This checker does not import the Rust codec, PostProject bindings or a BLAKE3
implementation. The Rust suite checks the published upstream hash vectors.
"""

from __future__ import annotations

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
    vectors = json.loads(
        (ROOT / "tests/fixtures/exchange/canonical-json.json").read_text()
    )
    for vector in vectors:
        assert canonical(vector["input"]) == vector["canonical"].encode("utf-8")
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
    print(f"{len(vectors)} independent canonical-byte vectors checked")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
