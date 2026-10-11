"""Prove that passive local root mappings stay outside portable knowledge."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from uuid import UUID, uuid4

import postproject as pp

CONTENT = b"same portable bytes at different machine paths"
FILENAME = "portable-local.dat"
ROOT = "machine-local"


def prepare(root: Path) -> None:
    recorded = root / FILENAME
    recorded.write_bytes(CONTENT)
    with pp.Production.open(root / "authority.pproj") as source:
        with source.proposal_builder(
            pp.ClientId(uuid4()), pp.RequestId(uuid4())
        ) as builder:
            prepared = builder.stage(
                [
                    pp.ImportMediaCommand(pp.FileSource(recorded)),
                    pp.AddMediaRootCommand(ROOT),
                ]
            )[0]
            assert isinstance(prepared, pp.PreparedAsset)
            with builder.prepare() as proposal:
                outcome = source.submit_proposal(proposal).outcome
                assert isinstance(outcome.status, pp.CommitReceipt)
        (root / "local-resolution.json").write_text(
            json.dumps({"asset": str(prepared.asset_id)})
        )
    # A real move leaves the recorded locator unavailable. No portable mapping
    # or automatic confirmation is added when each reader discovers its copy.
    directory = root / "machine-source"
    directory.mkdir()
    recorded.rename(directory / FILENAME)


def check(source: pp.Production, root: Path, directory: Path) -> None:
    identities = json.loads((root / "local-resolution.json").read_text())
    asset = pp.AssetId(UUID(identities["asset"]))
    directory.mkdir()
    path = directory / FILENAME
    path.write_bytes(CONTENT)
    head = source.exchange_state.head
    result = source.resolve(asset, {ROOT: directory})
    assert len(result) == 1 and len(result[0].resources) == 1
    resource = result[0].resources[0]
    assert resource.state is pp.ResourceResolutionState.RESOLVED_EXACT
    assert len(resource.candidates) == 1
    assert resource.candidates[0].uri == path.as_uri()
    assert resource.candidates[0].media_root == ROOT
    assert source.exchange_state.head == head


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    prepare(parser.parse_args().root.resolve())
