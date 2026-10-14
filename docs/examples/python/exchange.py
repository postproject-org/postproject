"""Prepare portable intent with typed domain calls and explicit lifecycle."""

import sys
from pathlib import Path
from uuid import uuid4

from postproject import (
    CommitReceipt,
    InvalidArgumentError,
    MetadataProperty,
    MetadataU64,
    Production,
    ProductionRef,
    UnsupportedError,
)
from postproject.exchange import (
    AddMediaRootCommand,
    ClientId,
    ExchangeLimits,
    ExchangeRole,
    PreparedMediaRoot,
    Proposal,
    RequestId,
)


def exercise(work: Path) -> None:
    with Production.create(work / "authority.pproj") as production:
        # [exchange-checkpoint]
        checkpoint = work / "checkpoint.ppxc"
        saved = production.export_checkpoint(checkpoint)
        assert Production.validate_checkpoint(checkpoint) == saved
        mirror_path = work / "mirror.pproj"
        with Production.import_checkpoint(checkpoint, mirror_path) as mirror:
            state = mirror.exchange_state
            assert state.role is ExchangeRole.PASSIVE_MIRROR
            assert state.head == saved
        # [/exchange-checkpoint]
        assert state.limits == ExchangeLimits()
        with Production.open(mirror_path) as mirror:
            try:
                mirror.edit()
            except UnsupportedError:
                pass
            else:
                raise AssertionError("passive ordinary edits must reject")
        # [exchange-command-collection]
        with production.proposal_builder(
            ClientId(uuid4()),
            RequestId(uuid4()),
            base=state.observed_base,
        ) as builder:
            results = builder.stage([AddMediaRootCommand("discarded")])
            assert isinstance(results[0], PreparedMediaRoot)
            with builder.prepare() as discarded:
                assert discarded.identity.history_id == state.observed_base.history_id
            builder.close()
        assert not production.media_roots  # Preparation/discard never commits.
        # [/exchange-command-collection]
        # [exchange-prepare]
        client, request = ClientId(uuid4()), RequestId(uuid4())
        with production.read_session() as view, view.edit() as edit:
            edit.add_metadata(
                ProductionRef(production.id),
                MetadataProperty("example:Exact", "count"),
                MetadataU64(2**64 - 1),
            )
            edit.add_media_root("rushes", priority=-5)
            proposal = edit.prepare_proposal(client, request)
        # [/exchange-prepare]
        assert production.latest_revision is None
        assert not production.media_roots
        # [exchange-submit]
        with proposal, Proposal.parse(proposal.document) as transported:
            identity = proposal.identity
            assert transported.identity == identity
            assert identity.client_id == client and identity.request_id == request
            submitted = production.submit_proposal(transported)
            assert isinstance(submitted.outcome.status, CommitReceipt)
            assert submitted.outcome.status.revision is not None
            assert submitted.outcome.status.revision.sequence == 1
            assert not submitted.leases
            with Production.open(work / "authority.pproj") as reopened:
                recovered = reopened.submission_outcome(identity)
                duplicate = reopened.submit_proposal(proposal)
                assert recovered.document == submitted.outcome.document
                assert duplicate.outcome.document == recovered.document
                assert not duplicate.leases
        # [/exchange-submit]
        # [exchange-catch-up]
        changes = work / "changes.ppxd"
        through = production.export_changes(saved, changes)
        with Production.open(mirror_path) as mirror:
            assert mirror.apply_changes(changes) == through
        with Production.open(mirror_path) as mirror:
            assert mirror.apply_changes(changes) == through
            assert mirror.exchange_state.head == through
            assert mirror.metadata[ProductionRef(mirror.id)][0].value == MetadataU64(
                2**64 - 1
            )
            assert len(mirror.media_roots) == 1
        # [/exchange-catch-up]
        assert (
            production.resynchronize_history(
                production.exchange_state.observed_base,
                records=100,
                encoded_bytes=1048576,
            )
            == saved
        )
        proposal.close()
        try:
            _ = proposal.document
        except InvalidArgumentError:
            pass
        else:
            raise AssertionError("closed proposal should reject inspection")


if __name__ == "__main__":
    exercise(Path(sys.argv[1]))
