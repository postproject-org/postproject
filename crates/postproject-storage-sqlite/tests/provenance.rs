//! Activity persistence, atomicity, and cycle-invariant integration tests.

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityRole, AgentIdentity,
    ArtifactReproducibilityIssue, Asset, AssetId, ContentStructure, ErrorKind, ExternalIdentifier,
    IdentifierScheme, Locator, LocatorAvailability, LocatorId, MetadataProperty, MetadataValue,
    ObjectRef, OriginalMediaImport, PropertyId, Representation, RepresentationId,
    RepresentationKind, Resource, ResourceId, Timestamp, ToolIdentity, VocabularyId,
};
use postproject_storage_sqlite::SqliteProduction;
use tempfile::tempdir;

fn import(label: u8) -> (OriginalMediaImport, RepresentationId) {
    let asset_id = AssetId::from_bytes([label; 16]);
    let representation_id = RepresentationId::from_bytes([label; 16]);
    let resource_id = ResourceId::from_bytes([label; 16]);
    let asset = Asset::new(
        asset_id,
        Timestamp::from_unix_micros(i64::from(label)),
        None,
        None,
    );
    let representation = Representation::new(
        representation_id,
        asset_id,
        RepresentationKind::Original,
        ContentStructure::single_resource(resource_id),
        Vec::new(),
    );
    let resource = Resource::new(resource_id, Vec::new(), None);
    let locator = Locator::new(
        LocatorId::from_bytes([label; 16]),
        resource_id,
        format!("file:///media/{label}.mov"),
        None,
        LocatorAvailability::Online,
    )
    .expect("valid locator");
    (
        OriginalMediaImport::new(asset, representation, vec![resource], vec![locator])
            .expect("valid import"),
        representation_id,
    )
}

fn activity(id: ActivityId, input: RepresentationId, output: RepresentationId) -> Activity {
    Activity::new(
        id,
        ActivityKind::new("org.postproject:transcode").expect("valid kind"),
        vec![ActivityInput::new(input, None)],
        vec![ActivityOutput::new(output, None)],
    )
    .expect("valid activity")
}

#[test]
fn complete_ancestry_rejects_depth_truncation() {
    let directory = tempdir().unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("deep.pproj"), None).unwrap();
    let mut transaction = production.begin_transaction().unwrap();
    for label in 1..=66 {
        transaction.import_original(&import(label).0).unwrap();
        if label > 1 {
            transaction
                .create_activity(&activity(
                    ActivityId::new(),
                    RepresentationId::from_bytes([label - 1; 16]),
                    RepresentationId::from_bytes([label; 16]),
                ))
                .unwrap();
        }
    }
    transaction.commit().unwrap();
    drop(transaction);
    assert_eq!(
        production
            .ancestors(RepresentationId::from_bytes([66; 16]))
            .unwrap_err()
            .kind(),
        ErrorKind::Unsupported
    );
    assert_eq!(
        production
            .descendants(RepresentationId::from_bytes([1; 16]))
            .unwrap_err()
            .kind(),
        ErrorKind::Unsupported
    );
    assert_eq!(
        production
            .ancestors(RepresentationId::from_bytes([2; 16]))
            .unwrap(),
        [RepresentationId::from_bytes([1; 16])]
    );
}

fn assert_activity_identifier(
    production: &SqliteProduction,
    activity_id: ActivityId,
    identifier: &ExternalIdentifier,
) {
    let target = ObjectRef::Activity(activity_id);
    assert_eq!(
        production
            .external_identifiers(target)
            .expect("load activity identifiers"),
        std::slice::from_ref(identifier)
    );
    assert_eq!(
        production
            .find_by_external_identifier(identifier.scheme(), identifier.value(), None)
            .expect("look up activity identifier"),
        [target]
    );
}

#[test]
fn activity_metadata_is_atomic_with_activity_creation() {
    let directory = tempdir().expect("create temporary directory");
    let path = directory.path().join("production.pproj");
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    let (source, source_id) = import(1);
    let (proxy, proxy_id) = import(2);
    let activity_id = ActivityId::new();
    let agent_identifier = ExternalIdentifier::new(
        IdentifierScheme::new("com.example.worker").expect("valid scheme"),
        "worker-7",
        None,
    )
    .expect("valid identifier");
    let activity_identifier = ExternalIdentifier::new(
        IdentifierScheme::new("com.example.render-job").expect("valid scheme"),
        "job-42",
        None,
    )
    .expect("valid identifier");
    let activity = Activity::new(
        activity_id,
        ActivityKind::new("org.postproject:transcode").expect("valid kind"),
        vec![ActivityInput::new(
            source_id,
            Some(ActivityRole::new("org.postproject:input.primary").expect("valid role")),
        )],
        vec![ActivityOutput::new(
            proxy_id,
            Some(ActivityRole::new("org.postproject:output.proxy").expect("valid role")),
        )],
    )
    .expect("valid activity")
    .with_timing(
        Some(Timestamp::from_unix_micros(10)),
        Some(Timestamp::from_unix_micros(20)),
    )
    .expect("valid timing")
    .with_tool(
        ToolIdentity::new(
            "FFmpeg",
            Some("8.0".to_owned()),
            Some("https://ffmpeg.org".to_owned()),
        )
        .expect("valid tool"),
    )
    .with_agent(
        AgentIdentity::new(Some("Render worker".to_owned()), Some(agent_identifier))
            .expect("valid agent"),
    );
    let property = MetadataProperty::new(
        VocabularyId::new("com.example.provenance").expect("valid vocabulary"),
        PropertyId::new("preset").expect("valid property"),
    );
    let value = MetadataValue::string("editorial-proxy").expect("valid value");
    {
        let mut transaction = production.begin_transaction().expect("begin transaction");
        transaction.import_original(&source).expect("import source");
        transaction.import_original(&proxy).expect("import proxy");
        transaction
            .create_activity(&activity)
            .expect("create activity");
        transaction
            .add_external_identifier(ObjectRef::Activity(activity_id), &activity_identifier)
            .expect("attach activity identifier");
        transaction
            .add_metadata_value(ObjectRef::Activity(activity_id), &property, &value)
            .expect("attach activity metadata");
        transaction.commit().expect("commit provenance");
    }

    let reopened = SqliteProduction::open(&path).expect("reopen production");
    let activities = reopened.activities().expect("load activities");
    assert_eq!(activities.len(), 1);
    assert_eq!(activities[0].id(), activity.id());
    assert!(activities[0].inputs()[0].snapshot().is_some());
    assert!(activities[0].outputs()[0].snapshot().is_some());
    assert_eq!(
        reopened
            .activities_producing(proxy_id)
            .expect("load producing activities")[0]
            .id(),
        activity.id()
    );
    assert_eq!(
        reopened
            .activities_consuming(source_id)
            .expect("load consuming activities")[0]
            .id(),
        activity.id()
    );
    assert_eq!(
        reopened
            .metadata_values(ObjectRef::Activity(activity_id), &property)
            .expect("load activity metadata"),
        [value]
    );
    assert_activity_identifier(&reopened, activity_id, &activity_identifier);
}

#[test]
fn reproducibility_report_names_each_missing_condition() {
    let directory = tempdir().expect("create temporary directory");
    let path = directory.path().join("reproducibility.pproj");
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    let (source, source_id) = import(11);
    let (proxy, proxy_id) = import(12);
    let (render, render_id) = import(13);
    let incomplete = activity(ActivityId::new(), source_id, proxy_id);
    let complete_id = ActivityId::new();
    let complete = activity(complete_id, proxy_id, render_id).with_tool(
        ToolIdentity::new("Renderer", Some("1.0".to_owned()), None).expect("valid tool"),
    );
    let parameter = MetadataProperty::new(
        VocabularyId::new("org.postproject.parameters").expect("valid vocabulary"),
        PropertyId::new("profile").expect("valid property"),
    );
    let value = MetadataValue::string("review").expect("valid value");

    let mut transaction = production.begin_transaction().expect("begin transaction");
    for item in [&source, &proxy, &render] {
        transaction.import_original(item).expect("import media");
    }
    transaction
        .create_activity(&incomplete)
        .expect("create incomplete activity");
    transaction
        .create_activity(&complete)
        .expect("create complete activity");
    transaction
        .add_metadata_value(ObjectRef::Activity(complete_id), &parameter, &value)
        .expect("record activity parameter");
    transaction.commit().expect("commit provenance");
    drop(transaction);

    let source_report = production
        .artifact_reproducibility(source_id)
        .expect("report original reproducibility");
    assert_eq!(
        source_report.issues(),
        [ArtifactReproducibilityIssue::ProducingActivityMissing]
    );

    let proxy_report = production
        .artifact_reproducibility(proxy_id)
        .expect("report proxy reproducibility");
    assert!(!proxy_report.is_reproducible());
    assert!(proxy_report.issues().iter().any(|issue| matches!(
        issue,
        ArtifactReproducibilityIssue::ToolIdentityMissing { .. }
    )));
    assert!(proxy_report.issues().iter().any(|issue| matches!(
        issue,
        ArtifactReproducibilityIssue::ParametersMissing { .. }
    )));

    let render_report = production
        .artifact_reproducibility(render_id)
        .expect("report render reproducibility");
    assert!(render_report.is_reproducible());
    assert_eq!(render_report.producing_activity_id(), Some(complete_id));
    assert_eq!(
        render_report.activity_kind().map(ActivityKind::as_str),
        Some("org.postproject:transcode")
    );
}

#[test]
fn invalid_activities_leave_no_partial_rows() {
    let directory = tempdir().expect("create temporary directory");
    let path = directory.path().join("production.pproj");
    let mut production = SqliteProduction::create(path, None).expect("create production");
    let (source, source_id) = import(3);
    let (proxy, proxy_id) = import(4);
    let (delivery, delivery_id) = import(5);
    {
        let mut transaction = production.begin_transaction().expect("begin transaction");
        transaction.import_original(&source).expect("import source");
        transaction.import_original(&proxy).expect("import proxy");
        transaction
            .import_original(&delivery)
            .expect("import delivery");
        transaction
            .create_activity(&activity(ActivityId::new(), source_id, proxy_id))
            .expect("create first activity");
        transaction.commit().expect("commit fixtures");
    }

    let rejected_id = ActivityId::new();
    let missing = RepresentationId::new();
    let property = MetadataProperty::new(
        VocabularyId::new("com.example.provenance").expect("valid vocabulary"),
        PropertyId::new("note").expect("valid property"),
    );
    let value = MetadataValue::string("invalid").expect("valid value");
    let mut transaction = production.begin_transaction().expect("begin transaction");
    assert_eq!(
        transaction
            .create_activity(&activity(rejected_id, source_id, missing))
            .expect_err("missing output must fail")
            .kind(),
        ErrorKind::NotFound
    );
    assert_eq!(
        transaction
            .add_metadata_value(ObjectRef::Activity(rejected_id), &property, &value)
            .expect_err("failed activity must be absent")
            .kind(),
        ErrorKind::NotFound
    );
    assert_eq!(
        transaction
            .create_activity(&activity(ActivityId::new(), proxy_id, source_id))
            .expect_err("cycle must fail")
            .kind(),
        ErrorKind::Conflict
    );
    transaction
        .create_activity(&activity(ActivityId::new(), proxy_id, delivery_id))
        .expect("transaction remains usable");
    transaction.commit().expect("commit valid activity");
    drop(transaction);

    assert_eq!(
        production.ancestors(delivery_id).expect("load ancestry"),
        [source_id, proxy_id]
    );
    assert_eq!(
        production.descendants(source_id).expect("load descendants"),
        [proxy_id, delivery_id]
    );
    assert_eq!(
        production
            .ancestors(RepresentationId::new())
            .expect_err("missing representation must fail")
            .kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn rollback_discards_activity_rows() {
    let directory = tempdir().expect("create temporary directory");
    let path = directory.path().join("production.pproj");
    let mut production = SqliteProduction::create(path, None).expect("create production");
    let (source, source_id) = import(6);
    let (proxy, proxy_id) = import(7);
    let activity_id = ActivityId::new();
    {
        let mut transaction = production.begin_transaction().expect("begin transaction");
        transaction.import_original(&source).expect("import source");
        transaction.import_original(&proxy).expect("import proxy");
        transaction
            .create_activity(&activity(activity_id, source_id, proxy_id))
            .expect("create activity");
        transaction.rollback().expect("roll back provenance");
    }

    let property = MetadataProperty::new(
        VocabularyId::new("com.example.provenance").expect("valid vocabulary"),
        PropertyId::new("note").expect("valid property"),
    );
    let value = MetadataValue::string("absent").expect("valid value");
    let mut transaction = production.begin_transaction().expect("begin transaction");
    assert_eq!(
        transaction
            .add_metadata_value(ObjectRef::Activity(activity_id), &property, &value)
            .expect_err("rolled-back activity must be absent")
            .kind(),
        ErrorKind::NotFound
    );
    transaction.rollback().expect("close transaction");
}

#[test]
fn failed_activity_staging_rolls_back_even_when_delete_cleanup_would_fail() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("failed-activity.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let (source, source_id) = import(21);
    let (output, output_id) = import(22);
    let mut transaction = production.begin_transaction().unwrap();
    transaction.import_original(&source).unwrap();
    transaction.import_original(&output).unwrap();
    transaction.commit().unwrap();
    drop(transaction);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_output BEFORE INSERT ON activity_outputs BEGIN SELECT RAISE(ABORT, 'injected output failure'); END;
        CREATE TRIGGER fail_delete BEFORE DELETE ON activities BEGIN SELECT RAISE(ABORT, 'injected cleanup failure'); END;").unwrap();
    let activity = activity(ActivityId::new(), source_id, output_id);
    let mut transaction = production.begin_transaction().unwrap();
    assert!(transaction.create_activity(&activity).is_err());
    transaction
        .add_media_root(
            postproject_core::MediaRoot::new(
                postproject_core::MediaRootId::new(),
                "independent",
                None,
                None,
                0,
                true,
            )
            .unwrap(),
        )
        .unwrap();
    transaction.commit().unwrap();
    drop(transaction);
    for table in [
        "activities",
        "activity_inputs",
        "activity_outputs",
        "activity_input_dependency_snapshots",
        "activity_input_dependency_paths",
        "activity_input_fingerprint_snapshots",
        "activity_output_fingerprint_snapshots",
        "activity_output_keys",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "failed activity prefix in {table}");
    }
    let record = production.record_reader(2).unwrap().manifest().clone();
    assert_eq!(record.effect_count(), 1);
    assert_eq!(record.event_count(), 1);
    connection
        .execute_batch("DROP TRIGGER fail_output; DROP TRIGGER fail_delete;")
        .unwrap();
    let mut transaction = production.begin_transaction().unwrap();
    transaction.create_activity(&activity).unwrap();
    transaction.commit().unwrap();
    drop(transaction);
    assert_eq!(production.activities().unwrap().len(), 1);
    assert_eq!(production.ancestors(output_id).unwrap(), [source_id]);
}
