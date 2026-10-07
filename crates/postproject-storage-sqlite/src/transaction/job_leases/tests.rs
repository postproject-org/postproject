//! Deterministic authority/fencing coverage without public clock injection.

use std::sync::atomic::{AtomicI64, Ordering};

use postproject_core::{
    ActivityId, ActivityKind, ActivityOutput, Asset, AssetId, ContentStructure, Job, JobKind,
    JobState, Locator, LocatorAvailability, LocatorId, OriginalMediaImport, Representation,
    RepresentationId, RepresentationKind, RequestedJobOutput, Resource, ResourceId,
};
use tempfile::{TempDir, tempdir};

use super::*;
use crate::{SqliteProduction, job_clock::JobClock};

#[derive(Debug)]
struct Clock(AtomicI64);

impl Clock {
    fn set(&self, micros: i64) {
        self.0.store(micros, Ordering::SeqCst);
    }
}

impl JobClock for Clock {
    fn now(&self) -> Result<Timestamp> {
        Ok(Timestamp::from_unix_micros(self.0.load(Ordering::SeqCst)))
    }
}

struct Fixture {
    _directory: TempDir,
    store: SqliteProduction,
    clock: Arc<Clock>,
    job: JobId,
    asset: AssetId,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempdir().unwrap();
        let mut store =
            SqliteProduction::create(directory.path().join("lease.pproj"), None).unwrap();
        let clock = Arc::new(Clock(AtomicI64::new(100)));
        store.job_clock = clock.clone();
        let asset = AssetId::new();
        let resource = ResourceId::new();
        let source = OriginalMediaImport::new(
            Asset::new(asset, Timestamp::from_unix_micros(1), None, None),
            Representation::new(
                RepresentationId::new(),
                asset,
                RepresentationKind::Original,
                ContentStructure::single_resource(resource),
                vec![],
            ),
            vec![Resource::new(resource, vec![], None)],
            vec![locator(resource)],
        )
        .unwrap();
        let job = Job::new(
            JobId::new(),
            JobKind::new("org.example:publish").unwrap(),
            vec![],
            RequestedJobOutput::new(asset, RepresentationKind::Derived, None).unwrap(),
        )
        .unwrap();
        let mut edit = store.begin_transaction().unwrap();
        edit.import_original(&source).unwrap();
        edit.request_job(&job).unwrap();
        edit.commit().unwrap();
        drop(edit);
        Self {
            _directory: directory,
            store,
            clock,
            job: job.id(),
            asset,
        }
    }

    fn claim(&mut self) -> SqliteJobLease {
        let mut edit = self.store.begin_transaction().unwrap();
        let lease = edit
            .claim_job_lease(self.job, &tool(), None, Duration::from_micros(100))
            .unwrap();
        assert_eq!(lease.state().unwrap(), JobLeaseState::Pending);
        assert!(lease.export_token().is_err());
        edit.commit().unwrap();
        assert_eq!(
            lease.state().unwrap(),
            JobLeaseState::Active {
                expires_at: Timestamp::from_unix_micros(200)
            }
        );
        lease
    }

    fn output(&self) -> (RepresentationImport, Activity) {
        let resource = ResourceId::new();
        let representation = Representation::new(
            RepresentationId::new(),
            self.asset,
            RepresentationKind::Derived,
            ContentStructure::single_resource(resource),
            vec![],
        );
        let activity = Activity::new(
            ActivityId::new(),
            ActivityKind::new("org.example:publish").unwrap(),
            vec![],
            vec![ActivityOutput::new(representation.id(), None)],
        )
        .unwrap();
        let output = RepresentationImport::new(
            representation,
            vec![Resource::new(resource, vec![], None)],
            vec![locator(resource)],
        )
        .unwrap();
        (output, activity)
    }
}

fn tool() -> ToolIdentity {
    ToolIdentity::new("worker", None, None).unwrap()
}

fn locator(resource: ResourceId) -> Locator {
    Locator::new(
        LocatorId::new(),
        resource,
        "file:///media/test.mov",
        None,
        LocatorAvailability::Online,
    )
    .unwrap()
}

#[test]
fn pending_ownership_activates_on_commit_and_closes_on_rollback_or_drop() {
    let mut fixture = Fixture::new();
    for explicit in [false, true] {
        let mut edit = fixture.store.begin_transaction().unwrap();
        let lease = edit
            .claim_job_lease(fixture.job, &tool(), None, Duration::from_micros(100))
            .unwrap();
        if explicit {
            edit.rollback().unwrap();
        }
        drop(edit);
        assert_eq!(lease.state().unwrap(), JobLeaseState::Closed);
        assert!(lease.export_token().is_err());
        assert!(matches!(
            fixture.store.job(fixture.job).unwrap().state(),
            JobState::Requested
        ));
    }
    let lease = fixture.claim();
    let token = lease.export_token().unwrap();
    let imported = fixture.store.import_job_lease(&token).unwrap();
    assert_eq!(imported.job_id(), lease.job_id());
    let mut edit = fixture.store.begin_transaction().unwrap();
    edit.release_job_lease(&lease).unwrap();
    assert!(matches!(
        lease.state().unwrap(),
        JobLeaseState::Active { .. }
    ));
    edit.commit().unwrap();
    drop(edit);
    assert_eq!(lease.state().unwrap(), JobLeaseState::Closed);
    assert!(fixture.store.import_job_lease(&token).is_err());
    assert!(
        fixture
            .store
            .begin_transaction()
            .unwrap()
            .release_job_lease(&imported)
            .is_err()
    );
}

#[test]
fn expiry_and_reacquisition_fence_every_old_worker_transition() {
    let mut fixture = Fixture::new();
    let old = fixture.claim();
    let token = old.export_token().unwrap();
    let (output, activity) = fixture.output();
    fixture.clock.set(200);
    let mut edit = fixture.store.begin_transaction().unwrap();
    let current = edit
        .claim_job_lease(fixture.job, &tool(), None, Duration::from_micros(100))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut edit = fixture.store.begin_transaction().unwrap();
    assert!(
        edit.renew_job_lease(&old, Duration::from_micros(100))
            .is_err()
    );
    assert!(edit.release_job_lease(&old).is_err());
    assert!(
        edit.fail_job_lease(&old, &JobFailure::new("failed").unwrap())
            .is_err()
    );
    assert!(edit.complete_job_lease(&old, &output, &activity).is_err());
    assert!(edit.commit_with_receipt().unwrap().revision().is_none());
    drop(edit);
    assert!(fixture.store.import_job_lease(&token).is_err());
    assert!(
        fixture
            .store
            .representation(output.representation().id())
            .is_err()
    );
    let activity_count: i64 = fixture
        .store
        .connection
        .query_row(
            "SELECT count(*) FROM activities WHERE id = ?1",
            [activity.id().as_bytes().as_slice()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(activity_count, 0);
    let mut edit = fixture.store.begin_transaction().unwrap();
    edit.fail_job_lease(&current, &JobFailure::new("worker failed").unwrap())
        .unwrap();
    edit.commit().unwrap();
    assert_eq!(current.state().unwrap(), JobLeaseState::Closed);
}
