"""Coherent views, detached scope validation, and explicit edit disposal."""

import tempfile
import unittest
from pathlib import Path

from postproject import (
    AssetRef,
    CommittedRevision,
    ConflictError,
    DecisionBase,
    ExternalIdentifier,
    InvalidArgumentError,
    LocatorIdentity,
    Production,
    file_locator,
    fingerprint_file,
)


class ReadSessionTests(unittest.TestCase):
    def test_roots_identifiers_and_known_media_use_the_same_pinned_view(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            media = Path(directory) / "camera.mov"
            media.write_bytes(b"coherent lookup fixture")
            with Production.create(Path(directory) / "lookup.pproj") as production:
                with production.transaction() as transaction:
                    asset = transaction.import_media(media)
                target = AssetRef(asset)
                with production.read_session() as view:
                    locator = LocatorIdentity(file_locator(media))
                    fingerprint = fingerprint_file(media)
                    matches = view.find_known_media_by_locator(locator, limit=1)
                    self.assertEqual(matches.items[0].asset_id, asset)
                    with view.edit() as edit:
                        edit.add_media_root("rushes")
                        edit.add_external_identifier(
                            target,
                            ExternalIdentifier("https://example.com/id", "camera"),
                        )
                        edit.commit()
                    self.assertEqual(view.media_roots, ())
                    self.assertEqual(view.external_identifiers(target), ())
                    self.assertEqual(
                        view.find_by_external_identifier(
                            "https://example.com/id", "camera", None
                        ),
                        (),
                    )
                    self.assertEqual(
                        view.find_known_media_by_fingerprint(
                            fingerprint, limit=1
                        ).items,
                        matches.items,
                    )
                    self.assertEqual(len(production.media_roots), 1)
                    self.assertEqual(
                        production.objects_by_external_identifier[
                            "https://example.com/id", "camera"
                        ],
                        (target,),
                    )
                self.assertEqual(matches.items[0].asset_id, asset)
                for operation in (
                    lambda: view.media_roots,
                    lambda: view.external_identifiers(target),
                    lambda: view.find_by_external_identifier(
                        "https://example.com/id", "camera", None
                    ),
                    lambda: view.find_known_media_by_locator(locator, limit=1),
                    lambda: view.find_known_media_by_fingerprint(fingerprint, limit=1),
                ):
                    with self.assertRaises(RuntimeError):
                        operation()

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
