//! Runs the Rust listings of the durable job guide.
//!
//! Each `// [name]` ... `// [/name]` region is included verbatim by the
//! documentation build, so keep regions self-contained and readable.
//! Lease durations use library authority time.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, AgentIdentity, AssetId, Job,
    JobFailure, JobId, JobKind, JobLeaseState, JobState, JobStateKind, MediaRoot, MediaRootId,
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RepresentationId, RepresentationKind,
    RequestedJobOutput, Result, ToolIdentity, VocabularyId,
};
use postproject_media::{
    EXECUTOR_PARAMETER_VOCABULARY, EXECUTOR_PROFILE_PROPERTY, GENERATE_PROXY_JOB_KIND,
    PROXY_720P_PROFILE, prepare_original_media, prepare_representation,
};
use postproject_storage_sqlite::SqliteProduction;

// [request-job]
fn request_proxy_job(
    production: &mut SqliteProduction,
    input_id: RepresentationId,
    asset_id: AssetId,
) -> Result<JobId> {
    let job = Job::new(
        JobId::new(),
        JobKind::new(GENERATE_PROXY_JOB_KIND)?,
        vec![input_id],
        // The output goes to the logical "proxies" media root.
        RequestedJobOutput::new(
            asset_id,
            RepresentationKind::Proxy,
            Some("proxies".to_owned()),
        )?,
    )?;
    let profile = MetadataProperty::new(
        VocabularyId::new(EXECUTOR_PARAMETER_VOCABULARY)?,
        PropertyId::new(EXECUTOR_PROFILE_PROPERTY)?,
    );
    {
        let mut transaction = production.begin_transaction()?;
        transaction.request_job(&job)?;
        // Typed job parameters are ordinary metadata on the job.
        transaction.add_metadata_value(
            ObjectRef::Job(job.id()),
            &profile,
            &MetadataValue::string(PROXY_720P_PROFILE)?,
        )?;
        transaction.commit()?;
    }

    let stored = production.job(job.id())?;
    println!("job {} ({})", stored.id(), stored.kind().as_str());
    for input in stored.inputs() {
        println!("  input: {input}");
    }
    let output = stored.requested_output();
    println!(
        "  output: {:?} for asset {} under {:?}",
        output.representation_kind(),
        output.asset_id(),
        output.target_root()
    );
    println!("  state: {:?}", stored.state().kind());
    Ok(job.id())
}
// [/request-job]

// [claim-job]
fn claim_renew_release(production: &mut SqliteProduction, job_id: JobId) -> Result<()> {
    let tool = ToolIdentity::new("Example Worker", Some("1.0".to_owned()), None)?;
    let agent = AgentIdentity::new(Some("Render node 4".to_owned()), None)?;

    let mut transaction = production.begin_transaction()?;
    let lease =
        transaction.claim_job_lease(job_id, &tool, Some(&agent), Duration::from_secs(60))?;
    assert_eq!(lease.state()?, JobLeaseState::Pending);
    transaction.commit()?;
    drop(transaction);
    assert_eq!(lease.production_id(), production.production().id());
    assert_eq!(lease.job_id(), job_id);
    assert!(matches!(lease.state()?, JobLeaseState::Active { .. }));

    // Explicit private transport when another process takes over this worker.
    let imported = production.import_job_lease(&lease.export_token()?)?;
    let mut transaction = production.begin_transaction()?;
    transaction.renew_job_lease(&imported, Duration::from_secs(120))?;
    transaction.release_job_lease(&imported)?;
    transaction.commit()?;
    assert_eq!(imported.state()?, JobLeaseState::Closed);
    Ok(())
}
// [/claim-job]

// [complete-job]
fn complete_proxy_job(
    production: &mut SqliteProduction,
    job_id: JobId,
    output_path: &Path,
) -> Result<(RepresentationId, ActivityId)> {
    let tool = ToolIdentity::new("Example Worker", Some("1.0".to_owned()), None)?;
    let job = production.job(job_id)?;
    let claim = {
        let mut transaction = production.begin_transaction()?;
        let claim = transaction.claim_job_lease(job_id, &tool, None, Duration::from_secs(60))?;
        transaction.commit()?;
        claim
    };

    // ... the worker writes the proxy file to output_path here ...
    let output = prepare_representation(
        job.requested_output().asset_id(),
        RepresentationKind::Proxy,
        output_path,
    )?;
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new(GENERATE_PROXY_JOB_KIND)?,
        job.inputs()
            .iter()
            .map(|input| ActivityInput::new(*input, None))
            .collect(),
        vec![ActivityOutput::new(output.representation().id(), None)],
    )?
    .with_tool(tool);

    let parameters = production.metadata(ObjectRef::Job(job_id))?;
    let mut transaction = production.begin_transaction()?;
    // Output, activity, and the succeeded state commit together or not at all.
    transaction.complete_job_lease(&claim, &output, &activity)?;
    // Keep the parameters on the activity so the artifact can be regenerated.
    for parameter in &parameters {
        transaction.add_metadata_value(
            ObjectRef::Activity(activity.id()),
            parameter.property(),
            parameter.value(),
        )?;
    }
    transaction.commit()?;
    Ok((output.representation().id(), activity.id()))
}
// [/complete-job]

// [fail-job]
fn fail_claimed_job(production: &mut SqliteProduction, job_id: JobId) -> Result<()> {
    let tool = ToolIdentity::new("Example Worker", Some("1.0".to_owned()), None)?;
    let mut transaction = production.begin_transaction()?;
    let claim = transaction.claim_job_lease(job_id, &tool, None, Duration::from_secs(60))?;
    // Failing records the diagnostic only: no representation or activity.
    transaction.fail_job_lease(&claim, &JobFailure::new("encoder exited with status 1")?)?;
    transaction.commit()?;
    drop(transaction);

    if let JobState::Failed(failure) = production.job(job_id)?.state() {
        println!("job failed: {}", failure.diagnostic());
    }
    Ok(())
}
// [/fail-job]

// [cancel-job]
fn cancel_requested_job(
    production: &mut SqliteProduction,
    job_id: JobId,
) -> Result<postproject_core::CommitReceipt> {
    let mut transaction = production.begin_transaction()?;
    transaction.cancel_job(job_id)?;
    transaction.commit()
}
// [/cancel-job]

// [plan-regeneration]
fn regenerate(production: &mut SqliteProduction, artifact_id: RepresentationId) -> Result<JobId> {
    // Planning is read-only: nothing is enqueued until the caller requests it.
    let plans = production.plan_regeneration(&[artifact_id])?;
    let [plan] = plans.as_slice() else {
        panic!("one plan per artifact");
    };
    println!(
        "regenerate {} with {} ({} parameters)",
        plan.artifact_representation_id(),
        plan.job().kind().as_str(),
        plan.parameters().len()
    );

    let mut transaction = production.begin_transaction()?;
    transaction.request_job(plan.job())?;
    for parameter in plan.parameters() {
        transaction.add_metadata_value(
            ObjectRef::Job(plan.job().id()),
            parameter.property(),
            parameter.value(),
        )?;
    }
    transaction.commit()?;
    Ok(plan.job().id())
}
// [/plan-regeneration]

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/fixtures/sample-media.dat")
}

fn job_state(production: &SqliteProduction, job_id: JobId) -> Result<JobStateKind> {
    Ok(production.job(job_id)?.state().kind())
}

#[test]
fn job_examples_run_in_order() -> Result<()> {
    let work = tempfile::tempdir().expect("temporary directory");
    let rushes = work.path().join("rushes");
    let proxies = work.path().join("proxies");
    for directory in [&rushes, &proxies] {
        fs::create_dir_all(directory).expect("work directory");
    }
    fs::copy(fixture(), rushes.join("A001.mov")).expect("media fixture");

    let mut production = SqliteProduction::create(work.path().join("jobs.pproj"), None)?;
    let import = prepare_original_media(rushes.join("A001.mov"), None, None)?;
    let asset_id = import.asset().id();
    let original_id = import.representation().id();
    {
        let mut transaction = production.begin_transaction()?;
        transaction.import_original(&import)?;
        transaction.add_media_root(MediaRoot::new(
            MediaRootId::new(),
            "proxies",
            Some("Editorial proxies".to_owned()),
            None,
            0,
            true,
        )?)?;
        transaction.commit()?;
    }

    let job_id = request_proxy_job(&mut production, original_id, asset_id)?;
    let job = production.job(job_id)?;
    assert_eq!(job.kind().as_str(), GENERATE_PROXY_JOB_KIND);
    assert_eq!(job.inputs(), [original_id]);
    assert_eq!(job.requested_output().target_root(), Some("proxies"));
    assert_eq!(job_state(&production, job_id)?, JobStateKind::Requested);
    assert_eq!(production.metadata(ObjectRef::Job(job_id))?.len(), 1);

    claim_renew_release(&mut production, job_id)?;
    assert_eq!(job_state(&production, job_id)?, JobStateKind::Requested);
    let output_path = proxies.join("A001_proxy.mov");
    fs::write(&output_path, "proxy of A001").expect("proxy output");
    let (proxy_id, activity_id) = complete_proxy_job(&mut production, job_id, &output_path)?;
    let JobState::Succeeded(completion) = production.job(job_id)?.state().clone() else {
        panic!("job must have succeeded");
    };
    assert_eq!(completion.activity_id(), activity_id);
    assert_eq!(completion.representation_id(), proxy_id);
    assert_eq!(production.representations(asset_id)?.len(), 2);
    assert_eq!(
        production.activities_producing(proxy_id)?[0].id(),
        activity_id
    );

    let failing = request_proxy_job(&mut production, original_id, asset_id)?;
    fail_claimed_job(&mut production, failing)?;
    let JobState::Failed(failure) = production.job(failing)?.state().clone() else {
        panic!("job must have failed");
    };
    assert_eq!(failure.diagnostic(), "encoder exited with status 1");
    assert_eq!(production.representations(asset_id)?.len(), 2);

    let cancelled = request_proxy_job(&mut production, original_id, asset_id)?;
    cancel_requested_job(&mut production, cancelled)?;
    assert_eq!(job_state(&production, cancelled)?, JobStateKind::Cancelled);

    let regenerated = regenerate(&mut production, proxy_id)?;
    let planned = production.job(regenerated)?;
    assert_eq!(planned.kind().as_str(), GENERATE_PROXY_JOB_KIND);
    assert_eq!(planned.inputs(), [original_id]);
    assert_eq!(planned.state().kind(), JobStateKind::Requested);
    assert_eq!(
        production.metadata(ObjectRef::Job(regenerated))?,
        production.metadata(ObjectRef::Job(job_id))?
    );
    Ok(())
}
