"""Verify reopened measured scale stores using an installed wheel and bounded reads.

Canonical section hashes exclude each export's random checkpoint identity. This
check runs after timing; its memory and query costs are separate measurements.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
from pathlib import Path

import postproject as pp


def bodies(path: Path) -> dict[str, str]:
    hashes = {}
    with path.open("rb") as stream:
        assert stream.read(8) == b"PPXC\0\0\0\1"
        while True:
            length = int.from_bytes(stream.read(8), "big")
            assert 0 < length <= 2 * 1024 * 1024
            raw = stream.read(length)
            assert len(raw) == length
            value = json.loads(raw)
            if value["kind"] == "checkpoint.manifest":
                assert int.from_bytes(stream.read(8), "big") == length
                assert not stream.read(1)
                return {
                    section["section"]: hashes[section["section"]].hexdigest()
                    if section["section"] in hashes
                    else hashlib.sha256(b"").hexdigest()
                    for section in value["sections"]
                }
            assert value["kind"] == "checkpoint.chunk"
            hashes.setdefault(value["section"], hashlib.sha256()).update(
                base64.b64decode(value["payload"], validate=True)
            )


def counts(production: pp.Production) -> tuple[int, int, int]:
    assets = representations = resources = 0
    cursor = None
    with production.read_session() as session:
        while True:
            page = session.assets_page(limit=1000, cursor=cursor)
            assets += len(page.items)
            for asset in page.items:
                representation_page = session.representations_page(asset.id, limit=10)
                assert representation_page.next_cursor is None
                for representation in representation_page.items:
                    representations += 1
                    resource_page = session.resources_page(representation.id, limit=10)
                    assert resource_page.next_cursor is None
                    resources += len(resource_page.items)
            cursor = page.next_cursor
            if cursor is None:
                break
    return assets, representations, resources


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    results = []
    for directory in sorted(args.root.glob("scale-*")):
        with (
            pp.Production.open(directory / "authority.pproj") as source,
            pp.Production.open(directory / "mirror.pproj") as mirror,
        ):
            assert source.exchange_state.head == mirror.exchange_state.head
            assert source.exchange_state.head is not None
            revision = source.exchange_state.head.base.decision.revision
            assert revision is not None and revision.sequence == 1001
            assert mirror.exchange_state.role == pp.ExchangeRole.PASSIVE_MIRROR
            expected = counts(source)
            assert expected == (11_000, 11_000, 11_000) == counts(mirror)
            hashes = []
            for label, production in (("source", source), ("mirror", mirror)):
                path = directory / f"verified-{label}.ppxc"
                production.export_checkpoint(path)
                hashes.append(bodies(path))
            assert hashes[0] == hashes[1]
            results.append(
                {
                    "sample": directory.name,
                    "head_sequence": revision.sequence,
                    "counts": expected,
                    "section_sha256": hashes[0],
                }
            )
    assert results
    args.output.write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
