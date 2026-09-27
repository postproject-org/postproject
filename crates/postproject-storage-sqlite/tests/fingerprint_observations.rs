//! Fingerprint observation history, events, and activity snapshot integration.

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, Asset, AssetId,
    ContentStructure, FileFacts, Locator, LocatorAvailability, LocatorId, OriginalMediaImport,
    Representation, RepresentationFingerprint, RepresentationId, RepresentationKind, Resource,
    ResourceFingerprint, ResourceId, RevisionEventKind, Timestamp,
};
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;
use tempfile::tempdir;

fn media(label: u8, kind: RepresentationKind) -> OriginalMediaImport {
    let asset_id = AssetId::from_bytes([label; 16]);
    let representation_id = RepresentationId::from_bytes([label; 16]);
    let resource_id = ResourceId::from_bytes([label; 16]);
    OriginalMediaImport::new(
        Asset::new(
            asset_id,
            Timestamp::from_unix_micros(i64::from(label)),
            None,
            None,
        ),
        Representation::new(
            representation_id,
            asset_id,
            kind,
            ContentStructure::single_resource(resource_id),
            vec![
                RepresentationFingerprint::new("aggregate", 1, vec![label])
                    .expect("valid representation fingerprint"),
            ],
        ),
        vec![Resource::new(
            resource_id,
            vec![
                ResourceFingerprint::new("content", 1, vec![label])
                    .expect("valid resource fingerprint"),
            ],
            None,
        )],
        vec![
            Locator::new(
                LocatorId::from_bytes([label; 16]),
                resource_id,
                format!("file:///media/{label}.mov"),
                None,
                LocatorAvailability::Online,
            )
            .expect("valid locator"),
        ],
    )
    .expect("valid media")
}

#[test]
fn observations_are_journaled_and_activity_edges_snapshot_storage_state() {
    let directory = tempdir().expect("create directory");
    let path = directory.path().join("observations.pproj");
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    let source = media(1, RepresentationKind::Original);
    let output = media(2, RepresentationKind::Original);
    let activity = Activity::new(
        ActivityId::from_bytes([3; 16]),
        ActivityKind::new("org.postproject:generate-proxy").expect("valid kind"),
        vec![ActivityInput::new(source.representation().id(), None)],
        vec![ActivityOutput::new(output.representation().id(), None)],
    )
    .expect("valid activity");
    {
        let mut transaction = production.begin_transaction().expect("begin import");
        transaction.import_original(&source).expect("import source");
        transaction.import_original(&output).expect("import output");
        transaction
            .create_activity(&activity)
            .expect("create activity");
        transaction.commit().expect("commit import");
    }

    let stored_activity = production.activities().expect("load activity").remove(0);
    let input_snapshot = stored_activity.inputs()[0]
        .snapshot()
        .expect("new input has snapshot");
    assert_eq!(input_snapshot.revision_sequence(), 1);
    assert_eq!(input_snapshot.fingerprints()[0].value(), [1]);
    assert_eq!(
        input_snapshot.fingerprints()[0].observed_revision_sequence(),
        Some(1)
    );
    let output_snapshot = stored_activity.outputs()[0]
        .snapshot()
        .expect("new output has snapshot");
    assert_eq!(output_snapshot.fingerprints()[0].value(), [2]);
    assert_eq!(
        output_snapshot.fingerprints()[0].observed_revision_sequence(),
        Some(1)
    );

    let unchanged = ResourceFingerprint::new("content", 1, vec![1]).expect("valid fingerprint");
    {
        let mut transaction = production.begin_transaction().expect("begin no-op");
        assert!(
            !transaction
                .record_resource_fingerprint(ResourceId::from_bytes([1; 16]), &unchanged)
                .expect("record identical fingerprint")
        );
        transaction.commit().expect("commit no-op");
    }
    assert_eq!(
        production
            .latest_revision()
            .expect("load revision")
            .expect("revision")
            .sequence(),
        1
    );

    let changed = ResourceFingerprint::new("content", 1, vec![9]).expect("valid fingerprint");
    {
        let mut transaction = production.begin_transaction().expect("begin observation");
        assert!(
            transaction
                .record_resource_fingerprint(ResourceId::from_bytes([1; 16]), &changed)
                .expect("record changed fingerprint")
        );
        transaction.commit().expect("commit observation");
    }
    let revision = production
        .latest_revision()
        .expect("load revision")
        .expect("revision");
    assert_eq!(revision.sequence(), 2);
    assert!(matches!(
        production.events_for_revision(revision.id()).expect("events")[0].kind(),
        RevisionEventKind::ResourceFingerprintObserved { resource_id, algorithm, version }
            if *resource_id == ResourceId::from_bytes([1; 16])
                && algorithm == "content" && *version == 1
    ));

    drop(production);
    let connection = Connection::open(&path).expect("open raw database");
    let history: (Vec<u8>, i64) = connection
        .query_row(
            "SELECT value, superseded_revision_sequence
             FROM resource_fingerprint_history",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load history");
    assert_eq!(history, (vec![1], 2));
    let dirty_count: u32 = connection
        .query_row(
            "SELECT count(*) FROM representation_fingerprint_recomputations",
            [],
            |row| row.get(0),
        )
        .expect("count recomputation markers");
    assert_eq!(dirty_count, 1);
}

#[test]
fn file_facts_are_recorded_with_an_observation() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("facts.pproj");
    let import = media(7, RepresentationKind::Original);
    let resource_id = import.resources()[0].id();
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    {
        let mut transaction = production.begin_transaction().expect("begin import");
        transaction.import_original(&import).expect("stage import");
        transaction.commit().expect("commit import");
    }
    let facts = FileFacts::new(1_234, Some(Timestamp::from_unix_micros(5)));
    {
        let mut transaction = production.begin_transaction().expect("begin observation");
        assert!(
            transaction
                .record_resource_file_facts(resource_id, facts)
                .expect("record facts")
        );
        assert!(
            !transaction
                .record_resource_file_facts(resource_id, facts)
                .expect("record identical facts")
        );
        assert!(
            transaction
                .record_resource_file_facts(ResourceId::from_bytes([9; 16]), facts)
                .is_err()
        );
        transaction.commit().expect("commit observation");
    }
    let stored = production
        .resources(RepresentationId::from_bytes([7; 16]))
        .expect("load resources");
    assert_eq!(stored[0].file_facts(), Some(facts));
}
