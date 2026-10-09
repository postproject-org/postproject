//! Distinct-file checkpoint bases and original contiguous suffix reconstruction.

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};
use postproject_protocol::{CheckpointChunk, CheckpointManifest, Limits, StoreRole};
use postproject_storage_sqlite::{CheckpointLimits, ReplayLimits, SqliteProduction};

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:unknown:checkpoint").unwrap(),
        PropertyId::new("Exact").unwrap(),
    )
}

fn export(source: &SqliteProduction) -> (CheckpointManifest, Vec<CheckpointChunk>) {
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    (manifest, chunks)
}

#[test]
fn two_independent_checkpoint_bases_converge_after_the_original_suffix() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(
        directory.path().join("authority.pproj"),
        Some("Exact 名".into()),
    )
    .unwrap();
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::u64(u64::MAX))
        .unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::u64(u64::MAX))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let (initial, chunks) = export(&source);
    let mut first = SqliteProduction::import_checkpoint(
        directory.path().join("first.pproj"),
        &initial,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(first.exchange_head().unwrap(), initial.head());
    assert_eq!(first.exchange_role(), StoreRole::PassiveMirror);
    assert_eq!(
        first.metadata_values(target, &property()).unwrap(),
        vec![MetadataValue::u64(u64::MAX), MetadataValue::u64(u64::MAX)]
    );
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.replace_metadata_values(target, &property(), &[MetadataValue::i64(-1)])
        .unwrap();
    edit.remove_metadata_property(target, &property()).unwrap();
    edit.add_metadata_value(
        target,
        &property(),
        &MetadataValue::bytes(vec![255; 2 * 1024 * 1024]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let (later, chunks) = export(&source);
    assert!(chunks.len() > 7);
    let second_path = directory.path().join("second.pproj");
    let second = SqliteProduction::import_checkpoint(
        &second_path,
        &later,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    let mut reader = source.record_reader(2).unwrap();
    let manifest = reader.manifest().clone();
    let mut suffix = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        suffix.push(chunk);
    }
    assert!(
        first
            .apply_record(
                &manifest,
                suffix.clone().into_iter().map(Ok),
                ReplayLimits::default()
            )
            .unwrap()
    );
    assert!(
        !first
            .apply_record(
                &manifest,
                suffix.into_iter().map(Ok),
                ReplayLimits::default()
            )
            .unwrap()
    );
    for mirror in [&first, &second] {
        assert_equivalent(&source, mirror, target);
    }
    assert!(first.begin_transaction().is_err());
    drop(second);
    let reopened = SqliteProduction::open(&second_path).unwrap();
    assert_eq!(reopened.exchange_head().unwrap(), later.head());
}

#[test]
fn failures_and_destination_races_never_publish_a_partial_store() {
    let directory = tempfile::tempdir().unwrap();
    let source = SqliteProduction::create(directory.path().join("authority.pproj"), None).unwrap();
    let (manifest, chunks) = export(&source);
    let destination = directory.path().join("mirror.pproj");
    for limits in [
        CheckpointLimits::new(1, 2 * 1024 * 1024, 10, Limits::default()).unwrap(),
        CheckpointLimits::new(1024 * 1024, 1, 10, Limits::default()).unwrap(),
        CheckpointLimits::new(1024 * 1024, 2 * 1024 * 1024, 1, Limits::default()).unwrap(),
    ] {
        assert!(
            SqliteProduction::import_checkpoint(
                &destination,
                &manifest,
                chunks.clone().into_iter().map(Ok),
                limits
            )
            .is_err()
        );
        assert!(!destination.exists());
    }
    assert!(
        SqliteProduction::import_checkpoint(
            &destination,
            &manifest,
            chunks[..1].iter().cloned().map(Ok),
            CheckpointLimits::default()
        )
        .is_err()
    );
    assert!(!destination.exists());
    let mut cancel = chunks
        .clone()
        .into_iter()
        .enumerate()
        .map(|(position, chunk)| {
            if position == 1 {
                Err(postproject_core::Error::new(
                    postproject_core::ErrorKind::Cancelled,
                    "cancelled",
                )
                .into())
            } else {
                Ok(chunk)
            }
        });
    assert!(
        SqliteProduction::import_checkpoint(
            &destination,
            &manifest,
            &mut cancel,
            CheckpointLimits::default()
        )
        .is_err()
    );
    assert!(!destination.exists());
    let competitor = b"existing user data";
    let race = chunks.into_iter().enumerate().map(|(position, chunk)| {
        if position == 0 {
            std::fs::write(&destination, competitor).unwrap();
        }
        Ok(chunk)
    });
    assert!(
        SqliteProduction::import_checkpoint(
            &destination,
            &manifest,
            race,
            CheckpointLimits::default()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&destination).unwrap(), competitor);
    assert_eq!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .filter(|entry| entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".postproject-import-"))
            .count(),
        0
    );
}

fn assert_equivalent(source: &SqliteProduction, mirror: &SqliteProduction, target: ObjectRef) {
    assert_eq!(mirror.production(), source.production());
    assert_eq!(
        mirror.exchange_scope().unwrap(),
        source.exchange_scope().unwrap()
    );
    assert_eq!(
        mirror.exchange_floor().unwrap(),
        source.exchange_floor().unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert_eq!(
        mirror.metadata_values(target, &property()).unwrap(),
        source.metadata_values(target, &property()).unwrap()
    );
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    let revision = mirror.latest_revision().unwrap().unwrap().id();
    assert_eq!(
        mirror.events_for_revision(revision).unwrap(),
        source.events_for_revision(revision).unwrap()
    );
    assert_eq!(
        mirror.record_reader(1).unwrap().manifest(),
        source.record_reader(1).unwrap().manifest()
    );
}
