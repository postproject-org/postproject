"""Detached verified choices keep their base and stage no partial publication."""

import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from uuid import uuid4

from postproject import (
    ConflictError,
    ConflictKeyKind,
    InvalidArgumentError,
    Production,
    ResourceId,
    UnsupportedError,
    VerificationMode,
    file_locator,
)
from postproject.exchange import ClientId, RequestId


class RelinkSelectionTests(unittest.TestCase):
    def test_proposal_retry_and_passive_replay_preserve_selected_locations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            file = root / "original.mov"
            file.write_bytes(b"camera original")
            with Production.create(root / "authority.pproj") as authority:
                with authority.edit() as edit:
                    imported = edit.import_media(file)
                    edit.commit()
                saved = authority.export_checkpoint(root / "checkpoint")
                moved = root / "moved"
                moved.mkdir()
                destination = file.rename(moved / file.name)
                (resolution,) = authority.resolve(
                    imported.asset_id, search_directories=[moved]
                )
                selection = resolution.select_verified()
                with authority.edit(base=selection.base) as edit:
                    edit.confirm_selection(selection)
                    proposal = edit.prepare_proposal(
                        ClientId(uuid4()), RequestId(uuid4())
                    )
                with proposal:
                    self.assertIn(b'"kind":"locator.confirm.v1"', proposal.document)
                    accepted = authority.submit_proposal(proposal)
                    again = authority.submit_proposal(proposal)
                    self.assertEqual(again.outcome.document, accepted.outcome.document)
                through = authority.export_changes(saved, root / "changes")
                destination.unlink()  # Replay uses recorded facts, never current media bytes.
                with Production.import_checkpoint(
                    root / "checkpoint", root / "mirror.pproj"
                ) as mirror:
                    self.assertEqual(mirror.apply_changes(root / "changes"), through)
                    self.assertEqual(
                        mirror.representation(imported.representation_id),
                        authority.representation(imported.representation_id),
                    )
                    with self.assertRaises(UnsupportedError):
                        mirror.edit(base=selection.base)

    def test_detached_locations_survive_the_view_and_staging_copies_them(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            old = root / "original.mov"
            old.write_bytes(b"camera original")
            with Production.create(root / "production.pproj") as production:
                with production.edit() as edit:
                    imported = edit.import_media(old)
                    edit.commit()
                moved = root / "moved"
                moved.mkdir()
                destination = old.rename(moved / old.name)
                with production.read_session() as view:
                    (resolution,) = view.resolve(
                        imported.asset_id, search_directories=[moved]
                    )
                    selection = resolution.select_verified()
                    self.assertEqual(selection.base, view.decision_base)
                self.assertEqual(
                    selection.representation_id, imported.representation_id
                )
                self.assertEqual(
                    selection.locations[0].candidate.uri, file_locator(destination)
                )
                with production.edit() as edit:
                    with self.assertRaises(ConflictError):
                        edit.confirm_selection(selection)
                    edit.rollback()
                with production.edit(base=selection.base) as edit:
                    edit.confirm_selection(selection)
                    del resolution, selection
                    receipt = edit.commit()
                self.assertIsNotNone(receipt.revision)
                resource = production.representation(
                    imported.representation_id
                ).resources[0]
                self.assertIn(
                    file_locator(destination), [item.uri for item in resource.locators]
                )

    def test_presence_unavailable_ambiguous_and_truncated_are_not_verified(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            old = root / "original.mov"
            old.write_bytes(b"camera original")
            with Production.create(root / "production.pproj") as production:
                with production.edit() as edit:
                    imported = edit.import_media(old)
                    edit.commit()
                (presence,) = production.resolve(imported.asset_id)
                with self.assertRaises(ConflictError):
                    presence.select_verified()
                (verified,) = production.resolve(
                    imported.asset_id, verification=VerificationMode.CONTENT
                )
                self.assertEqual(len(verified.select_verified().locations), 1)
                old.unlink()
                (offline,) = production.resolve(imported.asset_id)
                with self.assertRaises(ConflictError):
                    offline.select_verified()
                moved = root / "moved"
                moved.mkdir()
                (moved / "a.mov").write_bytes(b"camera original")
                (moved / "b.mov").write_bytes(b"camera original")
                (ambiguous,) = production.resolve(
                    imported.asset_id, search_directories=[moved]
                )
                with self.assertRaises(ConflictError):
                    ambiguous.select_verified()
                (truncated,) = production.resolve(
                    imported.asset_id,
                    search_directories=[moved],
                    max_entries_per_directory=1,
                )
                with self.assertRaises(ConflictError):
                    truncated.select_verified()

    def test_bad_locations_preserve_the_edit_and_stale_commits_are_terminal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            old = root / "original.mov"
            old.write_bytes(b"camera original")
            with Production.create(root / "production.pproj") as production:
                with production.edit() as edit:
                    imported = edit.import_media(old)
                    edit.commit()
                (resolution,) = production.resolve(
                    imported.asset_id, verification=VerificationMode.CONTENT
                )
                selection = resolution.select_verified()
                before = production.latest_revision
                malformed = replace(
                    selection,
                    locations=(
                        selection.locations[0],
                        replace(
                            selection.locations[0],
                            resource_id=ResourceId(uuid4()),
                            candidate=replace(
                                selection.locations[0].candidate, uri="relative.mov"
                            ),
                        ),
                    ),
                )
                with production.edit(base=selection.base) as edit:
                    with self.assertRaises(InvalidArgumentError):
                        edit.confirm_selection(malformed)
                    receipt = edit.commit()
                self.assertIsNone(receipt.revision)
                self.assertEqual(production.latest_revision, before)
                with Production.open(root / "production.pproj") as peer:
                    with peer.edit(base=selection.base) as edit:
                        edit.confirm_locator(
                            selection.locations[0].resource_id, "file:///peer.mov"
                        )
                        edit.commit()
                with production.edit(base=selection.base) as edit:
                    edit.confirm_selection(selection)
                    with self.assertRaises(ConflictError) as caught:
                        edit.commit()
                    conflict = caught.exception.conflict
                    assert conflict is not None
                    self.assertIs(conflict.key.kind, ConflictKeyKind.LOCATOR_SET)
                    with self.assertRaises(RuntimeError):
                        edit.commit()
