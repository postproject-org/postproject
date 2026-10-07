"""Native ownership, precise durations and atomic publication through leases."""

import tempfile
import unittest
from datetime import timedelta
from pathlib import Path
from typing import cast

from postproject import (
    ActiveJobLease,
    ActivityEdge,
    ActivitySpec,
    ClosedJobLease,
    ConflictError,
    ConflictKeyKind,
    Fingerprint,
    InvalidArgumentError,
    JobLease,
    JobRequest,
    JobState,
    MetadataProperty,
    MetadataString,
    PendingJobLease,
    Production,
    RepresentationKind,
    RepresentationRef,
    ToolIdentity,
)


class JobLeaseTests(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.media = self.root / "media.mov"
        self.media.write_bytes(b"media")
        self.production = Production.create(self.root / "production.pproj")
        self.addCleanup(self.production.close)
        with self.production.transaction() as edit:
            self.asset = edit.import_media(self.media)
            self.job = edit.request_job(
                JobRequest(
                    "example:publish", (), self.asset, RepresentationKind.DERIVED
                )
            )
            edit.commit()
        self.tool = ToolIdentity("worker")

    def claim(self) -> JobLease:
        with self.production.transaction() as edit:
            lease = edit.claim_job_lease(self.job, self.tool, timedelta(minutes=1))
            self.addCleanup(lease.close)
            self.assertIsInstance(lease.state, PendingJobLease)
            with self.assertRaises(InvalidArgumentError):
                lease.export_token()
            edit.commit()
        self.assertIsInstance(lease.state, ActiveJobLease)
        self.assertEqual(lease.job_id, self.job)
        self.assertEqual(lease.production_id, self.production.id)
        return lease

    def test_release_closes_ownership_and_fences_imported_worker(self) -> None:
        lease = self.claim()
        token = lease.export_token()
        imported = self.production.import_job_lease(token)
        self.addCleanup(imported.close)
        with self.production.transaction() as edit:
            edit.renew_job_lease(lease, timedelta(minutes=2))
            edit.commit()
        active = lease.state
        assert isinstance(active, ActiveJobLease)
        claim = self.production.job(self.job).claim
        assert claim is not None
        self.assertEqual(
            active.expires_at_unix_micros,
            claim.expires_at_unix_micros,
        )
        with self.production.transaction() as edit:
            edit.release_job_lease(lease)
            edit.commit()
        self.assertIsInstance(lease.state, ClosedJobLease)
        with self.assertRaises(ConflictError):
            self.production.import_job_lease(token)
        with self.production.transaction() as edit:
            edit.fail_job_lease(imported, "late failure")
            with self.assertRaises(ConflictError):
                edit.commit()
        self.assertEqual(self.production.job(self.job).state, JobState.REQUESTED)
        lease.close()
        with self.assertRaises(RuntimeError):
            _ = lease.state

    def test_pending_claim_and_metadata_publication_commit_together(self) -> None:
        with self.production.transaction() as edit:
            lease = edit.claim_job_lease(self.job, self.tool, timedelta(minutes=1))
            self.addCleanup(lease.close)
            output = edit.add_representation(
                self.asset, RepresentationKind.DERIVED, self.media
            )
            target = RepresentationRef(output)
            property = MetadataProperty("com.example", "title")
            edit.add_metadata(target, property, MetadataString("Published"))
            activity = edit.create_activity(
                ActivitySpec("example:publish", (ActivityEdge(output),), tool=self.tool)
            )
            edit.complete_job_lease(lease, output, activity)
            self.assertIsNotNone(edit.commit().revision)
        self.assertIsInstance(lease.state, ClosedJobLease)
        self.assertEqual(self.production.job(self.job).state, JobState.SUCCEEDED)
        self.assertEqual(
            self.production.metadata[target][0].value, MetadataString("Published")
        )

    def test_duration_validation_and_discard_leave_requested_job(self) -> None:
        with self.production.transaction() as edit:
            for invalid in [timedelta(0), timedelta(microseconds=-1), timedelta.max]:
                with self.assertRaises(ValueError):
                    edit.claim_job_lease(self.job, self.tool, invalid)
            for invalid_type in [True, 1, 1.0, "1s"]:
                with self.assertRaises(TypeError):
                    edit.claim_job_lease(
                        self.job, self.tool, cast(timedelta, invalid_type)
                    )
            lease = edit.claim_job_lease(self.job, self.tool, timedelta(microseconds=1))
            self.addCleanup(lease.close)
        self.assertIsInstance(lease.state, ClosedJobLease)
        self.assertEqual(self.production.job(self.job).state, JobState.REQUESTED)

    def test_pending_completion_rejects_misordered_output_without_mutating_edit(
        self,
    ) -> None:
        with self.production.transaction() as edit:
            output = edit.add_representation(
                self.asset, RepresentationKind.DERIVED, self.media
            )
            lease = edit.claim_job_lease(self.job, self.tool, timedelta(minutes=1))
            self.addCleanup(lease.close)
            activity = edit.create_activity(
                ActivitySpec("example:publish", (ActivityEdge(output),))
            )
            with self.assertRaises(InvalidArgumentError):
                edit.complete_job_lease(lease, output, activity)
        self.assertIsInstance(lease.state, ClosedJobLease)
        self.assertEqual(len(self.production.representations[self.asset]), 1)
        self.assertEqual(self.production.job(self.job).state, JobState.REQUESTED)

    def test_wrong_production_and_malformed_tokens_reject_before_mutation(self) -> None:
        lease = self.claim()
        with Production.create(self.root / "other.pproj") as other:
            with self.assertRaises(InvalidArgumentError):
                other.import_job_lease(lease.export_token())
            with other.transaction() as edit:
                with self.assertRaises(InvalidArgumentError):
                    edit.release_job_lease(lease)
                self.assertIsNone(edit.commit().revision)
        for token in ["", "x" * 10_000, "x" * 115]:
            with self.assertRaises((ValueError, InvalidArgumentError)):
                self.production.import_job_lease(token)

    def test_changed_input_rolls_back_publication_without_closing_lease(self) -> None:
        source = self.production.representations[self.asset][0]
        with self.production.transaction() as edit:
            job = edit.request_job(
                JobRequest(
                    "example:publish",
                    (source.id,),
                    self.asset,
                    RepresentationKind.DERIVED,
                )
            )
            edit.commit()
        with self.production.read_session() as view, view.edit() as edit:
            lease = edit.claim_job_lease(job, self.tool, timedelta(minutes=1))
            self.addCleanup(lease.close)
            edit.commit()
        token = lease.export_token()
        lease.close()
        imported = self.production.import_job_lease(token)
        self.addCleanup(imported.close)
        with self.production.read_session() as view, view.edit() as edit:
            edit.record_resource_fingerprint(
                source.resources[0].id, Fingerprint("example-content", 1, b"changed")
            )
            changed = edit.commit()
        with self.production.transaction() as edit:
            output = edit.add_representation(
                self.asset, RepresentationKind.DERIVED, self.media
            )
            activity = edit.create_activity(
                ActivitySpec(
                    "example:publish",
                    (ActivityEdge(output),),
                    inputs=(ActivityEdge(source.id),),
                )
            )
            edit.complete_job_lease(imported, output, activity)
            with self.assertRaises(ConflictError) as caught:
                edit.commit()
        conflict = caught.exception.conflict
        assert conflict is not None and changed.revision is not None
        self.assertEqual(conflict.key.kind, ConflictKeyKind.RESOURCE_FINGERPRINT)
        self.assertEqual(conflict.superseding_revision_id, changed.revision.id)
        self.assertEqual(len(self.production.representations[self.asset]), 1)
        self.assertEqual(self.production.job(job).state, JobState.CLAIMED)
        self.assertIsInstance(imported.state, ActiveJobLease)
        with self.production.transaction() as edit:
            edit.release_job_lease(imported)
            edit.commit()
        self.assertIsInstance(imported.state, ClosedJobLease)
