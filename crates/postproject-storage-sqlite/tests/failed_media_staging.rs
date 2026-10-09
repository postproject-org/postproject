//! A handled aggregate error must not contaminate a later successful commit.

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, Representation, RepresentationImport,
    RepresentationKind, RevisionEventKind, VocabularyId,
};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;

fn handled_locator_failure(existing_asset: bool) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("production.pproj");
    let media = directory.path().join("clip.mov");
    std::fs::write(&media, b"test media").unwrap();
    let original = prepare_original_media(&media, None, None).unwrap();
    let candidate = prepare_original_media(&media, None, None).unwrap();
    let representation = RepresentationImport::new(
        Representation::new(
            candidate.representation().id(),
            original.asset().id(),
            RepresentationKind::Proxy,
            candidate.representation().content_structure().clone(),
            candidate.representation().fingerprints().to_vec(),
        ),
        candidate.resources().to_vec(),
        candidate.locators().to_vec(),
    )
    .unwrap();
    let mut production = SqliteProduction::create(&path, None).unwrap();
    if existing_asset {
        let mut edit = production.begin_transaction().unwrap();
        edit.import_original(&original).unwrap();
        edit.commit().unwrap();
    }
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_locator BEFORE INSERT ON locators
         BEGIN SELECT RAISE(ABORT, 'injected locator failure'); END;",
        )
        .unwrap();
    let target = ObjectRef::Production(production.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:media:failure").unwrap(),
        PropertyId::new("retained").unwrap(),
    );
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(1))
        .unwrap();
    if existing_asset {
        assert!(edit.add_representation(&representation).is_err());
    } else {
        assert!(edit.import_original(&candidate).is_err());
    }
    edit.add_metadata_value(target, &property, &MetadataValue::i64(2))
        .unwrap();
    let revision = edit.commit().unwrap().revision().unwrap().id();
    drop(edit);
    assert_eq!(
        production.assets().unwrap().len(),
        usize::from(existing_asset)
    );
    if existing_asset {
        assert_eq!(
            production
                .representations(original.asset().id())
                .unwrap()
                .len(),
            1
        );
    }
    let resources: i64 = connection
        .query_row("SELECT COUNT(*) FROM resources", [], |row| row.get(0))
        .unwrap();
    assert_eq!(resources, i64::from(existing_asset));
    assert_eq!(
        production.metadata_values(target, &property).unwrap(),
        [MetadataValue::i64(1), MetadataValue::i64(2)]
    );
    let events = production.events_for_revision(revision).unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|event| matches!(
        event.kind(),
        RevisionEventKind::MetadataAddedOrReplaced { .. }
    )));

    // Retry the same assigned IDs after removing the injected failure; no
    // partially inserted asset, representation, resource or locator may remain.
    connection
        .execute_batch("DROP TRIGGER reject_locator")
        .unwrap();
    let mut edit = production.begin_transaction().unwrap();
    if existing_asset {
        edit.add_representation(&representation).unwrap();
    } else {
        edit.import_original(&candidate).unwrap();
    }
    edit.commit().unwrap();
}

#[test]
fn failed_original_import_leaves_no_rows_or_observations_when_caller_commits() {
    handled_locator_failure(false);
}

#[test]
fn failed_representation_addition_leaves_existing_asset_intact() {
    handled_locator_failure(true);
}
