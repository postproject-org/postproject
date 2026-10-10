"""Prepare portable intent with typed domain calls and explicit lifecycle."""

import sys
from pathlib import Path
from uuid import uuid4

from postproject import (
    ClientId,
    ExchangeLimits,
    ExchangeRole,
    InvalidArgumentError,
    MetadataProperty,
    MetadataU64,
    Production,
    ProductionRef,
    Proposal,
    RequestId,
    UnsupportedError,
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
                mirror.transaction()
            except UnsupportedError:
                pass
            else:
                raise AssertionError("passive ordinary edits must reject")
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
        with proposal, Proposal.parse(proposal.document) as transported:
            identity = proposal.identity
            assert transported.identity == identity
            assert identity.client_id == client and identity.request_id == request
        # [/exchange-prepare]
        assert production.latest_revision is None
        assert not production.media_roots
        # [exchange-catch-up]
        changes = work / "changes.ppxd"
        through = production.export_changes(saved, changes)
        with Production.open(mirror_path) as mirror:
            assert mirror.apply_changes(changes) == through
        with Production.open(mirror_path) as mirror:
            assert mirror.apply_changes(changes) == through
            assert mirror.exchange_state.head == through
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
