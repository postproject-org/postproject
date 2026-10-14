"""Proposal ownership and explicit preparation preserve ordinary edit behavior."""

import copy
import json
import tempfile
import unittest
from pathlib import Path
from uuid import UUID, uuid4

from postproject import (
    AlreadyExistsError,
    CommitReceipt,
    ConflictError,
    InvalidArgumentError,
    MetadataProperty,
    MetadataU64,
    NotFoundError,
    PostProjectError,
    Production,
    ProductionRef,
    UnsupportedError,
)
from postproject.exchange import (
    ClientId,
    ExchangeError,
    ExchangeFailure,
    ExchangeLimits,
    ExchangeRole,
    HistoryId,
    Proposal,
    ProtocolBase,
    RequestId,
)


class ExchangeTests(unittest.TestCase):
    def test_copied_original_receipt_and_conflict_survive_reopen(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "authority.pproj"
            client, request = ClientId(uuid4()), RequestId(uuid4())
            with Production.create(path) as production:
                target = ProductionRef(production.id)
                property = MetadataProperty("unknown:Exact名", "count")
                with production.edit() as edit:
                    edit.add_metadata(target, property, MetadataU64(2**64 - 1))
                    proposal = edit.prepare_proposal(client, request)
                with self.assertRaises(NotFoundError):
                    production.submission_outcome(proposal.identity)
                result = production.submit_proposal(proposal)
                self.assertIsInstance(result.outcome.status, CommitReceipt)
                assert isinstance(result.outcome.status, CommitReceipt)
                assert result.outcome.status.revision is not None
                self.assertEqual(result.outcome.status.revision.sequence, 1)
                state = production.exchange_state
                with production.edit(state.observed_base.decision) as edit:
                    edit.remove_metadata_property(target, property)
                    destructive = edit.prepare_proposal(
                        client, RequestId(uuid4()), state.observed_base
                    )
                with production.edit() as later:
                    later.add_metadata(target, property, MetadataU64(7))
                    later.commit()
                rejected = production.submit_proposal(destructive)
                self.assertIsInstance(rejected.outcome.status, ConflictError)
                assert isinstance(rejected.outcome.status, ConflictError)
                self.assertIsNotNone(rejected.outcome.status.conflict)
            with Production.open(path) as reopened, proposal, destructive:
                recovered = reopened.submission_outcome(proposal.identity)
                self.assertEqual(recovered.document, result.outcome.document)
                duplicate = reopened.submit_proposal(proposal)
                self.assertEqual(duplicate.outcome.document, result.outcome.document)
                self.assertEqual(duplicate.leases, [])
                conflict = reopened.submission_outcome(destructive.identity)
                self.assertEqual(conflict.document, rejected.outcome.document)
                assert isinstance(conflict.status, ConflictError)
                self.assertEqual(
                    conflict.status.conflict, rejected.outcome.status.conflict
                )
                self.assertEqual(
                    reopened.metadata[target][0].value, MetadataU64(2**64 - 1)
                )
                latest = reopened.latest_revision
                assert latest is not None
                self.assertEqual(latest.sequence, 2)
                with reopened.edit() as noop:
                    unchanged = noop.prepare_proposal(client, RequestId(uuid4()))
                with unchanged:
                    accepted = reopened.submit_proposal(unchanged).outcome.status
                    assert isinstance(accepted, CommitReceipt)
                    self.assertIsNone(accepted.revision)

    def test_passive_files_reopen_after_failed_record_and_duplicate_retry(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            checkpoint, mirror_path = work / "checkpoint", work / "mirror.pproj"
            with Production.create(work / "authority.pproj") as authority:
                saved = authority.export_checkpoint(checkpoint)
                self.assertEqual(Production.validate_checkpoint(checkpoint), saved)
                with self.assertRaises(ExchangeError) as bounded:
                    Production.validate_checkpoint(
                        checkpoint, ExchangeLimits(file_bytes=1)
                    )
                self.assertEqual(bounded.exception.kind, ExchangeFailure.LIMIT_EXCEEDED)
                with Production.import_checkpoint(checkpoint, mirror_path) as mirror:
                    state = mirror.exchange_state
                    self.assertEqual(state.role, ExchangeRole.PASSIVE_MIRROR)
                    self.assertEqual(state.head, saved)
                    for operation in (
                        mirror.edit,
                        lambda: mirror.edit(state.observed_base.decision),
                    ):
                        with self.assertRaises(UnsupportedError):
                            operation()
                    with self.assertRaises(ExchangeError) as forbidden:
                        mirror.resynchronize_history(
                            state.observed_base, records=1, encoded_bytes=4096
                        )
                    self.assertEqual(
                        forbidden.exception.kind, ExchangeFailure.MIRROR_READ_ONLY
                    )
                with self.assertRaises(AlreadyExistsError):
                    Production.import_checkpoint(checkpoint, mirror_path)
                for name in ("first", "second"):
                    with authority.edit() as edit:
                        edit.add_media_root(name)
                        edit.commit()
                    if name == "first":
                        first_head = authority.exchange_state.head
                changes, truncated = work / "changes", work / "truncated"
                through = authority.export_changes(saved, changes)
                truncated.write_bytes(changes.read_bytes()[:-16])
                with Production.open(mirror_path) as mirror:
                    with self.assertRaises(PostProjectError):
                        mirror.apply_changes(truncated)
                    self.assertEqual(mirror.exchange_state.head, saved)
                damaged = work / "damaged"
                damaged.write_bytes(corrupt_second_manifest(changes.read_bytes()))
                with Production.open(mirror_path) as mirror:
                    with self.assertRaises(ExchangeError) as invalid:
                        mirror.apply_changes(damaged)
                    self.assertEqual(invalid.exception.kind, ExchangeFailure.MALFORMED)
                with Production.open(mirror_path) as mirror:
                    self.assertEqual(mirror.exchange_state.head, first_head)
                    self.assertEqual(len(mirror.media_roots), 1)
                    self.assertEqual(mirror.apply_changes(changes), through)
                    self.assertEqual(mirror.exchange_state.head, through)
                    self.assertEqual(mirror.apply_changes(changes), through)
                    self.assertEqual(len(mirror.media_roots), 2)
                with self.assertRaises(ExchangeError) as forbidden:
                    authority.apply_changes(changes)
                self.assertEqual(
                    forbidden.exception.kind, ExchangeFailure.MIRROR_READ_ONLY
                )
                floor = authority.resynchronize_history(
                    authority.exchange_state.observed_base,
                    records=100,
                    encoded_bytes=1048576,
                )
                self.assertEqual(floor, saved)

    def test_preparation_is_terminal_only_after_success_and_never_commits(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "authority.pproj") as production:
                with production.edit() as edit:
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


def corrupt_second_manifest(encoded: bytes) -> bytes:
    """Keep the file seal intact while damaging the second logical record."""
    offset = 8
    payload = bytearray(encoded)
    while offset + 8 < len(encoded):
        length = int.from_bytes(encoded[offset : offset + 8], "big")
        start = offset + 8
        document = json.loads(encoded[start : start + length])
        if (
            document.get("kind") == "record.manifest"
            and document["revision"]["sequence"] == "2"
        ):
            payload[start] = ord("!")
            return bytes(payload)
        offset = start + length
    raise AssertionError("fixture has no second record manifest")


if __name__ == "__main__":
    unittest.main()
