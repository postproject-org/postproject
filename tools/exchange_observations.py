"""Compare portable checkpoint bodies and ordinary public reads in small fixtures."""

from __future__ import annotations

import base64
import dataclasses
import hashlib
import json
from enum import Enum
from fractions import Fraction
from pathlib import Path
from uuid import UUID

import postproject as pp


def exact(value):
    """Keep public alternatives and exact scalar values in the comparison."""
    if dataclasses.is_dataclass(value):
        return {
            "type": type(value).__name__,
            **{
                field.name: exact(getattr(value, field.name))
                for field in dataclasses.fields(value)
            },
        }
    if isinstance(value, UUID):
        return str(value)
    if isinstance(value, Enum):
        return {"type": type(value).__name__, "value": value.value}
    if isinstance(value, Fraction):
        return {
            "numerator": str(value.numerator),
            "denominator": str(value.denominator),
        }
    if isinstance(value, bytes):
        return {"bytes": base64.b64encode(value).decode()}
    if isinstance(value, (tuple, list)):
        return [exact(item) for item in value]
    if isinstance(value, dict):
        return {key: exact(item) for key, item in value.items()}
    if isinstance(value, int) and not isinstance(value, bool):
        return {"integer": str(value)}
    if value is None or isinstance(value, (str, bool)):
        return value
    raise TypeError(f"unsupported public comparison value: {type(value).__name__}")


def checkpoint_bodies(path: Path) -> str:
    """Hash exact framed section bytes, excluding this export's random identity.

    Input is a freshly exported, publicly validated small-fixture checkpoint.
    This comparison hash is SHA256, separate from protocol BLAKE3 integrity.
    """
    pp.Production.validate_checkpoint(path)
    payloads = {}
    with path.open("rb") as stream:
        assert stream.read(8) == b"PPXC\0\0\0\1"
        while True:
            length = int.from_bytes(stream.read(8), "big")
            assert 1 <= length <= 512 * 1024
            raw = stream.read(length)
            assert len(raw) == length
            value = json.loads(raw)
            if value["kind"] == "checkpoint.manifest":
                assert int.from_bytes(stream.read(8), "big") == length
                assert not stream.read(1)
                manifest = value
                break
            assert value["kind"] == "checkpoint.chunk"
            payloads.setdefault(value["section"], bytearray()).extend(
                base64.b64decode(value["payload"], validate=True)
            )
    digest = hashlib.sha256()
    for section in manifest["sections"]:
        name = section["section"].encode()
        payload = payloads.pop(section["section"], b"")
        digest.update(len(name).to_bytes(8, "big") + name)
        digest.update(int(section["items"]).to_bytes(8, "big"))
        digest.update(len(payload).to_bytes(8, "big") + payload)
    assert not payloads
    return digest.hexdigest()


def public_observations(source: pp.Production) -> dict:
    before = source.exchange_state.head
    with source.read_session() as view:
        assets = tuple(view.assets())
    representations = tuple(
        item for asset in assets for item in source.representations[asset.id]
    )
    resources = {
        resource.id: resource for item in representations for resource in item.resources
    }
    activities = source.activities
    jobs = source.jobs(limit=1000)
    assert jobs.next_cursor is None
    targets: list[pp.ObjectReference] = [pp.ProductionRef(source.id)]
    targets.extend(pp.AssetRef(item.id) for item in assets)
    targets.extend(pp.RepresentationRef(item.id) for item in representations)
    targets.extend(pp.ResourceRef(item.id) for item in resources.values())
    targets.extend(pp.ActivityRef(item.id) for item in activities)
    targets.extend(pp.jobs.JobRef(item.id) for item in jobs.items)
    revisions = source.changes_since(0, 1000)
    assert before is not None and before.base.decision.revision is not None
    assert len(revisions) == before.base.decision.revision.sequence
    result = {
        "assets": assets,
        "representations": representations,
        "activities": activities,
        "roots": source.media_roots,
        "jobs": jobs.items,
        "metadata": [(target, source.metadata[target]) for target in targets],
        "identifiers": [
            (target, source.external_identifiers[target])
            for target in targets
            if isinstance(
                target,
                (pp.AssetRef, pp.RepresentationRef, pp.ResourceRef, pp.ActivityRef),
            )
        ],
        "external_lookup": [
            (
                identifier,
                source.objects_by_external_identifier[
                    (identifier.scheme, identifier.value)
                    if identifier.qualifier is None
                    else (identifier.scheme, identifier.value, identifier.qualifier)
                ],
            )
            for target in targets
            if isinstance(
                target,
                (pp.AssetRef, pp.RepresentationRef, pp.ResourceRef, pp.ActivityRef),
            )
            for identifier in source.external_identifiers[target]
        ],
        "dependencies": [
            (item.id, source.dependency_set(item.id)) for item in representations
        ],
        "artifacts": [
            (
                item.id,
                source.evaluate_artifact(item.id),
                source.artifact_reproducibility(item.id),
            )
            for item in representations
        ],
        "revisions": revisions,
        "events": [(item.id, source.revision_events[item.id]) for item in revisions],
    }
    assert source.exchange_state.head == before
    return exact(result)
