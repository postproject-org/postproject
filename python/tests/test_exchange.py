"""Proposal ownership and explicit preparation preserve ordinary edit behavior."""

import copy
import tempfile
import unittest
from pathlib import Path
from uuid import UUID, uuid4

from postproject import (
    ClientId,
    ExchangeError,
    ExchangeFailure,
    HistoryId,
    InvalidArgumentError,
    Production,
    Proposal,
    ProtocolBase,
    RequestId,
)


class ExchangeTests(unittest.TestCase):
    def test_preparation_is_terminal_only_after_success_and_never_commits(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "authority.pproj") as production:
                with production.transaction() as edit:
                    edit.add_media_root("first")
                    proposal = edit.prepare_proposal(
                        ClientId(uuid4()), RequestId(uuid4())
                    )
                    with self.assertRaises(InvalidArgumentError):
                        edit.commit()
                with proposal:
                    identity = proposal.identity
                    document = proposal.document
                    self.assertIsInstance(identity.history_id, UUID)
                    with self.assertRaises(TypeError):
                        copy.copy(proposal)
                    with self.assertRaises(TypeError):
                        copy.deepcopy(proposal)
                    with Proposal.parse(document) as transported:
                        self.assertEqual(transported.identity, identity)
                        self.assertEqual(transported.document, document)
                self.assertIsNone(production.latest_revision)
                self.assertEqual(production.media_roots, ())
                with self.assertRaises(InvalidArgumentError):
                    _ = proposal.identity
                with production.read_session() as view:
                    base = view.decision_base
                with production.edit(base) as edit:
                    with self.assertRaises(ExchangeError) as rejected:
                        edit.prepare_proposal(
                            identity.client_id,
                            identity.request_id,
                            ProtocolBase(base, HistoryId(uuid4())),
                        )
                    self.assertEqual(
                        rejected.exception.kind, ExchangeFailure.SCOPE_MISMATCH
                    )
                    edit.add_media_root("after failure")
                    receipt = edit.commit()
                    self.assertIsNotNone(receipt.revision)

    def test_transport_rejects_malformed_values_without_a_handle(self) -> None:
        for malformed in (b"{}", b'{"version":1,"version":1}', b"\xff"):
            with self.subTest(document=malformed):
                with self.assertRaises(ExchangeError) as rejected:
                    Proposal.parse(malformed)
                self.assertEqual(rejected.exception.kind, ExchangeFailure.MALFORMED)


if __name__ == "__main__":
    unittest.main()
