"""Qualify rehashed malformed files through an installed Python/native package.

Run with the installed wheel and blake3==1.0.10. Fixtures and individual encoded
files are bounded to 2 MiB. No PostProject source import or SQL inspection occurs.
"""

from __future__ import annotations

import argparse
import copy
import json
import tempfile
from pathlib import Path

import postproject as pp
from exchange_fixture_files import (
    changes_file,
    checkpoint_file,
    fixture,
    rehash_checkpoint,
    rehash_record,
)
from exchange_observations import checkpoint_bodies, public_observations
from exchange_storage_cases import checkpoint_cases, record_cases


def baseline(root: Path, record: dict) -> pp.Production:
    sequence = int(record["manifest"]["predecessor"]["sequence"])
    profiles = {
        0: "genesis",
        1: "media",
        2: "access",
        3: "knowledge",
        4: "provenance",
        7: "jobs",
    }
    start = max(item for item in profiles if item <= sequence)
    path = root / "baseline.ppxc"
    checkpoint_file(path, fixture(f"checkpoints/{profiles[start]}.json"))
    mirror = pp.Production.import_checkpoint(path, root / "mirror.pproj")
    try:
        for index, name in enumerate(record.get("prefix", [])):
            prior = fixture(name)
            if int(prior["head"]["sequence"]) > start:
                path = root / f"prefix-{index}.ppxd"
                changes_file(path, prior)
                mirror.apply_changes(path)
        head = mirror.exchange_state.head
        assert head is not None
        revision = head.base.decision.revision
        assert (revision.sequence if revision is not None else 0) == sequence
        return mirror
    except BaseException:
        mirror.close()
        raise


def record_rejections() -> list[dict]:
    results = []
    for name, profile, expected, mutate in record_cases():
        original = fixture(f"streams/{profile}.json")
        altered = copy.deepcopy(original)
        mutate(altered)
        rehash_record(altered)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            control = root / "control"
            control.mkdir()
            with baseline(control, original) as mirror:
                changes_file(control / "valid.ppxd", original)
                head = mirror.apply_changes(control / "valid.ppxd")
                assert head.base.decision.revision is not None
                assert head.base.decision.revision.sequence == int(
                    original["head"]["sequence"]
                )
            with baseline(root, original) as mirror:
                before = public_observations(mirror)
                mirror.export_checkpoint(root / "before.ppxc")
                changes_file(root / "altered.ppxd", altered)
                try:
                    mirror.apply_changes(root / "altered.ppxd")
                except pp.PostProjectError as error:
                    assert error.code == expected, (name, error.code, str(error))
                else:
                    raise AssertionError(f"accepted {name}")
                assert public_observations(mirror) == before, name
                mirror.export_checkpoint(root / "after.ppxc")
                assert checkpoint_bodies(root / "before.ppxc") == checkpoint_bodies(
                    root / "after.ppxc"
                ), name
            with pp.Production.open(root / "mirror.pproj") as reopened:
                assert public_observations(reopened) == before, name
            results.append(
                {"case": name, "failure_code": expected, "unchanged_after_reopen": True}
            )
    return results


def checkpoint_rejections() -> list[dict]:
    results = []
    for name, profile, expected, mutate in checkpoint_cases():
        snapshot = fixture(f"checkpoints/{profile}.json")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            checkpoint_file(root / "valid.ppxc", snapshot)
            with pp.Production.import_checkpoint(
                root / "valid.ppxc", root / "control.pproj"
            ) as control:
                public_observations(control)
            mutate(snapshot)
            rehash_checkpoint(snapshot)
            checkpoint_file(root / "altered.ppxc", snapshot)
            destination = root / "mirror.pproj"
            try:
                with pp.Production.import_checkpoint(
                    root / "altered.ppxc", destination
                ):
                    raise AssertionError(f"accepted {name}")
            except pp.PostProjectError as error:
                assert error.code == expected, (name, error.code, str(error))
            assert not destination.exists(), name
            results.append(
                {"case": name, "failure_code": expected, "destination_absent": True}
            )
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = {"records": record_rejections(), "checkpoints": checkpoint_rejections()}
    with args.output.open("x") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(
        f"{len(result['records'])} record and {len(result['checkpoints'])} rehashed checkpoint rejections passed"
    )


if __name__ == "__main__":
    main()
