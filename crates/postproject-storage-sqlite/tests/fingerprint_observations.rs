//! Fingerprint observation history, events, and activity snapshot integration.

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, Asset, AssetId,
    ContentStructure, ErrorKind, FileFacts, Locator, LocatorAvailability, LocatorId,
    OriginalMediaImport, Representation, RepresentationFingerprint, RepresentationId,
    RepresentationKind, Resource, ResourceFingerprint, ResourceId, RevisionEventKind,
    SemanticConflictKey, Timestamp, TransactionState,
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
        let base = production.read_session().unwrap().decision_base();
        let mut transaction = production.begin_edit(base).expect("begin no-op");
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
        let base = production.read_session().unwrap().decision_base();
        let mut transaction = production.begin_edit(base).expect("begin observation");
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
        let base = production.read_session().unwrap().decision_base();
        let mut transaction = production.begin_edit(base).expect("begin observation");
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
        let receipt = transaction
            .commit_with_receipt()
            .expect("commit observation");
        let revision = receipt.revision().expect("facts-only revision");
        drop(transaction);
        assert_eq!(revision.sequence(), 2);
        let events = production.events_for_revision(revision.id()).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].kind(),
            RevisionEventKind::ResourceFileFactsObserved { resource_id: id }
                if *id == resource_id));
    }
    let stored = production
        .resources(RepresentationId::from_bytes([7; 16]))
        .expect("load resources");
    assert_eq!(stored[0].file_facts(), Some(facts));
    let base = production.read_session().unwrap().decision_base();
    let mut edit = production.begin_edit(base).unwrap();
    assert!(!edit.record_resource_file_facts(resource_id, facts).unwrap());
    assert!(edit.commit_with_receipt().unwrap().revision().is_none());
}

#[test]
fn explicit_observations_reject_missing_bases_without_staging_or_closing() {
    let directory = tempdir().unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("guard.pproj"), None).unwrap();
    let import = media(1, RepresentationKind::Original);
    let mut transaction = production.begin_transaction().unwrap();
    transaction.import_original(&import).unwrap();
    let resource_id = import.resources()[0].id();
    for error in [
        transaction
            .record_resource_file_facts(resource_id, FileFacts::new(123, None))
            .unwrap_err(),
        transaction
            .record_resource_fingerprint(resource_id, &import.resources()[0].fingerprints()[0])
            .unwrap_err(),
        transaction
            .record_representation_fingerprint(
                import.representation().id(),
                &import.representation().fingerprints()[0],
            )
            .unwrap_err(),
    ] {
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    }
    assert_eq!(transaction.state(), TransactionState::Open);
    transaction.commit().unwrap();
    drop(transaction);
    assert_eq!(
        production.resources(import.representation().id()).unwrap(),
        import.resources()
    );
    assert_eq!(production.latest_revision().unwrap().unwrap().sequence(), 1);
}

#[test]
fn stale_file_facts_roll_back_all_staged_observations_and_additive_work() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("stale.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let import = media(1, RepresentationKind::Original);
    let resource_id = import.resources()[0].id();
    {
        let mut transaction = production.begin_transaction().unwrap();
        transaction.import_original(&import).unwrap();
        transaction.commit().unwrap();
    }
    let base = production.read_session().unwrap().decision_base();
    let mut writer = SqliteProduction::open(&path).unwrap();
    let newer = FileFacts::new(42, Some(Timestamp::from_unix_micros(8)));
    let mut edit = writer.begin_edit(base).unwrap();
    edit.record_resource_file_facts(resource_id, newer).unwrap();
    let receipt = edit.commit_with_receipt().unwrap();
    let superseding = receipt.revision().unwrap();
    let unrelated = media(2, RepresentationKind::Original);
    let mut stale = production.begin_edit(base).unwrap();
    stale.import_original(&unrelated).unwrap();
    stale
        .record_resource_file_facts(resource_id, FileFacts::new(17, None))
        .unwrap();
    stale
        .record_resource_fingerprint(
            resource_id,
            &ResourceFingerprint::new("content", 1, vec![9]).unwrap(),
        )
        .unwrap();
    let error = stale.commit_with_receipt().unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Conflict);
    let detail = error.transaction_conflict_detail().unwrap();
    assert_eq!(
        detail.key(),
        &SemanticConflictKey::ResourceFileFacts(resource_id)
    );
    assert_eq!(detail.superseding_revision(), superseding.id());
    assert_eq!(stale.state(), TransactionState::RolledBack);
    assert!(stale.commit().is_err());
    drop(stale);
    let resources = production.resources(import.representation().id()).unwrap();
    assert_eq!(resources[0].file_facts(), Some(newer));
    assert_eq!(
        resources[0].fingerprints(),
        import.resources()[0].fingerprints()
    );
    assert!(production.asset(unrelated.asset().id()).is_err());
    assert_eq!(
        production.latest_revision().unwrap().unwrap().id(),
        superseding.id()
    );
}
