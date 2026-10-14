//! Produced-output tasks preserve complete effects and roll back staged prefixes.

use postproject_core::{
    ActivityKind, ActivityRole, AssetId, ErrorKind, MetadataProperty, MetadataValue, ObjectRef,
    PropertyId, RepresentationId, RepresentationKind, ToolIdentity, VocabularyId,
};
use postproject_media::{OutputRecipe, OutputSource, prepare_outputs};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

fn recipe() -> OutputRecipe {
    OutputRecipe::new(
        ActivityKind::new("example:render").unwrap(),
        std::iter::empty::<RepresentationId>(),
    )
    .unwrap()
}

#[test]
fn zero_input_multiple_outputs_keep_actual_parameters_and_replay_complete_effects() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("render.dat");
    let second = directory.path().join("thumbnail.dat");
    std::fs::write(&first, b"actual generated render").unwrap();
    std::fs::write(&second, b"actual generated thumbnail").unwrap();
    let mut outputs = [
        OutputSource::new(None, RepresentationKind::Original, &first),
        OutputSource::new(None, RepresentationKind::Original, &second),
    ];
    outputs[1].role = Some(ActivityRole::new("example:thumbnail").unwrap());
    let property = MetadataProperty::new(
        VocabularyId::new("example:recipe").unwrap(),
        PropertyId::new("setting").unwrap(),
    );
    let mut recipe = recipe();
    recipe.tool =
        Some(ToolIdentity::new("Actual fixture generator", Some("1.0".into()), None).unwrap());
    recipe.parameters = vec![
        (property.clone(), MetadataValue::i64(7)),
        (property.clone(), MetadataValue::i64(8)),
    ];
    let prepared = prepare_outputs(&outputs, recipe).unwrap();
    let identities = prepared.identities();
    let activity = prepared.activity().id();
    let mut store = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let mut edit = prepared.stage(store.begin_transaction().unwrap()).unwrap();
    assert!(edit.commit().unwrap().revision().is_some());
    drop(edit);
    let produced = store.activities().unwrap();
    assert_eq!(produced.len(), 1);
    assert_eq!(produced[0].inputs(), []);
    assert_eq!(
        produced[0].tool().unwrap().name(),
        "Actual fixture generator"
    );
    assert_eq!(produced[0].outputs().len(), 2);
    assert!(
        produced[0]
            .outputs()
            .iter()
            .all(|edge| edge.snapshot().is_some())
    );
    let mut values: Vec<_> = store
        .metadata_values(ObjectRef::Activity(activity), &property)
        .unwrap()
        .iter()
        .map(|value| value.as_i64().unwrap())
        .collect();
    values.sort_unstable();
    assert_eq!(values, [7, 8]);
    for identity in &identities {
        assert_eq!(
            store
                .representation(identity.representation_id)
                .unwrap()
                .asset_id(),
            identity.asset_id
        );
        assert!(
            store
                .dependency_set(identity.representation_id)
                .unwrap()
                .is_none()
        );
    }
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        store.production(),
        store.exchange_floor().unwrap(),
    )
    .unwrap();
    let mut reader = store.record_reader(1).unwrap();
    let manifest = reader.manifest().clone();
    mirror
        .apply_record(
            &manifest,
            std::iter::from_fn(|| reader.next_chunk().transpose()),
            ReplayLimits::default(),
        )
        .unwrap();
    assert_eq!(mirror.activities().unwrap(), produced);
    assert_eq!(
        mirror.metadata(ObjectRef::Activity(activity)).unwrap(),
        store.metadata(ObjectRef::Activity(activity)).unwrap()
    );
    for identity in identities {
        assert_eq!(
            mirror.representation(identity.representation_id).unwrap(),
            store.representation(identity.representation_id).unwrap()
        );
    }
}

#[test]
fn late_staging_failure_discards_the_first_output_and_releases_the_edit() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("render.dat");
    std::fs::write(&file, b"already produced").unwrap();
    let outputs = [
        OutputSource::new(None, RepresentationKind::Original, &file),
        OutputSource::new(Some(AssetId::new()), RepresentationKind::Derived, &file),
    ];
    let prepared = prepare_outputs(&outputs, recipe()).unwrap();
    let identities = prepared.identities();
    let mut store =
        SqliteProduction::create(directory.path().join("rollback.pproj"), None).unwrap();
    assert!(prepared.stage(store.begin_transaction().unwrap()).is_err());
    assert_eq!(store.assets().unwrap(), [] as [postproject_core::Asset; 0]);
    assert_eq!(
        store.activities().unwrap(),
        [] as [postproject_core::Activity; 0]
    );
    assert!(store.latest_revision().unwrap().is_none());
    assert!(
        store
            .representation(identities[0].representation_id)
            .is_err()
    );
    store.begin_transaction().unwrap().rollback().unwrap();
}

#[test]
fn invalid_recipe_is_rejected_before_attempting_media_io() {
    let id = RepresentationId::new();
    let mut invalid = recipe();
    invalid.inputs = vec![id.into(), id.into()];
    let output = OutputSource::new(
        None,
        RepresentationKind::Original,
        std::path::Path::new("/nonexistent/postproject-output-task-file"),
    );
    assert_eq!(
        prepare_outputs(&[output], invalid).unwrap_err().kind(),
        ErrorKind::InvalidArgument
    );
}
