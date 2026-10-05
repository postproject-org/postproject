"""Closed job alternatives and defensive value construction."""

import unittest
from dataclasses import FrozenInstanceError, replace
from uuid import UUID

from postproject import (
    ActivityId,
    AssetId,
    Job,
    JobCancelled,
    JobClaim,
    JobClaimId,
    JobCompletion,
    JobFailure,
    JobId,
    JobRequested,
    JobState,
    JobStatus,
    RepresentationId,
    RepresentationKind,
    ToolIdentity,
)


class JobStatusTests(unittest.TestCase):
    def job(self, status: JobStatus) -> Job:
        return Job(
            JobId(UUID(int=1)),
            "example:work",
            (),
            AssetId(UUID(int=2)),
            RepresentationKind.PROXY,
            None,
            status,
        )

    def test_each_status_has_only_its_applicable_detail(self) -> None:
        claim = JobClaim(JobClaimId(UUID(int=3)), ToolIdentity("worker"), None, 20)
        completion = JobCompletion(
            ActivityId(UUID(int=4)), RepresentationId(UUID(int=5))
        )
        alternatives: tuple[tuple[JobStatus, JobState], ...] = (
            (JobRequested(), JobState.REQUESTED),
            (claim, JobState.CLAIMED),
            (completion, JobState.SUCCEEDED),
            (JobFailure("encoder failed"), JobState.FAILED),
            (JobCancelled(), JobState.CANCELLED),
        )
        for status, kind in alternatives:
            with self.subTest(kind=kind):
                job = self.job(status)
                self.assertIs(job.status, status)
                self.assertIs(job.state, kind)
                self.assertEqual(job.claim, claim if kind is JobState.CLAIMED else None)
                self.assertEqual(
                    job.completion, completion if kind is JobState.SUCCEEDED else None
                )
                self.assertEqual(
                    job.failure_diagnostic,
                    "encoder failed" if kind is JobState.FAILED else None,
                )
                self.assertRaises(
                    FrozenInstanceError, setattr, job, "status", JobRequested()
                )

    def test_invalid_alternative_and_diagnostic_are_rejected(self) -> None:
        job = self.job(JobRequested())
        self.assertRaises(TypeError, replace, job, status=JobState.REQUESTED)
        self.assertRaises(TypeError, replace, job, status=object())
        self.assertRaises(TypeError, JobFailure, 123)
        for diagnostic in ("", "bad\0text", "é" * 2049):
            self.assertRaises(ValueError, JobFailure, diagnostic)
        self.assertEqual(JobFailure("é" * 2048).diagnostic, "é" * 2048)

    def test_input_collection_is_copied(self) -> None:
        inputs = [RepresentationId(UUID(int=6))]
        job = replace(self.job(JobRequested()), inputs=inputs)
        inputs.clear()
        self.assertEqual(job.inputs, (RepresentationId(UUID(int=6)),))


if __name__ == "__main__":
    unittest.main()
