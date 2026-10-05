//! Additive metadata writes and stale destructive decisions.

use postproject_core::{
    ErrorKind, MetadataProperty, MetadataValue, ObjectRef, PropertyId, SemanticConflictKey,
    TransactionState, VocabularyId,
};
use postproject_storage_sqlite::SqliteProduction;

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("com.example.editor").unwrap(),
        PropertyId::new("keywords").unwrap(),
    )
}

#[test]
fn appends_merge_but_stale_replacement_and_removal_roll_back() {
    let directory = tempfile::tempdir().unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("metadata.pproj"), None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let property = property();
    let base = production.read_session().unwrap().decision_base();
    let values = [
        MetadataValue::string("first").unwrap(),
        MetadataValue::string("second").unwrap(),
    ];
    for value in &values {
        let mut append = production.begin_edit(base).unwrap();
        append.add_metadata_value(target, &property, value).unwrap();
        append.commit_with_receipt().unwrap();
    }
    let superseding = production.latest_revision().unwrap().unwrap();
    for remove in [false, true] {
        let mut edit = production.begin_edit(base).unwrap();
        if remove {
            edit.remove_metadata_property(target, &property).unwrap();
        } else {
            edit.replace_metadata_values(target, &property, &[])
                .unwrap();
        }
        let error = edit
            .commit_with_receipt()
            .expect_err("append invalidated decision");
        assert_eq!(error.kind(), ErrorKind::Conflict);
        assert_eq!(edit.state(), TransactionState::RolledBack);
        let conflict = error.transaction_conflict_detail().unwrap();
        assert_eq!(
            conflict.key(),
            &SemanticConflictKey::MetadataProperty {
                target,
                property: property.clone()
            }
        );
        assert_eq!(conflict.base_revision(), None);
        assert_eq!(conflict.superseding_revision(), superseding.id());
        drop(edit);
        assert_eq!(
            production.metadata_values(target, &property).unwrap(),
            values
        );
        assert_eq!(
            production.latest_revision().unwrap().unwrap().id(),
            superseding.id()
        );
    }
}

#[test]
fn stale_append_merges_after_a_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("metadata.pproj"), None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let property = property();
    let base = production.read_session().unwrap().decision_base();
    let replacement = MetadataValue::string("replacement").unwrap();
    {
        let mut edit = production.begin_edit(base).unwrap();
        edit.replace_metadata_values(target, &property, std::slice::from_ref(&replacement))
            .unwrap();
        edit.commit().unwrap();
    }
    let appended = MetadataValue::string("append").unwrap();
    {
        let mut edit = production.begin_edit(base).unwrap();
        edit.add_metadata_value(target, &property, &appended)
            .unwrap();
        edit.commit()
            .expect("append is independent of older property contents");
    }
    assert_eq!(
        production.metadata_values(target, &property).unwrap(),
        [replacement, appended]
    );
}
