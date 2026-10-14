"""Verify a host-produced authority through two distinct checkpoint/suffix routes.

Run with an installed Python wheel. The host fixture must provide authority.pproj
and initial/later.ppxc files; mirror workers are independent processes.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import postproject as pp
from exchange_local_resolution import check as check_mapping
from exchange_observations import checkpoint_bodies, exact, public_observations


def worker(root: Path, directory: Path, name: str) -> None:
    path = directory / f"{name}.pproj"
    with pp.Production.import_checkpoint(root / f"{name}.ppxc", path) as mirror:
        original = mirror.exchange_state.head
        head = mirror.apply_changes(directory / f"{name}.ppxd")
        assert head != original
        assert mirror.apply_changes(directory / f"{name}.ppxd") == head
    with pp.Production.open(path) as reopened:
        assert reopened.exchange_state.role is pp.exchange.ExchangeRole.PASSIVE_MIRROR
        assert reopened.exchange_state.head == head
        for operation in (
            reopened.edit,
            lambda: reopened.edit(head.base.decision),
        ):
            try:
                operation()
            except pp.UnsupportedError:
                pass
            else:
                raise AssertionError("passive write accepted")
        if (root / "local-resolution.json").exists():
            check_mapping(reopened, root, directory / f"{name}-local-media")
        checkpoint = directory / f"{name}-final.ppxc"
        reopened.export_checkpoint(checkpoint)
        observations = {
            "head": exact(head),
            "portable_section_sha256": checkpoint_bodies(checkpoint),
            "public": public_observations(reopened),
        }
    (directory / f"{name}.json").write_text(
        json.dumps(observations, ensure_ascii=False)
    )


def compare(root: Path) -> None:
    early = pp.Production.validate_checkpoint(root / "initial.ppxc")
    later = pp.Production.validate_checkpoint(root / "later.ppxc")
    assert early.base.decision.revision is None
    assert later.base.decision.revision is not None
    with tempfile.TemporaryDirectory(prefix="exchange-host-", dir=root) as temporary:
        directory = Path(temporary)
        with pp.Production.open(root / "authority.pproj") as source:
            head = source.exchange_state.head
            assert head is not None and head != later
            if (root / "local-resolution.json").exists():
                check_mapping(source, root, directory / "authority-local-media")
            checkpoint = directory / "authority-final.ppxc"
            source.export_checkpoint(checkpoint)
            for name, position in (("initial", early), ("later", later)):
                assert (
                    source.export_changes(position, directory / f"{name}.ppxd") == head
                )
            expected = {
                "head": exact(head),
                "portable_section_sha256": checkpoint_bodies(checkpoint),
                "public": public_observations(source),
            }
        for name in ("initial", "later"):
            subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    str(root),
                    "--worker",
                    str(directory),
                    name,
                ],
                check=True,
            )
            actual = json.loads((directory / f"{name}.json").read_text())
            assert actual == expected, f"{name} checkpoint/suffix differs from source"
        print(
            json.dumps(
                {
                    "head": expected["head"],
                    "portable_section_sha256": expected["portable_section_sha256"],
                    "assets": len(expected["public"]["assets"]),
                    "activities": len(expected["public"]["activities"]),
                    "jobs": len(expected["public"]["jobs"]),
                    "distinct_local_mappings": (
                        root / "local-resolution.json"
                    ).exists(),
                    "routes": "two independent passive processes; duplicate apply and reopen passed",
                    "observations": "exact bodies, domain values, metadata/identifiers, dependency evidence, artifact results and revisions/events agree",
                },
                ensure_ascii=False,
                indent=2,
            )
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--worker", nargs=2)
    args = parser.parse_args()
    if args.worker:
        worker(args.root.resolve(), Path(args.worker[0]), args.worker[1])
    else:
        compare(args.root.resolve())
