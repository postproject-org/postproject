"""Race two installed-SDK writers from one base in a real host history."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from uuid import uuid4

import postproject as pp
from prepare_exchange_knowledge import proposal, submit

PROPERTY = pp.MetadataProperty("urn:vendor:Unknown", "process-race")


def worker(root: Path, name: str) -> None:
    with pp.Production.open(root / "authority.pproj") as source:
        target = pp.ProductionRef(source.id)
        with proposal(
            source,
            pp.exchange.ClientId(uuid4()),
            [pp.exchange.AppendMetadataCommand(target, PROPERTY, (name))],
        ) as prepared:
            print("ready", flush=True)
            assert sys.stdin.readline() == "submit\n"
            outcome = submit(source, prepared)
            (root / f"race-{name}.json").write_text(
                json.dumps(
                    {
                        "proposal": prepared.document.decode(),
                        "outcome": outcome.document.decode(),
                    }
                )
            )


def race(root: Path) -> None:
    client = pp.exchange.ClientId(uuid4())
    path = root / "authority.pproj"
    with pp.Production.open(path) as source:
        target = pp.ProductionRef(source.id)
        assert all(item.property != PROPERTY for item in source.metadata[target])
        with proposal(
            source,
            client,
            [pp.exchange.AppendMetadataCommand(target, PROPERTY, ("seed"))],
        ) as seed:
            submit(source, seed)
        stale = proposal(
            source, client, [pp.exchange.RemoveMetadataCommand(target, PROPERTY)]
        )
        original = source.exchange_state.head
        assert original is not None
    workers: list[subprocess.Popen[str]] = []
    try:
        for name in ("left", "right"):
            workers.append(
                subprocess.Popen(
                    [
                        sys.executable,
                        str(Path(__file__).resolve()),
                        str(root),
                        "--worker",
                        name,
                    ],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    text=True,
                )
            )
        for process in workers:
            assert process.stdout is not None
            assert process.stdout.readline() == "ready\n", "writer failed to prepare"
        for process in workers:
            assert process.stdin is not None
            process.stdin.write("submit\n")
            process.stdin.flush()
        for process in workers:
            assert process.wait() == 0, "writer failed to submit"
    finally:
        for process in workers:
            if process.poll() is None:
                process.terminate()
            process.wait()
            if process.stdin is not None:
                process.stdin.close()
            if process.stdout is not None:
                process.stdout.close()
    receipts: list[int] = []
    with stale, pp.Production.open(path) as source:
        head = source.exchange_state.head
        assert head is not None and head.base.decision.revision is not None
        assert original.base.decision.revision is not None
        assert (
            head.base.decision.revision.sequence
            == original.base.decision.revision.sequence + 2
        )
        rejected = source.submit_proposal(stale).outcome
        assert isinstance(rejected.status, pp.ConflictError)
        assert rejected.status.conflict is not None
        assert source.submit_proposal(stale).outcome.document == rejected.document
        assert source.exchange_state.head == head
        values = [
            item.value for item in source.metadata[target] if item.property == PROPERTY
        ]
        assert len(values) == 3
        assert set(values) == {(name) for name in ("seed", "left", "right")}
        for name in ("left", "right"):
            saved = json.loads((root / f"race-{name}.json").read_text())
            with pp.exchange.Proposal.parse(saved["proposal"].encode()) as prepared:
                outcome = source.submission_outcome(prepared.identity)
                assert outcome.document.decode() == saved["outcome"]
                assert (
                    source.submit_proposal(prepared).outcome.document
                    == outcome.document
                )
                assert isinstance(outcome.status, pp.CommitReceipt)
                assert outcome.status.revision is not None
                receipts.append(outcome.status.revision.sequence)
        assert sorted(receipts) == [
            head.base.decision.revision.sequence - 1,
            head.base.decision.revision.sequence,
        ]
    print(
        "Two prepared writer processes merged appends; stale removal rejected; own receipts recovered"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--worker")
    args = parser.parse_args()
    if args.worker:
        worker(args.root.resolve(), args.worker)
    else:
        race(args.root.resolve())
