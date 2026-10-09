//! Native lifecycle effects converge without conferring worker ownership.

#[path = "job_replay/atomic.rs"]
mod atomic;
#[path = "job_replay/integrity.rs"]
mod integrity;
#[path = "activity_replay/support.rs"]
mod support;

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ContentStructure,
    DependencyTarget, Job, JobFailure, JobId, JobKind, JobState, Representation, RepresentationId,
    RepresentationImport, RepresentationKind, RequestedJobOutput, Resource, ResourceId,
    ToolIdentity,
};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};
use std::time::Duration;

fn request(asset: postproject_core::AssetId, input: RepresentationId) -> Job {
    Job::new(
        JobId::new(),
        JobKind::new("unknown:Exact").unwrap(),
        vec![input],
        RequestedJobOutput::new(asset, RepresentationKind::Derived, None).unwrap(),
    )
    .unwrap()
}

fn output(asset: postproject_core::AssetId) -> RepresentationImport {
    let resource = ResourceId::new();
    RepresentationImport::new(
        Representation::new(
            RepresentationId::new(),
            asset,
            RepresentationKind::Derived,
            ContentStructure::single_resource(resource),
            Vec::new(),
        ),
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            postproject_core::Locator::new(
                postproject_core::LocatorId::new(),
                resource,
                "file:///unavailable/generated.dat",
                None,
                postproject_core::LocatorAvailability::Online,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn fixture(directory: &std::path::Path) -> (SqliteProduction, [JobId; 4], String) {
    let mut source = SqliteProduction::create(directory.join("source.pproj"), None).unwrap();
    let media = support::media(1, true);
    let jobs = std::array::from_fn::<_, 4, _>(|_| {
        request(media.asset().id(), media.representation().id())
    });
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&media).unwrap();
    for job in &jobs {
        edit.request_job(job).unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    let tool = ToolIdentity::new("  Exact worker  ", Some("custom".into()), None).unwrap();
    let hour = Duration::from_secs(3600);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_dependency_set(
        media.representation().id(),
        &[support::dependency(
            DependencyTarget::Representation(media.representation().id()),
            false,
        )],
    )
    .unwrap();
    let first = edit
        .claim_job_lease(jobs[0].id(), &tool, None, hour)
        .unwrap();
    edit.renew_job_lease(&first, hour * 2).unwrap();
    edit.release_job_lease(&first).unwrap();
    let second = edit
        .claim_job_lease(jobs[0].id(), &tool, None, hour)
        .unwrap();
    edit.fail_job_lease(
        &second,
        &JobFailure::new("  Exact 镜头 diagnostic  ").unwrap(),
    )
    .unwrap();
    let cancelled = edit
        .claim_job_lease(jobs[1].id(), &tool, None, hour)
        .unwrap();
    edit.cancel_job(jobs[1].id()).unwrap();
    let complete = edit
        .claim_job_lease(jobs[2].id(), &tool, None, hour)
        .unwrap();
    let output = output(media.asset().id());
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Exact").unwrap(),
        vec![ActivityInput::new(media.representation().id(), None)],
        vec![ActivityOutput::new(output.representation().id(), None)],
    )
    .unwrap();
    edit.complete_job_lease(&complete, &output, &activity)
        .unwrap();
    let active = edit
        .claim_job_lease(jobs[3].id(), &tool, None, hour)
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    drop(cancelled);
    (
        source,
        jobs.map(|job| job.id()),
        active.export_token().unwrap(),
    )
}

#[test]
fn intermediate_transitions_publication_and_inert_claims_survive_restart() {
    let directory = tempfile::tempdir().unwrap();
    let (source, jobs, token) = fixture(directory.path());
    let path = directory.path().join("mirror.pproj");
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    for sequence in 1..=2 {
        support::apply(&source, &mut mirror, sequence);
    }
    for job in jobs {
        assert_eq!(mirror.job(job).unwrap(), source.job(job).unwrap());
    }
    assert!(matches!(
        mirror.job(jobs[3]).unwrap().state(),
        JobState::Claimed(_)
    ));
    assert_eq!(mirror.activities().unwrap(), source.activities().unwrap());
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert!(mirror.import_job_lease(&token).is_err());
    assert!(mirror.begin_transaction().is_err());
    let mut reader = source.record_reader(2).unwrap();
    assert!(
        !mirror
            .apply_record(
                &reader.manifest().clone(),
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
    drop(mirror);
    let mut reopened = SqliteProduction::open(&path).unwrap();
    assert_eq!(reopened.job(jobs[3]).unwrap(), source.job(jobs[3]).unwrap());
    assert!(reopened.import_job_lease(&token).is_err());
    let connection = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM jobs WHERE claim_id IS NOT NULL",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT sum(claim_inert) FROM jobs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT high_water_micros FROM job_clock", [], |row| row
                .get::<_, Option<
                i64,
            >>(
                0
            ))
            .unwrap(),
        None
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM exchange_outcomes", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
