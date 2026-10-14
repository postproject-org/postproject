"""Extend a real host authority with typed compound and historical knowledge.

Use an installed Python wheel and a fresh host fixture directory. This helper
is a synthetic protocol qualifier, not another independent host application.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from uuid import uuid4

import postproject as pp
from exchange_knowledge import prepare


def proposal(
    source: pp.Production, client: pp.ClientId, commands: list[pp.ProposalCommand]
) -> pp.Proposal:
    with source.proposal_builder(
        client, pp.RequestId(uuid4()), base=source.exchange_state.observed_base
    ) as builder:
        builder.stage(commands)
        return builder.prepare()


def submit(source: pp.Production, value: pp.Proposal) -> pp.SubmissionOutcome:
    delivered = source.submit_proposal(value)
    assert isinstance(delivered.outcome.status, pp.CommitReceipt)
    assert delivered.leases == []
    return delivered.outcome


def run(root: Path) -> None:
    path = root / "authority.pproj"
    client = pp.ClientId(uuid4())
    with pp.Production.open(path) as source:
        assert source.exchange_state.role is pp.ExchangeRole.AUTHORITY
        prepared, imported, configured, sequence = prepare(
            source, root, client, pp.RequestId(uuid4())
        )
        # This deliberate transport extension test preserves typed preparation;
        # ordinary domain commands above never construct wire JSON.
        document = json.loads(prepared.document)
        document["extensions"]["vendor:qualification"] = {
            "ordered": ["同", "same", "同"]
        }
        with prepared, pp.Proposal.parse(json.dumps(document).encode()) as extended:
            original_outcome = submit(source, extended)
            retained_identity = extended.identity
            assert submit(source, extended).document == original_outcome.document
        original = next(
            item
            for item in source.representations[imported.asset_id]
            if item.kind is pp.RepresentationKind.ORIGINAL
        )
        resource = original.resources[0]
        dependency = pp.Dependency(
            "vendor:Unknown",
            pp.AssetRef(imported.asset_id),
            "Exact://同 😀",
            resolved_representation_id=original.id,
        )
        with proposal(
            source,
            client,
            [
                pp.RecordResourceFingerprintCommand(
                    resource.id, pp.Fingerprint("EXACT_resource", 7, b"\0\xff")
                ),
                pp.RecordRepresentationFingerprintCommand(
                    original.id, pp.Fingerprint("EXACT_representation", 3, b"\xff\0")
                ),
                pp.RecordDependencySetCommand(
                    sequence.representation_id, [dependency, dependency]
                ),
                pp.CreateActivityCommand(
                    pp.ActivitySpec(
                        "vendor:Build",
                        outputs=(sequence.representation_id,),
                        inputs=((original.id, "vendor:source"),),
                        started_at_unix_micros=-(2**53 + 1),
                        finished_at_unix_micros=2**53 + 1,
                        tool=pp.ToolIdentity("Exact 同", "Case", "urn:vendor:tool"),
                    )
                ),
            ],
        ) as publication:
            submit(source, publication)
        assert (
            source.evaluate_artifact(sequence.representation_id).state
            is pp.ArtifactKnowledgeState.CURRENT
        )
        with proposal(
            source,
            client,
            [
                pp.RecordRepresentationFingerprintCommand(
                    original.id, pp.Fingerprint("EXACT_representation", 3, b"changed")
                )
            ],
        ) as observation:
            submit(source, observation)
        assert (
            source.evaluate_artifact(sequence.representation_id).state
            is pp.ArtifactKnowledgeState.STALE
        )

        property = pp.MetadataProperty("urn:vendor:Unknown", "ordered")
        target = pp.AssetRef(imported.asset_id)
        stale = proposal(source, client, [pp.RemoveMetadataCommand(target, property)])
        with stale:
            with source.edit() as native:
                native.add_metadata(target, property, ("native suffix"))
                native.commit()
            rejected = source.submit_proposal(stale).outcome
            assert isinstance(rejected.status, pp.ConflictError)
            assert rejected.status.conflict is not None
            rejection_identity = stale.identity
            assert source.submit_proposal(stale).outcome.document == rejected.document

        (root / "knowledge" / "first.dat").write_bytes(b"new measured content")
        with proposal(
            source,
            client,
            [
                pp.ObserveResourceContentCommand(
                    resource.id, root / "knowledge" / "first.dat"
                ),
                pp.ConfirmLocatorCommand(resource.id, "file:///recorded/relocated.dat"),
                pp.RetireLocatorCommand(resource.locators[0].id),
                pp.SetMediaRootEnabledCommand(configured.root_id, False),
                pp.RemoveIdentifierCommand(
                    target,
                    pp.ExternalIdentifier("urn:vendor:EXACT", "remove", "Temporary"),
                ),
            ],
        ) as changed:
            submit(source, changed)
        transient = pp.MetadataProperty("urn:vendor:Unknown", "transient")
        with proposal(
            source,
            client,
            [
                pp.AppendMetadataCommand(target, transient, ("old")),
                pp.ReplaceMetadataCommand(
                    target,
                    transient,
                    [pp.MetadataU64(2**64 - 1), ("new 同")],
                ),
                pp.RemoveMetadataCommand(target, transient),
            ],
        ) as replaced:
            submit(source, replaced)
        assert all(item.property != transient for item in source.metadata[target])
        with proposal(source, client, []) as noop:
            receipt = submit(source, noop).status
            assert isinstance(receipt, pp.CommitReceipt) and receipt.revision is None
            assert submit(source, noop).status == receipt
        (root / "knowledge-identities.json").write_text(
            json.dumps(
                {
                    "asset": str(imported.asset_id),
                    "input": str(original.id),
                    "derived": str(sequence.representation_id),
                }
            )
        )
    with pp.Production.open(path) as reopened:
        assert (
            reopened.submission_outcome(retained_identity).document
            == original_outcome.document
        )
        assert (
            reopened.submission_outcome(rejection_identity).document
            == rejected.document
        )
        assert len(reopened.representations[imported.asset_id]) == 4
        assert len(reopened.metadata[target]) == 3
        print(
            "Prepared four content shapes, exact values, original outcomes, stale rejection and historical evidence"
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    run(parser.parse_args().root.resolve())
