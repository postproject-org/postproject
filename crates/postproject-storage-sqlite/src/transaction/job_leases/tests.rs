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
    input: RepresentationId,
    resource: ResourceId,
}

impl Fixture {
    fn new() -> Self {
        Self::with_inputs(false)
    }

    fn with_inputs(has_inputs: bool) -> Self {
        let directory = tempdir().unwrap();
        let mut store =
            SqliteProduction::create(directory.path().join("lease.pproj"), None).unwrap();
        let clock = Arc::new(Clock(AtomicI64::new(100)));
        store.job_clock = clock.clone();
        let asset = AssetId::new();
        let resource = ResourceId::new();
        let input = RepresentationId::new();
        let source = OriginalMediaImport::new(
            Asset::new(asset, Timestamp::from_unix_micros(1), None, None),
            Representation::new(
                input,
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
            if has_inputs { vec![input] } else { vec![] },
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
            input,
            resource,
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

#[test]
fn claim_and_publication_share_one_commit_and_terminal_ownership() {
    let mut fixture = Fixture::new();
    let (output, activity) = fixture.output();
    let mut edit = fixture.store.begin_transaction().unwrap();
    let lease = edit
        .claim_job_lease(fixture.job, &tool(), None, Duration::from_micros(100))
        .unwrap();
    edit.complete_job_lease(&lease, &output, &activity).unwrap();
    let receipt = edit.commit_with_receipt().unwrap();
    assert!(receipt.revision().is_some());
    assert_eq!(lease.state().unwrap(), JobLeaseState::Closed);
    drop(edit);
    assert!(matches!(
        fixture.store.job(fixture.job).unwrap().state(),
        JobState::Succeeded(_)
    ));
    assert_eq!(
        fixture
            .store
            .representation(output.representation().id())
            .unwrap(),
        *output.representation()
    );
    assert!(
        fixture
            .store
            .begin_transaction()
            .unwrap()
            .release_job_lease(&lease)
            .is_err()
    );
}

#[test]
fn delayed_completion_rolls_back_publication_and_preserves_time_on_reopen() {
    let mut fixture = Fixture::new();
    let lease = fixture.claim();
    let (output, activity) = fixture.output();
    let previous = fixture.store.latest_revision().unwrap();
    let mut edit = fixture.store.begin_transaction().unwrap();
    edit.complete_job_lease(&lease, &output, &activity).unwrap();
    fixture.clock.set(200);
    assert_eq!(edit.commit().unwrap_err().kind(), ErrorKind::Conflict);
    assert_eq!(edit.state(), TransactionState::RolledBack);
    drop(edit);
    assert!(matches!(
        lease.state().unwrap(),
        JobLeaseState::Active { .. }
    ));
    assert!(
        fixture
            .store
            .representation(output.representation().id())
            .is_err()
    );
    assert_eq!(fixture.store.latest_revision().unwrap(), previous);
    let mut reopened = SqliteProduction::open(fixture.store.path()).unwrap();
    reopened.job_clock = fixture.clock.clone();
    fixture.clock.set(199);
    assert!(
        reopened
            .begin_transaction()
            .unwrap()
            .claim_job_lease(fixture.job, &tool(), None, Duration::from_micros(100))
            .is_err()
    );
    fixture.clock.set(200);
    let mut edit = reopened.begin_transaction().unwrap();
    edit.claim_job_lease(fixture.job, &tool(), None, Duration::from_micros(100))
        .unwrap();
    edit.commit().unwrap();
}

#[test]
fn renewal_must_commit_before_previous_expiry_and_validation_is_recoverable() {
    let mut fixture = Fixture::new();
    let lease = fixture.claim();
    fixture.clock.set(150);
    let mut edit = fixture.store.begin_transaction().unwrap();
    assert!(edit.renew_job_lease(&lease, Duration::ZERO).is_err());
    assert!(
        edit.renew_job_lease(&lease, Duration::from_nanos(1))
            .is_err()
    );
    assert!(
        edit.renew_job_lease(&lease, Duration::from_micros(10))
            .is_err()
    );
    edit.renew_job_lease(&lease, Duration::from_micros(200))
        .unwrap();
    fixture.clock.set(200);
    assert!(edit.commit().is_err());
    drop(edit);
    assert_eq!(
        lease.state().unwrap(),
        JobLeaseState::Active {
            expires_at: Timestamp::from_unix_micros(200)
        }
    );
    assert!(
        fixture
            .store
            .begin_transaction()
            .unwrap()
            .release_job_lease(&lease)
            .is_err()
    );
}

#[test]
fn wrong_scopes_and_coordinator_cancellation_cannot_publish() {
    let mut fixture = Fixture::new();
    let lease = fixture.claim();
    let token = lease.export_token().unwrap();
    let mut other = Fixture::new();
    assert_eq!(
        other.store.import_job_lease(&token).unwrap_err().kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        other
            .store
            .begin_transaction()
            .unwrap()
            .release_job_lease(&lease)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidArgument
    );
    let wrong_job = token.replace(&lease.job_id().to_string(), &JobId::new().to_string());
    assert!(fixture.store.import_job_lease(&wrong_job).is_err());
    let (output, activity) = fixture.output();
    let mut edit = fixture.store.begin_transaction().unwrap();
    edit.cancel_job(fixture.job).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut edit = fixture.store.begin_transaction().unwrap();
    assert!(edit.complete_job_lease(&lease, &output, &activity).is_err());
    assert!(edit.commit_with_receipt().unwrap().revision().is_none());
    drop(edit);
    assert!(
        fixture
            .store
            .representation(output.representation().id())
            .is_err()
    );
    assert!(fixture.store.import_job_lease(&token).is_err());
}

#[test]
fn schema_eighteen_claims_expire_without_losing_attribution_or_requests() {
    let mut fixture = Fixture::new();
    let mut edit = fixture.store.begin_transaction().unwrap();
    edit.claim_job_with_id(
        fixture.job,
        JobClaimId::new(),
        &tool(),
        None,
        Timestamp::from_unix_micros(10),
        Timestamp::from_unix_micros(1_000),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let path = fixture.store.path().to_path_buf();
    fixture
        .store
        .connection
        .execute_batch(
            "DROP TABLE job_clock;
         DELETE FROM schema_migrations WHERE version = 19;
         UPDATE productions SET schema_version = 18;
         PRAGMA user_version = 18;",
        )
        .unwrap();
    drop(fixture.store);
    let reopened = SqliteProduction::open(path).unwrap();
    let job = reopened.job(fixture.job).unwrap();
    let JobState::Claimed(claim) = job.state() else {
        panic!("claim attribution must remain");
    };
    assert_eq!(claim.expires_at(), Timestamp::from_unix_micros(0));
    assert_eq!(claim.tool().name(), "worker");
    let high_water: Option<i64> = reopened
        .connection
        .query_row(
            "SELECT high_water_micros FROM job_clock WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(high_water, None);
    assert_eq!(reopened.production().schema_version(), 19);
}

#[test]
fn changed_input_fingerprints_and_dependencies_reject_publication_after_reopen() {
    for kind in ["resource", "representation", "dependencies"] {
        let mut fixture = Fixture::with_inputs(true);
        let lease = fixture.claim();
        let token = lease.export_token().unwrap();
        let base = fixture.store.read_session().unwrap().decision_base();
        let mut edit = fixture.store.begin_edit(base).unwrap();
        match kind {
            "resource" => {
                edit.record_resource_fingerprint(
                    fixture.resource,
                    &postproject_core::ResourceFingerprint::new("example-content", 1, vec![2])
                        .unwrap(),
                )
                .unwrap();
            }
            "representation" => {
                edit.record_representation_fingerprint(
                    fixture.input,
                    &postproject_core::RepresentationFingerprint::new(
                        "example-content",
                        1,
                        vec![2],
                    )
                    .unwrap(),
                )
                .unwrap();
            }
            _ => {
                edit.record_dependency_set(fixture.input, &[]).unwrap();
            }
        }
        let changed = edit.commit_with_receipt().unwrap();
        drop(edit);
        let (output, _) = fixture.output();
        let activity = Activity::new(
            ActivityId::new(),
            ActivityKind::new("example:publish").unwrap(),
            vec![postproject_core::ActivityInput::new(fixture.input, None)],
            vec![ActivityOutput::new(output.representation().id(), None)],
        )
        .unwrap();
        let mut reopened = SqliteProduction::open(fixture.store.path()).unwrap();
        reopened.job_clock = fixture.clock.clone();
        let imported = reopened.import_job_lease(&token).unwrap();
        let mut edit = reopened.begin_transaction().unwrap();
        let error = edit
            .complete_job_lease(&imported, &output, &activity)
            .unwrap_err();
        let conflict = error.transaction_conflict_detail().unwrap();
        assert_eq!(
            conflict.superseding_revision(),
            changed.revision().unwrap().id()
        );
        assert_eq!(
            conflict.key().kind().as_str(),
            match kind {
                "resource" => "resource_fingerprint",
                "representation" => "representation_fingerprint",
                _ => "dependency_set",
            }
        );
        edit.rollback().unwrap();
        drop(edit);
        assert!(reopened.activities().unwrap().is_empty());
        assert_eq!(reopened.representations(fixture.asset).unwrap().len(), 1);
        assert!(matches!(
            reopened.job(fixture.job).unwrap().state(),
            JobState::Claimed(_)
        ));
    }
}

#[test]
fn claiming_from_an_older_view_checks_input_knowledge_before_acquisition() {
    let mut fixture = Fixture::with_inputs(true);
    let old = fixture.store.read_session().unwrap().decision_base();
    let mut edit = fixture.store.begin_edit(old).unwrap();
    edit.record_resource_fingerprint(
        fixture.resource,
        &postproject_core::ResourceFingerprint::new("example-content", 1, vec![2]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut edit = fixture.store.begin_edit(old).unwrap();
    let error = edit
        .claim_job_lease(fixture.job, &tool(), None, Duration::from_micros(100))
        .unwrap_err();
    assert!(error.transaction_conflict_detail().is_some());
    edit.commit().unwrap();
    drop(edit);
    assert!(matches!(
        fixture.store.job(fixture.job).unwrap().state(),
        JobState::Requested
    ));
}

#[test]
fn required_dependency_changes_cannot_be_attributed_to_earlier_work() {
    use postproject_core::{
        Dependency, DependencyKind, DependencyTarget, RepresentationFingerprint,
    };

    for floating in [false, true] {
        for change_set in [false, true] {
            let mut fixture = Fixture::with_inputs(true);
            let (dependency, _) = fixture.output();
            let target = dependency.representation().id();
            let edge = |id| {
                Dependency::new(
                    None,
                    DependencyKind::new("example:input").unwrap(),
                    if floating {
                        DependencyTarget::Asset(fixture.asset)
                    } else {
                        DependencyTarget::Representation(id)
                    },
                    floating.then_some(id),
                    true,
                    "reference",
                )
                .unwrap()
            };
            let base = fixture.store.read_session().unwrap().decision_base();
            let mut edit = fixture.store.begin_edit(base).unwrap();
            edit.add_representation(&dependency).unwrap();
            edit.record_dependency_set(fixture.input, &[edge(target)])
                .unwrap();
            // Required cycles must deduplicate, not hang or bypass the guard.
            edit.record_dependency_set(target, &[edge(fixture.input)])
                .unwrap();
            edit.commit().unwrap();
            drop(edit);
            let lease = fixture.claim();
            let base = fixture.store.read_session().unwrap().decision_base();
            let mut edit = fixture.store.begin_edit(base).unwrap();
            if change_set {
                edit.record_dependency_set(target, &[]).unwrap();
            } else {
                edit.record_representation_fingerprint(
                    target,
                    &RepresentationFingerprint::new("example-content", 1, vec![2]).unwrap(),
                )
                .unwrap();
            }
            let changed = edit.commit().unwrap();
            drop(edit);
            let (output, _) = fixture.output();
            let activity = Activity::new(
                ActivityId::new(),
                ActivityKind::new("example:publish").unwrap(),
                vec![postproject_core::ActivityInput::new(fixture.input, None)],
                vec![ActivityOutput::new(output.representation().id(), None)],
            )
            .unwrap();
            let mut edit = fixture.store.begin_transaction().unwrap();
            let error = edit
                .complete_job_lease(&lease, &output, &activity)
                .unwrap_err();
            let conflict = error.transaction_conflict_detail().unwrap();
            assert_eq!(
                conflict.superseding_revision(),
                changed.revision().unwrap().id()
            );
            assert_eq!(
                conflict.key().kind().as_str(),
                if change_set {
                    "dependency_set"
                } else {
                    "representation_fingerprint"
                }
            );
            edit.rollback().unwrap();
            drop(edit);
            assert_eq!(
                fixture.store.representations(fixture.asset).unwrap().len(),
                2
            );
            assert!(fixture.store.activities().unwrap().is_empty());
        }
    }
}

#[test]
fn corrupt_input_conflict_key_rejects_publication_as_storage_error() {
    let mut fixture = Fixture::with_inputs(true);
    let lease = fixture.claim();
    let base = fixture.store.read_session().unwrap().decision_base();
    let mut edit = fixture.store.begin_edit(base).unwrap();
    edit.record_resource_fingerprint(
        fixture.resource,
        &postproject_core::ResourceFingerprint::new("example-content", 1, vec![2]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut corrupt = vec![6];
    corrupt.extend_from_slice(fixture.resource.as_bytes());
    corrupt.extend_from_slice(&1_u32.to_be_bytes());
    corrupt.push(b'?');
    corrupt.extend_from_slice(&1_u16.to_be_bytes());
    fixture.store.connection.execute(
        "UPDATE conflict_versions SET conflict_key = ?1 WHERE substr(conflict_key, 1, 1) = x'06'",
        [corrupt],
    ).unwrap();
    let (output, _) = fixture.output();
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("example:publish").unwrap(),
        vec![postproject_core::ActivityInput::new(fixture.input, None)],
        vec![ActivityOutput::new(output.representation().id(), None)],
    )
    .unwrap();
    let mut edit = fixture.store.begin_transaction().unwrap();
    assert_eq!(
        edit.complete_job_lease(&lease, &output, &activity)
            .unwrap_err()
            .kind(),
        ErrorKind::Storage
    );
    edit.rollback().unwrap();
    drop(edit);
    assert!(fixture.store.activities().unwrap().is_empty());
    assert_eq!(
        fixture.store.representations(fixture.asset).unwrap().len(),
        1
    );
}
