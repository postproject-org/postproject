"""Public job observations and private submission recovery remain separate."""

import tempfile
import unittest
from datetime import timedelta
from itertools import repeat
from pathlib import Path
from uuid import uuid4

from postproject import (
    ActivityEdge,
    ActivitySpec,
    ClaimedJobResult,
    ClientId,
    ClosedJobLease,
    ExchangeError,
    ExchangeFailure,
    FailedJobResult,
    InvalidArgumentError,
    JobCancelled,
    JobCompletion,
    JobLease,
    JobRequest,
    JobRequested,
    Production,
    RepresentationKind,
    RequestId,
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
        with self.production.transaction() as setup:
            self.asset = setup.import_media(self.media)
            setup.commit()
        self.input = self.production.representations[self.asset][0].id
        with self.production.transaction() as edit:
            self.job = edit.request_job(
                JobRequest(
                    "example:proxy", (self.input,), self.asset, RepresentationKind.PROXY
                )
            )
            proposal = edit.prepare_proposal(self.client, RequestId(uuid4()))
        with proposal:
            result = self.production.submit_proposal(proposal)
            self.assertEqual(result.outcome.jobs[0].job_id, self.job)
            self.assertIsInstance(result.outcome.jobs[0].status, JobRequested)

    def claim(self) -> JobLease:
        with self.production.transaction() as edit:
            pending = edit.claim_job_lease(self.job, self.tool, timedelta(minutes=1))
            proposal = edit.prepare_proposal(self.client, RequestId(uuid4()))
        with pending, proposal:
            self.assertIsInstance(pending.state, ClosedJobLease)
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
        with self.production.transaction() as edit:
            edit.renew_job_lease(lease, timedelta(minutes=2))
            renewal = edit.prepare_proposal(self.client, RequestId(uuid4()))
        with renewal:
            renewed = self.production.submit_proposal(renewal, leases=[lease])
            with self.production.transaction() as release:
                release.release_job_lease(lease)
                release.commit()
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
        with self.production.transaction() as edit:
            edit.fail_job_lease(lease, "exact failure 名")
            failed = edit.prepare_proposal(self.client, RequestId(uuid4()))
        with failed:
            result = self.production.submit_proposal(failed, leases=[lease])
            self.assertIsInstance(result.outcome.jobs[0].status, FailedJobResult)
            self.assertEqual(result.leases, [])
        with self.production.transaction() as edit:
            cancelled_job = edit.request_job(
                JobRequest(
                    "example:cancel",
                    (self.input,),
                    self.asset,
                    RepresentationKind.PROXY,
                )
            )
            edit.cancel_job(cancelled_job)
            cancelled = edit.prepare_proposal(self.client, RequestId(uuid4()))
        with cancelled:
            result = self.production.submit_proposal(cancelled)
            self.assertEqual(result.outcome.jobs[0].job_id, cancelled_job)
            self.assertIsInstance(result.outcome.jobs[0].status, JobCancelled)

    def test_completion_copies_original_publication_ids_after_reopen(self) -> None:
        lease = self.claim()
        with self.production.transaction() as edit:
            output = edit.add_representation(
                self.asset, RepresentationKind.PROXY, self.media
            )
            activity = edit.create_activity(
                ActivitySpec(
                    "example:publish",
                    (ActivityEdge(output),),
                    (ActivityEdge(self.input),),
                    tool=self.tool,
                )
            )
            edit.complete_job_lease(lease, output, activity)
            completed = edit.prepare_proposal(self.client, RequestId(uuid4()))
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
