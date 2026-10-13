"""Public job observations and private submission recovery remain separate."""

import tempfile
import unittest
from datetime import timedelta
from itertools import repeat
from pathlib import Path
from uuid import uuid4

from postproject import (
    ActivitySpec,
    AddRepresentationCommand,
    CancelJobCommand,
    ClaimedJobResult,
    ClaimJobCommand,
    ClientId,
    ClosedJobLease,
    CompleteJobCommand,
    CreateActivityCommand,
    ExchangeError,
    ExchangeFailure,
    FailedJobResult,
    FailJobCommand,
    InvalidArgumentError,
    JobCancelled,
    JobCompletion,
    JobLease,
    JobRequest,
    JobRequested,
    PreparedActivity,
    PreparedJob,
    PreparedRepresentation,
    Production,
    ReleaseJobCommand,
    RenewJobCommand,
    RepresentationKind,
    RequestId,
    RequestJobCommand,
    ToolIdentity,
)


class ExchangeJobTests(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.work = Path(directory.name)
        self.media = self.work / "media.dat"
        self.media.write_bytes(b"independent exact media fixture")
        self.path = self.work / "authority.pproj"
        self.production = Production.create(self.path)
        self.addCleanup(self.production.close)
        self.client = ClientId(uuid4())
        self.tool = ToolIdentity("Worker")
        with self.production.edit() as setup:
            self.asset = setup.import_media(self.media).asset_id
            setup.commit()
        self.input = self.production.representations[self.asset][0].id
        with self.production.proposal_builder(
            self.client, RequestId(uuid4())
        ) as builder:
            requested = builder.stage(
                [
                    RequestJobCommand(
                        JobRequest(
                            "example:proxy",
                            (self.input,),
                            self.asset,
                            RepresentationKind.PROXY,
                        )
                    )
                ]
            )[0]
            assert isinstance(requested, PreparedJob)
            self.job = requested.job_id
            proposal = builder.prepare()
        with proposal:
            result = self.production.submit_proposal(proposal)
            self.assertEqual(result.outcome.jobs[0].job_id, self.job)
            self.assertIsInstance(result.outcome.jobs[0].status, JobRequested)

    def claim(self) -> JobLease:
        with self.production.proposal_builder(
            self.client, RequestId(uuid4())
        ) as builder:
            builder.stage([ClaimJobCommand(self.job, self.tool, timedelta(minutes=1))])
            proposal = builder.prepare()
        with proposal:
            result = self.production.submit_proposal(proposal)
            duplicate = self.production.submit_proposal(proposal)
            self.assertEqual(duplicate.leases, [])
            self.assertIsInstance(result.outcome.jobs[0].status, ClaimedJobResult)
            self.assertEqual(len(result.leases), 1)
            lease = result.leases[0]
            self.addCleanup(lease.close)
            return lease

    def test_recovery_after_release_checks_original_private_binding(self) -> None:
        lease = self.claim()
        token = lease.export_token()
        with self.production.proposal_builder(
            self.client, RequestId(uuid4())
        ) as builder:
            builder.stage([RenewJobCommand(lease, timedelta(minutes=2))])
            renewal = builder.prepare()
        with renewal:
            renewed = self.production.submit_proposal(renewal, leases=[lease])
            with self.production.proposal_builder(
                self.client, RequestId(uuid4())
            ) as builder:
                builder.stage([ReleaseJobCommand(lease)])
                release = builder.prepare()
            with release:
                released = self.production.submit_proposal(release, leases=[lease])
                self.assertIsInstance(released.outcome.jobs[0].status, JobRequested)
            self.assertIsInstance(lease.state, ClosedJobLease)
            with Production.open(self.path) as reopened:
                by_handle = reopened.submit_proposal(renewal, leases=[lease])
                by_token = reopened.submit_proposal(renewal, private_tokens=[token])
                self.assertEqual(by_handle.leases, [])
                self.assertEqual(by_token.leases, [])
                self.assertEqual(by_handle.outcome.document, renewed.outcome.document)
                self.assertEqual(by_token.outcome.document, renewed.outcome.document)
                recovered = reopened.submission_outcome(renewal.identity)
                self.assertEqual(recovered.document, renewed.outcome.document)
                with self.assertRaises(ExchangeError) as missing:
                    reopened.submit_proposal(renewal)
                self.assertEqual(
                    missing.exception.kind, ExchangeFailure.REQUEST_IDENTITY_MISMATCH
                )
                with self.assertRaises(InvalidArgumentError):
                    reopened.submit_proposal(renewal, leases=[lease, lease])
                with self.assertRaises(ExchangeError) as bounded:
                    reopened.submit_proposal(renewal, private_tokens=repeat(token))
                self.assertEqual(bounded.exception.kind, ExchangeFailure.LIMIT_EXCEEDED)
            secret = token.rsplit(":", 1)[1].encode("ascii")
            self.assertFalse(secret in renewal.document or secret in recovered.document)

    def test_public_terminal_job_alternatives_do_not_grant_ownership(self) -> None:
        lease = self.claim()
        with self.production.proposal_builder(
            self.client, RequestId(uuid4())
        ) as builder:
            builder.stage([FailJobCommand(lease, "exact failure 名")])
            failed = builder.prepare()
        with failed:
            result = self.production.submit_proposal(failed, leases=[lease])
            self.assertIsInstance(result.outcome.jobs[0].status, FailedJobResult)
            self.assertEqual(result.leases, [])
        with self.production.proposal_builder(
            self.client, RequestId(uuid4())
        ) as builder:
            requested = builder.stage(
                [
                    RequestJobCommand(
                        JobRequest(
                            "example:cancel",
                            (self.input,),
                            self.asset,
                            RepresentationKind.PROXY,
                        )
                    )
                ]
            )[0]
            assert isinstance(requested, PreparedJob)
            cancelled_job = requested.job_id
            builder.stage([CancelJobCommand(cancelled_job)])
            cancelled = builder.prepare()
        with cancelled:
            result = self.production.submit_proposal(cancelled)
            self.assertEqual(result.outcome.jobs[0].job_id, cancelled_job)
            self.assertIsInstance(result.outcome.jobs[0].status, JobCancelled)

    def test_completion_copies_original_publication_ids_after_reopen(self) -> None:
        lease = self.claim()
        with self.production.proposal_builder(
            self.client, RequestId(uuid4())
        ) as builder:
            representation = builder.stage(
                [
                    AddRepresentationCommand(
                        self.asset, RepresentationKind.PROXY, self.media
                    )
                ]
            )[0]
            assert isinstance(representation, PreparedRepresentation)
            output = representation.representation_id
            created = builder.stage(
                [
                    CreateActivityCommand(
                        ActivitySpec(
                            "example:publish",
                            (output,),
                            (self.input,),
                            tool=self.tool,
                        )
                    )
                ]
            )[0]
            assert isinstance(created, PreparedActivity)
            activity = created.activity_id
            builder.stage([CompleteJobCommand(lease, output, activity)])
            completed = builder.prepare()
        with completed:
            result = self.production.submit_proposal(completed, leases=[lease])
            self.assertEqual(
                result.outcome.jobs[0].status, JobCompletion(activity, output)
            )
            self.assertEqual(result.leases, [])
            with Production.open(self.path) as reopened:
                recovered = reopened.submission_outcome(completed.identity)
                self.assertEqual(recovered.jobs, result.outcome.jobs)
                self.assertEqual(reopened.representation(output).asset_id, self.asset)


if __name__ == "__main__":
    unittest.main()
