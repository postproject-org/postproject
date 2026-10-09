use postproject_core::{
    MediaRoot, MediaRootId, MetadataProperty, MetadataValue, ObjectRef, PropertyId,
    SemanticConflictKey, VocabularyId,
};

use crate::{ExchangeError, SqliteProduction};

#[test]
fn migrated_guard_audit_accepts_baseline_facts_but_post_floor_guards_need_authored_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let root = MediaRoot::new(MediaRootId::new(), "Essence", None, None, 1, true).unwrap();
    let property = MetadataProperty::new(
        VocabularyId::new("urn:exact").unwrap(),
        PropertyId::new("key").unwrap(),
    );
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.add_media_root(root.clone()).unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(1))
        .unwrap();
    let revision = edit.commit().unwrap().revision().unwrap().id();
    drop(edit);
    let events = source.events_for_revision(revision).unwrap();
    for floor in [0, 1] {
        let transaction = source.connection.transaction().unwrap();
        super::create(&transaction).unwrap();
        for event in &events {
            super::observed(&transaction, event, 1).unwrap();
        }
        if floor == 0 {
            assert!(
                matches!(super::finish(&transaction, floor), Err(ExchangeError::Protocol(error)) if error.kind() == postproject_protocol::FailureKind::Integrity)
            );
            super::recorded(&transaction, &SemanticConflictKey::MediaRoot(root.id()), 1).unwrap();
            assert!(super::finish(&transaction, floor).is_err());
            super::recorded(
                &transaction,
                &SemanticConflictKey::MetadataProperty {
                    target,
                    property: property.clone(),
                },
                1,
            )
            .unwrap();
        }
        super::finish(&transaction, floor).unwrap();
        let remaining: i64 = transaction
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name LIKE 'checkpoint_guard_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
        transaction.rollback().unwrap();
    }
    let transaction = source.connection.transaction().unwrap();
    super::create(&transaction).unwrap();
    for event in &events {
        super::observed(&transaction, event, 1).unwrap();
    }
    transaction
        .execute(
            "UPDATE conflict_versions SET last_changed_revision_sequence = 2",
            [],
        )
        .unwrap();
    assert!(
        matches!(super::finish(&transaction, 1), Err(ExchangeError::Protocol(error)) if error.kind() == postproject_protocol::FailureKind::Integrity)
    );
    transaction.rollback().unwrap();
}

#[test]
fn initial_locator_observations_do_not_invent_a_changed_semantic_guard() {
    use postproject_core::{LocatorId, ResourceId, RevisionEvent, RevisionEventKind};
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let property = MetadataProperty::new(
        VocabularyId::new("urn:exact").unwrap(),
        PropertyId::new("key").unwrap(),
    );
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(1))
        .unwrap();
    let revision = edit.commit().unwrap().revision().unwrap().id();
    drop(edit);
    let resource_id = ResourceId::new();
    let transaction = source.connection.transaction().unwrap();
    super::create(&transaction).unwrap();
    let created = RevisionEventKind::ResourceAdded { resource_id };
    crate::transaction::persist_revision_event(&transaction, revision, 1, &created).unwrap();
    let located = RevisionEventKind::LocatorAdded {
        resource_id,
        locator_id: LocatorId::new(),
    };
    let event = RevisionEvent::new(revision, 2, located);
    super::observed(&transaction, &event, 1).unwrap();
    let required: bool = transaction
        .query_row(
            "SELECT required FROM checkpoint_guard_observations",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!required);
    let retired = RevisionEvent::new(
        revision,
        3,
        RevisionEventKind::LocatorRetired {
            resource_id,
            locator_id: LocatorId::new(),
        },
    );
    super::observed(&transaction, &retired, 1).unwrap();
    let required: bool = transaction
        .query_row(
            "SELECT required FROM checkpoint_guard_observations",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(required);
    assert!(super::observed(&transaction, &retired, 0).is_err());
    transaction.rollback().unwrap();
}
