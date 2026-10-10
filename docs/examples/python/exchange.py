"""Prepare portable intent with typed domain calls and explicit lifecycle."""

import sys
from pathlib import Path
from uuid import uuid4

from postproject import (
    ClientId,
    InvalidArgumentError,
    MetadataProperty,
    MetadataU64,
    Production,
    ProductionRef,
    Proposal,
    RequestId,
)


def exercise(work: Path) -> None:
    with Production.create(work / "authority.pproj") as production:
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
        proposal.close()
        try:
            _ = proposal.document
        except InvalidArgumentError:
            pass
        else:
            raise AssertionError("closed proposal should reject inspection")


if __name__ == "__main__":
    exercise(Path(sys.argv[1]))
