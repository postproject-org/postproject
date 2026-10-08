//! Durable job request integration coverage.

use std::{
    env, fs,
    io::{Read, Write},
    process::{Command, Stdio},
    time::Duration,
};

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, AgentIdentity, Asset,
    AssetId, ContentStructure, ErrorKind, ExternalIdentifier, IdentifierScheme, Job, JobFailure,
    JobId, JobKind, JobLeaseState, JobQuery, JobState, JobStateKind, Locator, LocatorAvailability,
    LocatorId, MediaRoot, MediaRootId, MetadataProperty, MetadataValue, ObjectRef,
    OriginalMediaImport, PropertyId, QueryPageRequest, Representation, RepresentationId,
    RepresentationImport, RepresentationKind, RequestedJobOutput, Resource, ResourceId,
    RevisionEventKind, Timestamp, ToolIdentity, VocabularyId,
};
use postproject_storage_sqlite::SqliteProduction;
use tempfile::tempdir;

const CLAIM_WORKER_PRODUCTION: &str = "POSTPROJECT_TEST_CLAIM_PRODUCTION";
const CLAIM_WORKER_RESULT: &str = "POSTPROJECT_TEST_CLAIM_RESULT";

fn source_import() -> OriginalMediaImport {
    let asset_id = AssetId::from_bytes([1; 16]);
    let representation_id = RepresentationId::from_bytes([2; 16]);
    let resource_id = ResourceId::from_bytes([3; 16]);
    OriginalMediaImport::new(
        Asset::new(
            asset_id,
            Timestamp::from_unix_micros(1),
            Some("Source".to_owned()),
            None,
        ),
        Representation::new(
            representation_id,
            asset_id,
            RepresentationKind::Original,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        ),
        vec![Resource::new(resource_id, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::from_bytes([4; 16]),
                resource_id,
                "file:///media/source.mov",
                None,
                LocatorAvailability::Online,
            )
            .expect("valid locator"),
        ],
    )
    .expect("valid source import")
}

fn requested_job(source: &OriginalMediaImport) -> Job {
    Job::new(
        JobId::from_bytes([5; 16]),
        JobKind::new("org.postproject:generate-proxy").expect("valid job kind"),
        vec![source.representation().id()],
        RequestedJobOutput::new(
            source.asset().id(),
            RepresentationKind::Proxy,
            Some("proxies".to_owned()),
        )
        .expect("valid requested output"),
    )
    .expect("valid job")
}

fn all_jobs(production: &SqliteProduction) -> Vec<Job> {
    let page = production
        .jobs(
            &JobQuery::default(),
            &QueryPageRequest::new(1_000, None).expect("page request"),
        )
        .expect("list jobs");
    assert!(page.next_cursor().is_none());
    page.into_items()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one durable setup exercises paging, both filters, and cursor binding"
)]
fn job_queries_are_filtered_and_keyset_paginated() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("jobs.pproj"), None)
        .expect("create production");
    let source = source_import();
    let proxy_kind = JobKind::new("org.postproject:generate-proxy").expect("kind");
    let inspect_kind = JobKind::new("org.postproject:inspect-media").expect("kind");
    let jobs = [
        Job::new(
            JobId::from_bytes([5; 16]),
            proxy_kind.clone(),
            vec![source.representation().id()],
            RequestedJobOutput::new(source.asset().id(), RepresentationKind::Proxy, None)
                .expect("output"),
        )
        .expect("job"),
        Job::new(
            JobId::from_bytes([6; 16]),
            inspect_kind.clone(),
            vec![source.representation().id()],
            RequestedJobOutput::new(source.asset().id(), RepresentationKind::Derived, None)
                .expect("output"),
        )
        .expect("job"),
        Job::new(
            JobId::from_bytes([7; 16]),
            proxy_kind.clone(),
            vec![source.representation().id()],
            RequestedJobOutput::new(source.asset().id(), RepresentationKind::Proxy, None)
                .expect("output"),
        )
        .expect("job"),
    ];
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.import_original(&source).expect("import source");
        for job in &jobs {
            transaction.request_job(job).expect("request job");
        }
        transaction.cancel_job(jobs[1].id()).expect("cancel job");
        transaction.commit().expect("commit setup");
    }

    let query = JobQuery::default();
    let mut cursor = None;
    let mut ids = Vec::new();
    loop {
        let page = production
            .jobs(
                &query,
                &QueryPageRequest::new(1, cursor).expect("page request"),
            )
            .expect("query jobs");
        assert!(!page.traversal_truncated());
        ids.extend(page.items().iter().map(Job::id));
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(ids, jobs.iter().map(Job::id).collect::<Vec<_>>());

    let requested_proxies = JobQuery::new(Some(JobStateKind::Requested), Some(proxy_kind));
    let page = production
        .jobs(
            &requested_proxies,
            &QueryPageRequest::new(10, None).expect("page request"),
        )
        .expect("query requested proxies");
    assert_eq!(
        page.items().iter().map(Job::id).collect::<Vec<_>>(),
        [jobs[0].id(), jobs[2].id()]
    );
    assert!(page.next_cursor().is_none());

    let cancelled = JobQuery::new(Some(JobStateKind::Cancelled), None);
    let page = production
        .jobs(
            &cancelled,
            &QueryPageRequest::new(10, None).expect("page request"),
        )
        .expect("query cancelled jobs");
    assert_eq!(
        page.items().iter().map(Job::id).collect::<Vec<_>>(),
        [jobs[1].id()]
    );

    let first_page = production
        .jobs(
            &query,
            &QueryPageRequest::new(1, None).expect("page request"),
        )
        .expect("query first page");
    let mismatched = QueryPageRequest::new(1, first_page.next_cursor().cloned()).expect("page");
    let error = production
        .jobs(&requested_proxies, &mismatched)
        .expect_err("cursor must include filters");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
}

#[test]
fn claim_worker_process() {
    let Some(production_path) = env::var_os(CLAIM_WORKER_PRODUCTION) else {
        return;
    };
    let result_path = env::var_os(CLAIM_WORKER_RESULT).expect("worker result path");
    let mut signal = [0_u8];
    std::io::stdin()
        .read_exact(&mut signal)
        .expect("parent starts worker");

    let mut production = SqliteProduction::open(production_path).expect("open shared production");
    let mut transaction = production
        .begin_transaction()
        .expect("begin claim transaction");
    let tool = ToolIdentity::new("claim-worker", None, None).expect("valid tool");
    let outcome = match transaction.claim_job_lease(
        JobId::from_bytes([5; 16]),
        &tool,
        None,
        Duration::from_secs(60),
    ) {
        Ok(_) => {
            transaction.commit().expect("commit winning claim");
            "claimed"
        }
        Err(error) if error.kind() == ErrorKind::Conflict => "conflict",
        Err(error) => panic!("unexpected claim failure: {error}"),
    };
    fs::write(result_path, outcome).expect("write worker result");
}

#[test]
fn concurrent_processes_cannot_both_claim_one_job() {
    let directory = tempdir().expect("temporary directory");
    let production_path = directory.path().join("shared.pproj");
    let source = source_import();
    let job = requested_job(&source);
    let mut production = SqliteProduction::create(&production_path, Some("Claim race".to_owned()))
        .expect("create production");
    let mut transaction = production.begin_transaction().expect("begin setup");
    transaction.import_original(&source).expect("import source");
    transaction
        .add_media_root(
            MediaRoot::new(
                MediaRootId::from_bytes([6; 16]),
                "proxies",
                None,
                None,
                0,
                true,
            )
            .expect("valid root"),
        )
        .expect("add root");
    transaction.request_job(&job).expect("request job");
    transaction.commit().expect("commit setup");
    drop(transaction);
    drop(production);

    let executable = env::current_exe().expect("current test executable");
    let result_paths = [
        directory.path().join("worker-one.result"),
        directory.path().join("worker-two.result"),
    ];
    let mut children = result_paths
        .iter()
        .map(|result_path| {
            Command::new(&executable)
                .args(["--exact", "claim_worker_process", "--nocapture"])
                .env(CLAIM_WORKER_PRODUCTION, &production_path)
                .env(CLAIM_WORKER_RESULT, result_path)
                .stdin(Stdio::piped())
                .spawn()
                .expect("spawn claim worker")
        })
        .collect::<Vec<_>>();

    for child in &mut children {
        child
            .stdin
            .take()
            .expect("worker stdin")
            .write_all(b"x")
            .expect("start claim worker");
    }
    for child in &mut children {
        assert!(child.wait().expect("wait for claim worker").success());
    }

    let mut outcomes = result_paths
        .iter()
        .map(|path| fs::read_to_string(path).expect("read worker result"))
        .collect::<Vec<_>>();
    outcomes.sort_unstable();
    assert_eq!(outcomes, ["claimed", "conflict"]);
}

fn proxy_import(
    source: &OriginalMediaImport,
    representation_id: RepresentationId,
    resource_id: ResourceId,
) -> RepresentationImport {
    RepresentationImport::new(
        Representation::new(
            representation_id,
            source.asset().id(),
            RepresentationKind::Proxy,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        ),
        vec![Resource::new(resource_id, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::from_bytes([22; 16]),
                resource_id,
                "file:///media/proxy.mov",
                None,
                LocatorAvailability::Online,
            )
            .expect("valid proxy locator"),
        ],
    )
    .expect("valid proxy import")
}

#[test]
fn requested_job_and_metadata_round_trip_and_are_journaled() {
    let directory = tempdir().expect("create temporary directory");
    let path = directory.path().join("production.pproj");
    let source = source_import();
    let job = requested_job(&source);
    let property = MetadataProperty::new(
        VocabularyId::new("org.postproject.job").expect("valid vocabulary"),
        PropertyId::new("codec").expect("valid property"),
    );
    let value = MetadataValue::string("prores").expect("valid value");
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.import_original(&source).expect("import source");
        transaction
            .add_media_root(
                MediaRoot::new(
                    MediaRootId::from_bytes([6; 16]),
                    "proxies",
                    None,
                    None,
                    0,
                    true,
                )
                .expect("valid root"),
            )
            .expect("add root");
        transaction.commit().expect("commit setup");
    }
    {
        let mut transaction = production.begin_transaction().expect("begin request");
        transaction.request_job(&job).expect("request job");
        transaction
            .add_metadata_value(ObjectRef::Job(job.id()), &property, &value)
            .expect("add job parameter");
        transaction.commit().expect("commit request");
    }

    drop(production);
    let production = SqliteProduction::open(path).expect("reopen production");
    assert_eq!(production.job(job.id()).expect("load job"), job);
    assert_eq!(all_jobs(&production).as_slice(), std::slice::from_ref(&job));
    assert_eq!(
        production
            .metadata_values(ObjectRef::Job(job.id()), &property)
            .expect("load parameters"),
        [value]
    );
    let revision = production
        .latest_revision()
        .expect("load revision")
        .unwrap();
    let events = production
        .events_for_revision(revision.id())
        .expect("load events");
    assert!(matches!(
        events[0].kind(),
        RevisionEventKind::JobRequested { job_id } if *job_id == job.id()
    ));
    assert!(matches!(
        events[1].kind(),
        RevisionEventKind::MetadataAddedOrReplaced {
            target: ObjectRef::Job(job_id),
            property: event_property,
        } if *job_id == job.id() && event_property == &property
    ));
}

#[test]
fn invalid_job_references_leave_no_partial_request() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");
    let source = source_import();
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.import_original(&source).expect("import source");
        transaction.commit().expect("commit setup");
    }
    let missing_root = requested_job(&source);
    let error = production
        .begin_transaction()
        .expect("begin request")
        .request_job(&missing_root)
        .expect_err("missing root must fail");
    assert_eq!(error.kind(), ErrorKind::NotFound);

    let missing_input = Job::new(
        JobId::from_bytes([7; 16]),
        JobKind::new("org.postproject:generate-proxy").expect("valid kind"),
        vec![RepresentationId::from_bytes([8; 16])],
        RequestedJobOutput::new(source.asset().id(), RepresentationKind::Proxy, None)
            .expect("valid output"),
    )
    .expect("valid job");
    let mut transaction = production.begin_transaction().expect("begin request");
    let error = transaction
        .request_job(&missing_input)
        .expect_err("missing input must fail");
    assert_eq!(error.kind(), ErrorKind::NotFound);
    transaction.commit().expect("commit empty transaction");
    drop(transaction);
    assert_eq!(all_jobs(&production), []);
    assert!(matches!(missing_root.state(), JobState::Requested));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered scenario exercises token replacement, renewal, release, and failure"
)]
fn owning_leases_preserve_attribution_and_fence_released_workers() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");
    let source = source_import();
    let job = requested_job(&source);
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.import_original(&source).expect("import source");
        transaction
            .add_media_root(
                MediaRoot::new(
                    MediaRootId::from_bytes([6; 16]),
                    "proxies",
                    None,
                    None,
                    0,
                    true,
                )
                .expect("valid root"),
            )
            .expect("add root");
        transaction.request_job(&job).expect("request job");
        transaction.commit().expect("commit setup");
    }
    let tool = ToolIdentity::new(
        "worker",
        Some("1.0".to_owned()),
        Some("https://example.com/worker".to_owned()),
    )
    .expect("valid tool");
    let agent = AgentIdentity::new(
        Some("Render node".to_owned()),
        Some(
            ExternalIdentifier::new(
                IdentifierScheme::new("com.example.worker").expect("valid scheme"),
                "node-7",
                None,
            )
            .expect("valid identifier"),
        ),
    )
    .expect("valid agent");

    let first_claim = {
        let mut transaction = production.begin_transaction().expect("begin claim");
        let error = transaction
            .claim_job_lease(job.id(), &tool, Some(&agent), Duration::ZERO)
            .expect_err("zero duration must fail");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        let claim = transaction
            .claim_job_lease(job.id(), &tool, Some(&agent), Duration::from_secs(60))
            .expect("claim job");
        transaction.commit().expect("commit claim");
        claim
    };
    assert!(matches!(
        production.job(job.id()).expect("load claimed job").state(),
        JobState::Claimed(claim) if claim.tool() == &tool && claim.agent() == Some(&agent)
    ));

    let obsolete = production
        .import_job_lease(&first_claim.export_token().unwrap())
        .unwrap();
    {
        let mut edit = production.begin_transaction().unwrap();
        edit.release_job_lease(&first_claim).unwrap();
        edit.commit().unwrap();
    }
    let second_claim = {
        let mut transaction = production.begin_transaction().expect("begin replacement");
        let claim = transaction
            .claim_job_lease(job.id(), &tool, Some(&agent), Duration::from_secs(60))
            .expect("claim released job");
        transaction.commit().expect("commit replacement");
        claim
    };
    assert_eq!(first_claim.state().unwrap(), JobLeaseState::Closed);
    assert_ne!(
        second_claim.export_token().unwrap(),
        obsolete.export_token().unwrap()
    );
    {
        let mut transaction = production.begin_transaction().expect("begin renewal");
        let error = transaction
            .release_job_lease(&obsolete)
            .expect_err("stale token must fail");
        assert_eq!(error.kind(), ErrorKind::Conflict);
        transaction
            .renew_job_lease(&second_claim, Duration::from_secs(120))
            .expect("renew current claim");
        transaction
            .release_job_lease(&second_claim)
            .expect("release current claim");
        transaction.commit().expect("commit release");
    }
    assert!(matches!(
        production.job(job.id()).expect("load released job").state(),
        JobState::Requested
    ));

    let final_claim = {
        let mut transaction = production.begin_transaction().expect("begin final claim");
        let claim = transaction
            .claim_job_lease(job.id(), &tool, None, Duration::from_secs(60))
            .expect("claim released job");
        transaction.commit().expect("commit final claim");
        claim
    };
    let failure = JobFailure::new("encoder exited with status 1").expect("valid failure");
    {
        let mut transaction = production.begin_transaction().expect("begin failure");
        transaction
            .fail_job_lease(&final_claim, &failure)
            .expect("fail active claim");
        transaction.commit().expect("commit failure");
    }
    assert!(matches!(
        production.job(job.id()).expect("load failed job").state(),
        JobState::Failed(stored) if stored == &failure
    ));
    assert_eq!(production.activities().expect("load activities"), []);
    assert_eq!(
        production
            .representations(source.asset().id())
            .expect("load representations")
            .len(),
        1
    );
    let revision = production
        .latest_revision()
        .expect("load revision")
        .unwrap();
    assert!(matches!(
        production
            .events_for_revision(revision.id())
            .expect("load events")[0]
            .kind(),
        RevisionEventKind::JobFailed { job_id } if *job_id == job.id()
    ));
}

#[test]
fn cancellation_accepts_requested_and_claimed_jobs_only() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");
    let source = source_import();
    let requested = Job::new(
        JobId::from_bytes([9; 16]),
        JobKind::new("org.postproject:inspect-media").expect("valid kind"),
        vec![source.representation().id()],
        RequestedJobOutput::new(source.asset().id(), RepresentationKind::Derived, None)
            .expect("valid output"),
    )
    .expect("valid job");
    let claimed = Job::new(
        JobId::from_bytes([10; 16]),
        JobKind::new("org.postproject:generate-thumbnail").expect("valid kind"),
        vec![source.representation().id()],
        RequestedJobOutput::new(source.asset().id(), RepresentationKind::Derived, None)
            .expect("valid output"),
    )
    .expect("valid job");
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.import_original(&source).expect("import source");
        transaction.request_job(&requested).expect("request job");
        transaction.request_job(&claimed).expect("request job");
        transaction.commit().expect("commit setup");
    }
    let tool = ToolIdentity::new("worker", None, None).expect("valid tool");
    {
        let mut transaction = production.begin_transaction().expect("begin cancellation");
        transaction
            .claim_job_lease(claimed.id(), &tool, None, Duration::from_secs(60))
            .expect("claim job");
        transaction
            .cancel_job(requested.id())
            .expect("cancel requested job");
        transaction
            .cancel_job(claimed.id())
            .expect("cancel claimed job");
        transaction.commit().expect("commit cancellation");
    }
    for job_id in [requested.id(), claimed.id()] {
        assert!(matches!(
            production.job(job_id).expect("load cancelled job").state(),
            JobState::Cancelled
        ));
        let error = production
            .begin_transaction()
            .expect("begin repeated cancellation")
            .cancel_job(job_id)
            .expect_err("terminal job must not be cancelled twice");
        assert_eq!(error.kind(), ErrorKind::Conflict);
    }
    let missing = production
        .begin_transaction()
        .expect("begin missing cancellation")
        .cancel_job(JobId::from_bytes([11; 16]))
        .expect_err("missing job must fail");
    assert_eq!(missing.kind(), ErrorKind::NotFound);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one scenario proves failed completion rollback and successful atomic output provenance"
)]
fn completion_is_atomic_and_records_output_activity_and_snapshots() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");
    let source = source_import();
    let job = requested_job(&source);
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.import_original(&source).expect("import source");
        transaction
            .add_media_root(
                MediaRoot::new(
                    MediaRootId::from_bytes([6; 16]),
                    "proxies",
                    None,
                    None,
                    0,
                    true,
                )
                .expect("valid root"),
            )
            .expect("add root");
        transaction.request_job(&job).expect("request job");
        transaction.commit().expect("commit setup");
    }
    let claim = {
        let mut transaction = production.begin_transaction().expect("begin claim");
        let claim = transaction
            .claim_job_lease(
                job.id(),
                &ToolIdentity::new("worker", None, None).expect("valid tool"),
                None,
                Duration::from_secs(60),
            )
            .expect("claim job");
        transaction.commit().expect("commit claim");
        claim
    };
    let output_id = RepresentationId::from_bytes([20; 16]);
    let activity_id = ActivityId::from_bytes([23; 16]);
    let activity = Activity::new(
        activity_id,
        ActivityKind::new("org.postproject:transcode").expect("valid activity kind"),
        vec![ActivityInput::new(source.representation().id(), None)],
        vec![ActivityOutput::new(output_id, None)],
    )
    .expect("valid completion activity");

    let revision_before_failure = production
        .latest_revision()
        .expect("load revision")
        .expect("claim revision");
    let colliding_output = proxy_import(&source, output_id, source.resources()[0].id());
    {
        let mut transaction = production
            .begin_transaction()
            .expect("begin failed completion");
        let error = transaction
            .complete_job_lease(&claim, &colliding_output, &activity)
            .expect_err("colliding resource must fail");
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        transaction
            .commit()
            .expect("commit after rolled-back completion");
    }
    assert_eq!(
        production
            .latest_revision()
            .expect("load unchanged revision")
            .expect("claim revision")
            .id(),
        revision_before_failure.id()
    );
    assert!(matches!(
        production.job(job.id()).expect("load claimed job").state(),
        JobState::Claimed(stored) if stored.tool().name() == "worker"
    ));
    assert_eq!(
        production
            .representations(source.asset().id())
            .expect("load representations")
            .len(),
        1
    );
    assert_eq!(production.activities().expect("load activities"), []);

    let output = proxy_import(&source, output_id, ResourceId::from_bytes([21; 16]));
    {
        let mut transaction = production.begin_transaction().expect("begin completion");
        transaction
            .complete_job_lease(&claim, &output, &activity)
            .expect("complete job");
        transaction.commit().expect("commit completion");
    }
    assert!(matches!(
        production.job(job.id()).expect("load succeeded job").state(),
        JobState::Succeeded(completion)
            if completion.activity_id() == activity_id
                && completion.representation_id() == output_id
    ));
    assert_eq!(
        production
            .representations(source.asset().id())
            .expect("load representations")
            .len(),
        2
    );
    let activities = production.activities().expect("load activities");
    assert_eq!(activities.len(), 1);
    assert!(activities[0].inputs()[0].snapshot().is_some());
    assert!(activities[0].outputs()[0].snapshot().is_some());
    let revision = production
        .latest_revision()
        .expect("load revision")
        .unwrap();
    let events = production
        .events_for_revision(revision.id())
        .expect("load completion events");
    assert!(matches!(
        events.last().expect("job event").kind(),
        RevisionEventKind::JobSucceeded { job_id } if *job_id == job.id()
    ));
}

#[test]
fn regeneration_planning_repeats_the_completed_jobs_kind_and_target_root() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");
    let source = source_import();
    let job = requested_job(&source);
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.import_original(&source).expect("import source");
        transaction
            .add_media_root(
                MediaRoot::new(
                    MediaRootId::from_bytes([6; 16]),
                    "proxies",
                    None,
                    None,
                    0,
                    true,
                )
                .expect("valid root"),
            )
            .expect("add root");
        transaction.request_job(&job).expect("request job");
        transaction.commit().expect("commit setup");
    }
    let claim = {
        let mut transaction = production.begin_transaction().expect("begin claim");
        let claim = transaction
            .claim_job_lease(
                job.id(),
                &ToolIdentity::new("worker", None, None).expect("valid tool"),
                None,
                Duration::from_secs(60),
            )
            .expect("claim job");
        transaction.commit().expect("commit claim");
        claim
    };
    // The worker names its activity differently from the requested kind.
    let output_id = RepresentationId::from_bytes([40; 16]);
    let activity = Activity::new(
        ActivityId::from_bytes([41; 16]),
        ActivityKind::new("org.postproject:transcode").expect("valid activity kind"),
        vec![ActivityInput::new(source.representation().id(), None)],
        vec![ActivityOutput::new(output_id, None)],
    )
    .expect("valid completion activity");
    {
        let output = proxy_import(&source, output_id, ResourceId::from_bytes([42; 16]));
        let mut transaction = production.begin_transaction().expect("begin completion");
        transaction
            .complete_job_lease(&claim, &output, &activity)
            .expect("complete job");
        transaction.commit().expect("commit completion");
    }

    let plans = production
        .plan_regeneration(&[output_id])
        .expect("plan regeneration");
    assert_eq!(plans.len(), 1);
    let planned = plans[0].job();
    assert_eq!(planned.kind().as_str(), "org.postproject:generate-proxy");
    assert_eq!(planned.requested_output().target_root(), Some("proxies"));
    assert_eq!(planned.inputs(), [source.representation().id()]);
}

#[test]
fn regeneration_planning_copies_producer_inputs_kind_and_parameters_without_enqueuing() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");
    let source = source_import();
    let proxy_id = RepresentationId::from_bytes([30; 16]);
    let proxy = proxy_import(&source, proxy_id, ResourceId::from_bytes([31; 16]));
    let activity_id = ActivityId::from_bytes([32; 16]);
    let activity = Activity::new(
        activity_id,
        ActivityKind::new("org.postproject:generate-proxy").expect("valid kind"),
        vec![ActivityInput::new(source.representation().id(), None)],
        vec![ActivityOutput::new(proxy_id, None)],
    )
    .expect("valid activity")
    .with_tool(ToolIdentity::new("encoder", None, None).expect("valid tool"));
    let property = MetadataProperty::new(
        VocabularyId::new("org.postproject.job").expect("valid vocabulary"),
        PropertyId::new("profile").expect("valid property"),
    );
    let value = MetadataValue::string("editing-proxy").expect("valid value");
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.import_original(&source).expect("import source");
        transaction.add_representation(&proxy).expect("add proxy");
        transaction
            .create_activity(&activity)
            .expect("create producer");
        transaction
            .add_metadata_value(ObjectRef::Activity(activity_id), &property, &value)
            .expect("add activity parameter");
        transaction.commit().expect("commit setup");
    }
    let revision = production
        .latest_revision()
        .expect("load revision")
        .unwrap();

    let plans = production
        .plan_regeneration(&[proxy_id, proxy_id])
        .expect("plan regeneration");
    assert_eq!(plans.len(), 1);
    let plan = &plans[0];
    assert_eq!(plan.artifact_representation_id(), proxy_id);
    assert_eq!(plan.job().kind().as_str(), "org.postproject:generate-proxy");
    assert_eq!(plan.job().inputs(), [source.representation().id()]);
    assert_eq!(
        plan.job().requested_output().asset_id(),
        source.asset().id()
    );
    assert_eq!(
        plan.job().requested_output().representation_kind(),
        RepresentationKind::Proxy
    );
    assert_eq!(plan.job().requested_output().target_root(), None);
    assert_eq!(plan.parameters().len(), 1);
    assert_eq!(plan.parameters()[0].property(), &property);
    assert_eq!(plan.parameters()[0].value(), &value);
    assert_eq!(all_jobs(&production), []);
    assert_eq!(
        production
            .latest_revision()
            .expect("load revision")
            .unwrap()
            .id(),
        revision.id()
    );

    {
        let mut transaction = production.begin_transaction().expect("begin enqueue");
        transaction.request_job(plan.job()).expect("enqueue plan");
        for parameter in plan.parameters() {
            transaction
                .add_metadata_value(
                    ObjectRef::Job(plan.job().id()),
                    parameter.property(),
                    parameter.value(),
                )
                .expect("copy job parameter");
        }
        transaction.commit().expect("commit enqueue");
    }
    assert_eq!(
        production
            .metadata(ObjectRef::Job(plan.job().id()))
            .expect("load job parameters"),
        plan.parameters()
    );
    let error = production
        .plan_regeneration(&[source.representation().id()])
        .expect_err("original has no producing activity");
    assert_eq!(error.kind(), ErrorKind::Conflict);
}

#[test]
fn regeneration_plans_share_their_copied_parameter_budget() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("plans.pproj"), None)
        .expect("create production");
    let source = source_import();
    let first_id = RepresentationId::new();
    let second_id = RepresentationId::new();
    let first = proxy_import(&source, first_id, ResourceId::new());
    let second_resource = ResourceId::new();
    let second = RepresentationImport::new(
        Representation::new(
            second_id,
            source.asset().id(),
            RepresentationKind::Proxy,
            ContentStructure::single_resource(second_resource),
            Vec::new(),
        ),
        vec![Resource::new(second_resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                second_resource,
                "file:///media/second.mov",
                None,
                LocatorAvailability::Online,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let activity_id = ActivityId::new();
    let activity = Activity::new(
        activity_id,
        ActivityKind::new("example:encode").unwrap(),
        vec![ActivityInput::new(source.representation().id(), None)],
        vec![
            ActivityOutput::new(first_id, None),
            ActivityOutput::new(second_id, None),
        ],
    )
    .unwrap();
    let property = MetadataProperty::new(
        VocabularyId::new("example:parameters").unwrap(),
        PropertyId::new("payload").unwrap(),
    );
    let value = MetadataValue::bytes(vec![7; 9 * 1024 * 1024]).unwrap();
    let mut transaction = production.begin_transaction().unwrap();
    transaction.import_original(&source).unwrap();
    transaction.add_representation(&first).unwrap();
    transaction.add_representation(&second).unwrap();
    transaction.create_activity(&activity).unwrap();
    for _ in 0..4 {
        transaction
            .add_metadata_value(ObjectRef::Activity(activity_id), &property, &value)
            .unwrap();
    }
    let receipt = transaction.commit().unwrap();
    drop(transaction);
    let one = production
        .plan_regeneration(&[first_id])
        .expect("one plan is bounded");
    assert_eq!(one[0].parameters().len(), 4);
    assert_eq!(one[0].parameters()[0].value(), &value);
    assert_eq!(
        production
            .plan_regeneration(&[first_id, second_id])
            .unwrap_err()
            .kind(),
        ErrorKind::Unsupported
    );
    assert_eq!(all_jobs(&production), [] as [postproject_core::Job; 0]);
    assert_eq!(
        production.latest_revision().unwrap().unwrap().id(),
        receipt.revision().unwrap().id()
    );
}
