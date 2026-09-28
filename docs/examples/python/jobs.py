"""Run the Python job request, worker, and regeneration listings.

Each ``[name]`` ... ``[/name]`` region is included verbatim by the
documentation build, so keep regions self-contained and readable. Usage::

    POSTPROJECT_LIBRARY=/path/to/libpostproject.so python jobs.py WORK_DIRECTORY

The work directory is prepared by ``prepare-workdir.cmake``.
"""

from __future__ import annotations

import sys
from pathlib import Path

from postproject import (
    ActivityEdge,
    ActivitySpec,
    AgentIdentity,
    AssetId,
    ExternalIdentifier,
    Job,
    JobClaimId,
    JobId,
    JobRequest,
    JobState,
    MetadataAssertion,
    MetadataProperty,
    MetadataString,
    Production,
    RegenerationJobPlan,
    RepresentationId,
    RepresentationKind,
    ToolIdentity,
)

# Every job transition takes a caller-supplied time in Unix microseconds.
T0 = 1_750_000_000_000_000
MINUTE = 60_000_000
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
        transaction.add_metadata(job_id, PROFILE, MetadataString("proxy-720p"))

    page = production.jobs(
        limit=100, state=JobState.REQUESTED, kind="org.postproject:generate-proxy"
    )
    job = next(job for job in page.items if job.id == job_id)
    inputs = ", ".join(str(item) for item in job.inputs)
    print(
        f"{job.kind} {job.state.name}: {inputs} -> {job.output_representation_kind.name}"
    )
    for parameter in production.metadata[job_id]:
        print(f"  {parameter.property.property} = {parameter.value}")
    return job_id


# [/request-job]


# [claim-job]
def claim_renew_release(production: Production, job_id: JobId) -> None:
    with production.transaction() as transaction:
        # Keep the claim token private: every later transition requires it.
        claim_id = transaction.claim_job(
            job_id, WORKER, AGENT, T0, expires_at_unix_micros=T0 + 5 * MINUTE
        )

    with production.transaction() as transaction:
        transaction.renew_job_claim(job_id, claim_id, T0 + 4 * MINUTE, T0 + 9 * MINUTE)

    # Abandon the work without recording a failure: the job is requested again.
    with production.transaction() as transaction:
        transaction.release_job_claim(job_id, claim_id)


# [/claim-job]


# [complete-job]
def run_proxy_job(production: Production, job_id: JobId, output: Path) -> None:
    now = T0 + 10 * MINUTE
    with production.transaction() as transaction:
        claim_id = transaction.claim_job(job_id, WORKER, AGENT, now, now + 5 * MINUTE)
    job = job_by_id(production, job_id)

    output.write_bytes(b"720p proxy essence\n")  # the actual work

    # Stage the output, the activity, and the completion in one transaction;
    # never commit the representation or activity separately.
    with production.transaction() as transaction:
        proxy_id = transaction.add_single_file_representation(
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
        for parameter in production.metadata[job_id]:
            transaction.add_metadata(activity_id, parameter.property, parameter.value)
        transaction.complete_job(job_id, claim_id, now + MINUTE, proxy_id, activity_id)


# [/complete-job]


# [fail-job]
def fail_after_tool_error(production: Production, job_id: JobId) -> JobClaimId:
    now = T0 + 20 * MINUTE
    with production.transaction() as transaction:
        claim_id = transaction.claim_job(job_id, WORKER, AGENT, now, now + 5 * MINUTE)

    # A failure records a bounded diagnostic and no representation.
    with production.transaction() as transaction:
        transaction.fail_job(job_id, claim_id, now + MINUTE, "encoder exited with 1")
    return claim_id


# [/fail-job]


# [cancel-job]
def cancel(production: Production, job_id: JobId) -> None:
    with production.transaction() as transaction:
        transaction.cancel_job(job_id)


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
            transaction.add_metadata(job_id, parameter.property, parameter.value)
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
        source_id = production.representations[asset_id][0].id
        parameter = MetadataString("proxy-720p")

        job_id = request_proxy(production, source_id, asset_id)
        job = job_by_id(production, job_id)
        assert job.state is JobState.REQUESTED
        assert job.inputs == (source_id,)
        assert job.output_asset_id == asset_id
        assert job.output_representation_kind is RepresentationKind.PROXY
        assert job.target_root == "proxies"
        assert production.metadata[job_id] == (
            MetadataAssertion(job_id, PROFILE, parameter),
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
        assert production.metadata[producer.id] == (
            MetadataAssertion(producer.id, PROFILE, parameter),
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
        assert plan.parameters == (MetadataAssertion(plan.job.id, PROFILE, parameter),)
        latest = production.latest_revision
        assert revision is not None and latest is not None
        assert latest.sequence == revision.sequence + 1
        enqueued = job_by_id(production, enqueued_id)
        assert enqueued.state is JobState.REQUESTED
        assert enqueued.target_root == "proxies"
        assert production.metadata[enqueued_id] == (
            MetadataAssertion(enqueued_id, PROFILE, parameter),
        )
        requested = production.jobs(limit=100, state=JobState.REQUESTED).items
        assert [item.id for item in requested] == [enqueued_id]


if __name__ == "__main__":
    main()
