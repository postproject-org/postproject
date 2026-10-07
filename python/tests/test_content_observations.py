"""Observation guards, file-fact receipts and retained native knowledge."""

import os
import tempfile
import unittest
from pathlib import Path

from postproject import (
    ConflictError,
    ConflictKeyKind,
    ContentObservationOutcome,
    Fingerprint,
    InvalidArgumentError,
    Production,
    ResourceFileFactsObservedEvent,
)


class ContentObservationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "production.pproj"
        self.media = Path(self.directory.name) / "clip.dat"
        self.media.write_bytes(b"original content")
        os.utime(self.media, (1, 1))
        self.production = Production.create(self.path)
        self.addCleanup(self.production.close)
        with self.production.transaction() as edit:
            self.asset = edit.import_media(self.media)
            edit.commit()
        self.representation = self.production.representations[self.asset][0]
        self.resource = self.representation.resources[0].id

    def test_missing_base_rejects_before_file_access_and_leaves_edit_open(self) -> None:
        head = self.production.latest_revision
        with self.production.transaction() as edit:
            with self.assertRaises(InvalidArgumentError):
                edit.observe_resource_content(
                    self.resource, self.media.with_name("absent")
                )
            with self.assertRaises(InvalidArgumentError):
                edit.record_resource_fingerprint(
                    self.resource, Fingerprint("host", 1, b"r")
                )
            with self.assertRaises(InvalidArgumentError):
                edit.record_representation_fingerprint(
                    self.representation.id, Fingerprint("host", 1, b"a")
                )
            self.assertIsNone(edit.commit().revision)
        self.assertEqual(self.production.latest_revision, head)

    def test_changed_file_facts_have_their_own_event_and_filtered_revision(
        self,
    ) -> None:
        head = self.production.latest_revision
        assert head is not None
        os.utime(self.media, (2, 2))
        with self.production.read_session() as view, view.edit() as edit:
            self.assertIs(
                edit.observe_resource_content(self.resource, self.media),
                ContentObservationOutcome.UNCHANGED,
            )
            receipt = edit.commit()
        revision = receipt.revision
        assert revision is not None
        events = self.production.revision_events[revision.id]
        self.assertEqual(len(events), 1)
        self.assertEqual(
            events[0].payload, ResourceFileFactsObservedEvent(self.resource)
        )
        page = self.production.changes_since_filtered(
            head.sequence, (ResourceFileFactsObservedEvent,), limit=1
        )
        self.assertEqual([item.id for item in page.revisions], [revision.id])
        resource = self.production.representations[self.asset][0].resources[0]
        self.assertEqual(resource.modified_at_unix_micros, 2_000_000)
        self.assertEqual(
            resource.fingerprints, self.representation.resources[0].fingerprints
        )
        with self.production.read_session() as view, view.edit() as edit:
            edit.observe_resource_content(self.resource, self.media)
            self.assertIsNone(edit.commit().revision)

    def test_released_view_edit_keeps_old_knowledge_and_rejects_stale_noop(
        self,
    ) -> None:
        view = self.production.read_session()
        self.addCleanup(view.close)
        base = view.decision_base
        self.media.write_bytes(b"new content")
        with Production.open(self.path) as writer:
            with writer.read_session() as fresh, fresh.edit() as winner:
                winner.observe_resource_content(self.resource, self.media)
                winner.commit()
        head = self.production.latest_revision
        with view.edit() as edit:
            view.close()
            self.assertIs(
                edit.observe_resource_content(self.resource, self.media),
                ContentObservationOutcome.CHANGED,
            )
            with self.assertRaises(ConflictError) as caught:
                edit.commit()
            detail = caught.exception.conflict
            assert detail is not None
            self.assertIs(detail.key.kind, ConflictKeyKind.RESOURCE_FINGERPRINT)
            with self.assertRaises(RuntimeError):
                edit.commit()
        self.assertEqual(self.production.latest_revision, head)
        with self.production.edit(base) as detached:
            with self.assertRaises(ConflictError):
                detached.observe_resource_content(self.resource, self.media)
            self.assertIsNone(detached.commit().revision)
