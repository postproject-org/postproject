"""Independently seal small fixed exchange fixtures and rehash changed facts.

This qualification helper uses standard-library JSON, with the separately
verified BLAKE3 primitive. It never calls the PostProject codec or bindings.
"""

from __future__ import annotations

import base64
import copy
import json
from pathlib import Path

from check_exchange_vectors import canonical

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures/exchange"
MAX_BYTES = 2 * 1024 * 1024


def fixture(name: str) -> dict:
    path = ROOT / name
    assert path.stat().st_size <= MAX_BYTES
    return json.loads(path.read_text())


def encoded(value: dict) -> bytes:
    return canonical(json.dumps(value, ensure_ascii=False))


def digest(value: dict, domain: str) -> str:
    from blake3 import blake3

    return blake3(
        encoded(value), derive_key_context=f"postproject.exchange.v1.{domain}"
    ).hexdigest()


def framed(values: list[dict]) -> bytes:
    data = bytearray()
    for value in values:
        item = encoded(value)
        data.extend(len(item).to_bytes(8, "big") + item)
        assert len(data) <= MAX_BYTES
    return bytes(data)


def seal(path: Path, magic: bytes, values: list[dict], manifest: dict) -> None:
    data = (
        magic + framed(values + [manifest]) + len(encoded(manifest)).to_bytes(8, "big")
    )
    assert len(data) <= MAX_BYTES
    with path.open("xb") as stream:
        stream.write(data)


def checkpoint_file(path: Path, snapshot: dict) -> None:
    seal(path, b"PPXC\0\0\0\1", snapshot["chunks"], snapshot["manifest"])


def changes_file(path: Path, record: dict) -> None:
    seal(
        path,
        b"PPXD\0\0\0\1",
        [record["manifest"]["predecessor"], record["manifest"], *record["chunks"]],
        record["head"],
    )


def rechunk(frames: list[dict], template: dict) -> tuple[list[dict], dict]:
    payload = framed(frames)
    assert payload
    chunks = []
    previous = None
    for index, part in enumerate([payload[:3], payload[3:]]):
        chunk = {
            key: copy.deepcopy(value)
            for key, value in template.items()
            if key != "digest"
        }
        chunk.update(
            index=str(index), previous=previous, payload=base64.b64encode(part).decode()
        )
        chunk["digest"] = digest(chunk, "chunk")
        previous = chunk["digest"]
        chunks.append(chunk)
    summary = {
        "count": str(len(chunks)),
        "payload_bytes": str(len(payload)),
        "last_digest": previous,
    }
    return chunks, summary


def rehash_record(record: dict) -> None:
    chunks, summary = rechunk(record["body"], record["chunks"][0])
    record["chunks"] = chunks
    manifest = record["manifest"]
    manifest["chunks"] = summary
    del manifest["digest"], manifest["record_digest"]
    manifest["record_digest"] = digest(manifest, "record")
    manifest["digest"] = digest(manifest, "manifest")
    record["head"]["digest"] = manifest["record_digest"]


def rehash_checkpoint(snapshot: dict) -> None:
    chunks = []
    for section in snapshot["manifest"]["sections"]:
        frames = snapshot["bodies"].get(section["section"], [])
        if not frames:
            section["chunks"] = None
            continue
        template = next(
            chunk
            for chunk in snapshot["chunks"]
            if chunk["section"] == section["section"]
        )
        parts, section["chunks"] = rechunk(frames, template)
        chunks.extend(parts)
    snapshot["chunks"] = chunks
    del snapshot["manifest"]["digest"]
    snapshot["manifest"]["digest"] = digest(snapshot["manifest"], "manifest")
