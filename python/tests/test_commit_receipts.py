"""Atomic attribution and terminal failures through the public binding."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from uuid import UUID

from postproject import AlreadyExistsError, Production, ResourceId


class CommitReceiptTests(unittest.TestCase):
    def test_receipt_identifies_own_revision_after_another_writer(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "production.pproj"
            with Production.create(path) as first, Production.open(path) as second:
                with first.transaction() as a:
                    a.add_media_root("rushes")
                    receipt = a.commit()
                with second.transaction() as b:
                    b.add_media_root("renders")
                    other = b.commit()
                self.assertEqual(receipt.production_id, first.id)
                assert receipt.revision is not None and other.revision is not None
                self.assertEqual(receipt.revision.sequence, 1)
                self.assertEqual(other.revision.sequence, 2)
                self.assertNotEqual(receipt.revision.id, other.revision.id)
                with first.transaction() as noop:
                    empty = noop.commit()
                self.assertIsNone(empty.revision)
                head = first.latest_revision
                assert head is not None
                self.assertEqual(head.id, other.revision.id)

    def test_failed_commit_context_does_not_attempt_a_second_commit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "failure.pproj") as production:
                with production.transaction() as edit:
                    edit.confirm_locator(ResourceId(UUID(int=1)), "file:///absent")
                    with self.assertRaises(AlreadyExistsError):
                        edit.commit()
                    # The original operation failure is preserved; exiting this
                    # context must not raise a second closed-transaction error.
                self.assertIsNone(production.latest_revision)
                with production.transaction() as next_edit:
                    next_edit.add_media_root("usable")
                    self.assertIsNotNone(next_edit.commit().revision)
