"""Run the Python job request, worker, and regeneration listings.

Each ``[name]`` ... ``[/name]`` region is included verbatim by the
documentation build, so keep regions self-contained and readable. Usage::

    POSTPROJECT_LIBRARY=/path/to/libpostproject.so python jobs.py WORK_DIRECTORY

The work directory is prepared by ``prepare-workdir.cmake``.
"""

from __future__ import annotations

import sys
from datetime import timedelta
from pathlib import Path

from postproject import (
    ActiveJobLease,
    ActivityEdge,
    ActivityRef,
    ActivitySpec,
    AgentIdentity,
    AssetId,
    ClosedJobLease,
    ExternalIdentifier,
    Job,
    JobId,
    JobRef,
    JobRequest,
    JobState,
    MetadataAssertion,
    MetadataProperty,
    MetadataString,
    PendingJobLease,
    Production,
    RegenerationJobPlan,
    RepresentationId,
    RepresentationKind,
    ToolIdentity,
)

WORKER = ToolIdentity("Example Proxy Worker", "1.4", "https://example.com/worker")
AGENT = AgentIdentity("render-node-4", ExternalIdentifier("com.example.host", "node-4"))


def job_by_id(production: Production, job_id: JobId) -> Job:
    return next(job for job in production.jobs(limit=100).items if job.id == job_id)


# [request-job]
PROFILE = MetadataProperty(
    "https://postproject.org/ns/executor-parameters/1", "profile"
)


def request_proxy(
    production: Production, input_id: RepresentationId, asset_id: AssetId
) -> JobId:
    request = JobRequest(
        "org.postproject:generate-proxy",
        (input_id,),
        asset_id,
        RepresentationKind.PROXY,
        target_root="proxies",
    )
    # The job and its typed parameters commit together.
    with production.transaction() as transaction:
        job_id = transaction.request_job(request)
        transaction.add_metadata(JobRef(job_id), PROFILE, MetadataString("proxy-720p"))
        transaction.commit()

    page = production.jobs(
        limit=100, state=JobState.REQUESTED, kind="org.postproject:generate-proxy"
    )
    job = next(job for job in page.items if job.id == job_id)
    inputs = ", ".join(str(item) for item in job.inputs)
    print(
        f"{job.kind} {job.state.name}: {inputs} -> {job.output_representation_kind.name}"
    )
    for parameter in production.metadata[JobRef(job_id)]:
        print(f"  {parameter.property.property} = {parameter.value}")
    return job_id


# [/request-job]


# [claim-job]
def claim_renew_release(production: Production, job_id: JobId) -> None:
    with production.transaction() as transaction:
        lease = transaction.claim_job_lease(
            job_id, WORKER, timedelta(minutes=5), agent=AGENT
        )
        assert isinstance(lease.state, PendingJobLease)
        transaction.commit()

    with lease:
        assert lease.production_id == production.id and lease.job_id == job_id
        assert isinstance(lease.state, ActiveJobLease)
        # Explicit private transport when work moves to another process.
        with production.import_job_lease(lease.export_token()) as imported:
            with production.transaction() as transaction:
                transaction.renew_job_lease(imported, timedelta(minutes=10))
                transaction.commit()
            with production.transaction() as transaction:
                transaction.release_job_lease(imported)
                transaction.commit()
            assert isinstance(imported.state, ClosedJobLease)


# [/claim-job]


# [complete-job]
def run_proxy_job(production: Production, job_id: JobId, output: Path) -> None:
    with production.transaction() as transaction:
        lease = transaction.claim_job_lease(
            job_id, WORKER, timedelta(minutes=5), agent=AGENT
        )
        transaction.commit()
    with lease:
        job = job_by_id(production, job_id)

        output.write_bytes(b"720p proxy essence\n")  # the actual work

        # Stage the output, the activity, and the completion in one transaction;
        # never commit the representation or activity separately.
        with production.transaction() as transaction:
            proxy_id = transaction.add_representation(
                job.output_asset_id, job.output_representation_kind, output
            )
            activity_id = transaction.create_activity(
                ActivitySpec(
                    job.kind,
                    inputs=tuple(ActivityEdge(item) for item in job.inputs),
                    outputs=(ActivityEdge(proxy_id),),
                    tool=WORKER,
                    agent=AGENT,
                )
            )
            # Copy the job parameters so the activity can be reproduced.
            for parameter in production.metadata[JobRef(job_id)]:
                transaction.add_metadata(
                    ActivityRef(activity_id), parameter.property, parameter.value
                )
            transaction.complete_job_lease(lease, proxy_id, activity_id)
            transaction.commit()


# [/complete-job]


# [fail-job]
def fail_after_tool_error(production: Production, job_id: JobId) -> None:
    with production.transaction() as transaction:
        lease = transaction.claim_job_lease(
            job_id, WORKER, timedelta(minutes=5), agent=AGENT
        )
        transaction.commit()
    # A failure records a bounded diagnostic and no representation.
    with lease, production.transaction() as transaction:
        transaction.fail_job_lease(lease, "encoder exited with 1")
        transaction.commit()


# [/fail-job]


# [cancel-job]
def cancel(production: Production, job_id: JobId) -> None:
    with production.transaction() as transaction:
        transaction.cancel_job(job_id)
        transaction.commit()


# [/cancel-job]


# [plan-regeneration]
def plan_and_enqueue(
    production: Production, artifact_id: RepresentationId
) -> tuple[RegenerationJobPlan, JobId]:
    # Planning is read-only: it proposes a job but never enqueues one.
    (plan,) = production.plan_regeneration([artifact_id])
    proposed = plan.job
    print(f"regenerate {artifact_id} with {proposed.kind}")

    # Enqueue explicitly. The proposal repeats the kind and target root of the
    # job that produced the artifact.
    with production.transaction() as transaction:
        job_id = transaction.request_job(
            JobRequest(
                proposed.kind,
                proposed.inputs,
                proposed.output_asset_id,
                proposed.output_representation_kind,
                proposed.target_root,
            )
        )
        # The planned parameters target the proposal; retarget them.
        for parameter in plan.parameters:
            transaction.add_metadata(
                JobRef(job_id), parameter.property, parameter.value
            )
        transaction.commit()
    return plan, job_id


# [/plan-regeneration]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: jobs.py WORK_DIRECTORY")
    work = Path(sys.argv[1])
    output = work / "proxies" / "A001_proxy.mov"
    output.parent.mkdir()

    with Production.create(work / "jobs.pproj", "Jobs") as production:
        with production.transaction() as transaction:
            asset_id = transaction.import_media(work / "rushes" / "A001.mov")
            transaction.add_media_root("proxies", "Proxy volume")
            transaction.commit()
        source_id = production.representations[asset_id][0].id
        parameter = MetadataString("proxy-720p")

        job_id = request_proxy(production, source_id, asset_id)
        job = job_by_id(production, job_id)
        assert job.state is JobState.REQUESTED
        assert job.inputs == (source_id,)
        assert job.output_asset_id == asset_id
        assert job.output_representation_kind is RepresentationKind.PROXY
        assert job.target_root == "proxies"
        assert production.metadata[JobRef(job_id)] == (
            MetadataAssertion(JobRef(job_id), PROFILE, parameter),
        )

        claim_renew_release(production, job_id)
        job = job_by_id(production, job_id)
        assert job.state is JobState.REQUESTED and job.claim is None

        run_proxy_job(production, job_id, output)
        job = job_by_id(production, job_id)
        assert job.state is JobState.SUCCEEDED and job.completion is not None
        (producer,) = production.activities_producing[job.completion.representation_id]
        assert producer.id == job.completion.activity_id
        assert producer.tool == WORKER and producer.agent == AGENT
        assert production.metadata[ActivityRef(producer.id)] == (
            MetadataAssertion(ActivityRef(producer.id), PROFILE, parameter),
        )
        proxy_id = job.completion.representation_id

        representations_before = len(production.representations[asset_id])
        failing_id = request_proxy(production, source_id, asset_id)
        fail_after_tool_error(production, failing_id)
        failed = job_by_id(production, failing_id)
        assert failed.state is JobState.FAILED
        assert failed.failure_diagnostic == "encoder exited with 1"
        assert len(production.representations[asset_id]) == representations_before

        cancelled_id = request_proxy(production, source_id, asset_id)
        cancel(production, cancelled_id)
        assert job_by_id(production, cancelled_id).state is JobState.CANCELLED

        revision = production.latest_revision
        plan, enqueued_id = plan_and_enqueue(production, proxy_id)
        assert plan.artifact_representation_id == proxy_id
        assert plan.job.state is JobState.REQUESTED
        assert plan.job.inputs == (source_id,)
        assert plan.job.target_root == "proxies"
        assert plan.parameters == (
            MetadataAssertion(JobRef(plan.job.id), PROFILE, parameter),
        )
        latest = production.latest_revision
        assert revision is not None and latest is not None
        assert latest.sequence == revision.sequence + 1
        enqueued = job_by_id(production, enqueued_id)
        assert enqueued.state is JobState.REQUESTED
        assert enqueued.target_root == "proxies"
        assert production.metadata[JobRef(enqueued_id)] == (
            MetadataAssertion(JobRef(enqueued_id), PROFILE, parameter),
        )
        requested = production.jobs(limit=100, state=JobState.REQUESTED).items
        assert [item.id for item in requested] == [enqueued_id]


if __name__ == "__main__":
    main()
