//! Actual process exits around private commit, sealing and final publication.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};

use crate::{CheckpointLimits, SqliteProduction};

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:checkpoint:crash").unwrap(),
        PropertyId::new("Exact").unwrap(),
    )
}

fn crash(directory: &Path, phase: &str) -> PathBuf {
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "exchange::checkpoint::import::tests::worker",
            "--nocapture",
        ])
        .env("POSTPROJECT_CHECKPOINT_TEST_DIRECTORY", directory)
        .env("POSTPROJECT_CHECKPOINT_TEST_CRASH", phase)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(77));
    assert!(!directory.join("mirror.pproj").exists());
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".postproject-import-")
        })
        .unwrap()
}

#[test]
fn recover_actual_process_exits_before_and_after_private_completion() {
    for phase in ["during-stream", "before-seal", "after-seal"] {
        let directory = tempfile::tempdir().unwrap();
        let stage = crash(directory.path(), phase);
        let destination = directory.path().join("mirror.pproj");
        let source = SqliteProduction::open(directory.path().join("source.pproj")).unwrap();
        let recovered = SqliteProduction::recover_checkpoint_import(
            &stage,
            &destination,
            CheckpointLimits::default(),
        )
        .unwrap();
        assert!(!stage.exists());
        let mirror = if let Some(mirror) = recovered {
            assert_eq!(phase, "after-seal");
            mirror
        } else {
            assert_ne!(phase, "after-seal");
            assert!(!destination.exists());
            let mut chunks = Vec::new();
            let manifest = source
                .export_checkpoint(|chunk| {
                    chunks.push(chunk);
                    Ok(())
                })
                .unwrap();
            SqliteProduction::import_checkpoint(
                &destination,
                &manifest,
                chunks.into_iter().map(Ok),
                CheckpointLimits::default(),
            )
            .unwrap()
        };
        assert_eq!(
            mirror.exchange_head().unwrap(),
            source.exchange_head().unwrap()
        );
        assert_eq!(
            mirror
                .metadata_values(ObjectRef::Production(source.production().id()), &property())
                .unwrap(),
            vec![MetadataValue::u64(u64::MAX)]
        );
    }
}

#[test]
fn recovery_preserves_unrelated_files_existing_destinations_and_altered_stages() {
    let directory = tempfile::tempdir().unwrap();
    let stage = crash(directory.path(), "after-seal");
    let destination = directory.path().join("mirror.pproj");
    let unrelated = stage.join("user-note");
    std::fs::write(&unrelated, b"keep").unwrap();
    assert!(
        SqliteProduction::recover_checkpoint_import(
            &stage,
            &destination,
            CheckpointLimits::default()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&unrelated).unwrap(), b"keep");
    std::fs::remove_file(unrelated).unwrap();
    std::fs::write(&destination, b"existing").unwrap();
    assert!(
        SqliteProduction::recover_checkpoint_import(
            &stage,
            &destination,
            CheckpointLimits::default()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
    assert!(stage.exists());
    std::fs::remove_file(&destination).unwrap();
    let database = std::fs::read_dir(&stage)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "pproj")
        })
        .unwrap();
    let original = std::fs::read(&database).unwrap();
    let mut changed = original.clone();
    changed[0] ^= 1;
    std::fs::write(&database, changed).unwrap();
    assert!(
        SqliteProduction::recover_checkpoint_import(
            &stage,
            &destination,
            CheckpointLimits::default()
        )
        .is_err()
    );
    assert!(stage.exists());
    assert!(!destination.exists());
    std::fs::write(&database, original).unwrap();
    assert!(
        SqliteProduction::recover_checkpoint_import(
            &stage,
            &destination,
            CheckpointLimits::default()
        )
        .unwrap()
        .is_some()
    );
    assert!(!stage.exists());
}

#[test]
fn worker() {
    let Some(directory) = std::env::var_os("POSTPROJECT_CHECKPOINT_TEST_DIRECTORY") else {
        return;
    };
    let directory = PathBuf::from(directory);
    let mut source = SqliteProduction::create(directory.join("source.pproj"), None).unwrap();
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::u64(u64::MAX))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let stream = chunks.into_iter().enumerate().map(|(position, chunk)| {
        if position == 1
            && std::env::var("POSTPROJECT_CHECKPOINT_TEST_CRASH").unwrap() == "during-stream"
        {
            std::process::exit(77);
        }
        Ok(chunk)
    });
    SqliteProduction::import_checkpoint(
        directory.join("mirror.pproj"),
        &manifest,
        stream,
        CheckpointLimits::default(),
    )
    .unwrap();
    panic!("crash worker unexpectedly reached publication");
}
