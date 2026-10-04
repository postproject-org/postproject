"""Coherent views, detached scope validation, and explicit edit disposal."""

import tempfile
import unittest
from pathlib import Path

from postproject import (
    CommittedRevision,
    ConflictError,
    DecisionBase,
    InvalidArgumentError,
    Production,
)


class ReadSessionTests(unittest.TestCase):
    def test_empty_view_remains_pinned_and_stale_edit_conflicts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "production.pproj") as production:
                with production.read_session() as empty:
                    base = empty.decision_base
                    self.assertIsNone(base.revision)
                    with empty.edit() as edit:
                        root = edit.add_media_root("rushes")
                        receipt = edit.commit()
                    self.assertEqual(empty.decision_base, base)
                    self.assertEqual(empty.assets_page(limit=1).items, ())
                    with production.edit(base) as stale:
                        stale.set_media_root_enabled(root, False)
                        with self.assertRaises(ConflictError) as failure:
                            stale.commit()
                        assert failure.exception.conflict is not None
                        self.assertIsNone(failure.exception.conflict.base_revision_id)
                with self.assertRaises(RuntimeError):
                    empty.assets_page(limit=1)
                empty.close()
                with production.read_session() as refreshed:
                    self.assertEqual(refreshed.decision_base.revision, receipt.revision)

    def test_detached_bases_validate_scope_and_integer_bounds(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with (
                Production.create(Path(directory) / "first.pproj") as first,
                Production.create(Path(directory) / "second.pproj") as second,
            ):
                with first.read_session() as view:
                    base = view.decision_base
                with self.assertRaises(InvalidArgumentError):
                    second.edit(base)
                with first.edit(base) as discarded:
                    discarded.add_media_root("discarded")
                self.assertEqual(first.media_roots, ())
                with first.edit(base) as edit:
                    edit.add_media_root("saved")
                    receipt = edit.commit()
                assert receipt.revision is not None
                for sequence in (0, -1, 2**64, True):
                    invalid = DecisionBase(
                        first.id, CommittedRevision(receipt.revision.id, sequence)
                    )
                    with self.assertRaises((InvalidArgumentError, ValueError)):
                        first.edit(invalid)
                with first.edit(base) as additive:
                    additive.add_media_root("independent")
                    self.assertIsNotNone(additive.commit().revision)
