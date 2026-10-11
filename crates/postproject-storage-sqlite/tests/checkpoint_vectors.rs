//! Fixed Python-authored checkpoint bytes create an ordinary readable mirror.

use std::path::Path;

use postproject_core::ErrorKind;
use postproject_protocol::{
    CheckpointChunk, CheckpointManifest, Document, Limits, Position, StoreRole,
};
use postproject_storage_sqlite::{CheckpointLimits, SqliteProduction};

#[test]
fn independent_checkpoints_import_without_regenerating_source_facts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/exchange");
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/exchange/checkpoint-vectors.json"
    ))
    .unwrap();
    for vector in vectors.as_array().unwrap() {
        let snapshot: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(vector["file"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let document = |value: &serde_json::Value| {
            Document::parse(&serde_json::to_vec(value).unwrap(), Limits::default()).unwrap()
        };
        let manifest_document = document(&snapshot["manifest"]);
        assert_eq!(
            manifest_document.canonical_bytes().unwrap(),
            vector["canonical_manifest"].as_str().unwrap().as_bytes()
        );
        let manifest = CheckpointManifest::from_document(&manifest_document).unwrap();
        assert_eq!(manifest.document().unwrap(), manifest_document);
        let anchor = Position::anchor(manifest.floor().decision_base()).unwrap();
        assert_eq!(anchor, manifest.floor());
        assert_eq!(anchor.digest().to_string(), vector["anchor_digest"]);
        let chunks: Vec<_> = snapshot["chunks"]
            .as_array()
            .unwrap()
            .iter()
            .zip(vector["canonical_chunks"].as_array().unwrap())
            .map(|(value, expected)| {
                let document = document(value);
                assert_eq!(
                    document.canonical_bytes().unwrap(),
                    expected.as_str().unwrap().as_bytes()
                );
                let chunk = CheckpointChunk::from_document(&document).unwrap();
                assert_eq!(chunk.document().unwrap(), document);
                chunk
            })
            .collect();
        assert_eq!(chunks.len(), snapshot["chunks"].as_array().unwrap().len());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mirror.pproj");
        assert!(
            SqliteProduction::import_checkpoint(
                &path,
                &manifest,
                chunks[..chunks.len() - 1].iter().cloned().map(Ok),
                CheckpointLimits::default(),
            )
            .is_err()
        );
        assert!(!path.exists());
        let mirror = SqliteProduction::import_checkpoint(
            &path,
            &manifest,
            chunks.into_iter().map(Ok),
            CheckpointLimits::default(),
        )
        .unwrap();
        assert_eq!(mirror.exchange_role(), StoreRole::PassiveMirror);
        assert_eq!(mirror.exchange_head().unwrap(), manifest.head());
        assert_eq!(
            mirror.production().id(),
            manifest.head().scope().production()
        );
        assert_eq!(mirror.production().created_at().as_unix_micros(), i64::MIN);
        assert_eq!(mirror.production().display_name(), Some("Exact 同 😀\n"));
        compare_exported_bodies(&mirror, &snapshot);
        if vector["file"] == "checkpoints/archives.json" {
            check_archive_boundary(&mirror);
        }
        assert_eq!(
            mirror.changes_since(0, 10).unwrap().len() as u64,
            manifest.head().sequence()
        );
        if manifest.head().sequence() == 1 {
            let asset = mirror.assets().unwrap().pop().unwrap();
            assert_eq!(
                asset.id().to_string(),
                "00000000-0000-4000-8000-000000000005"
            );
            assert_eq!(asset.display_name(), Some("同 😀"));
            assert_eq!(mirror.representations(asset.id()).unwrap().len(), 2);
            let reader = mirror.record_reader(1).unwrap();
            assert_eq!(reader.manifest().event_count(), 9);
            assert_eq!(reader.manifest().effect_count(), 2);
        }
        drop(mirror);
        let mut reopened = SqliteProduction::open(&path).unwrap();
        assert_eq!(reopened.exchange_floor().unwrap(), manifest.floor());
        assert!(
            matches!(reopened.begin_transaction(), Err(error) if error.kind() == ErrorKind::Unsupported)
        );
    }
}

fn check_archive_boundary(mirror: &SqliteProduction) {
    use postproject_protocol::FailureKind;
    use postproject_storage_sqlite::ExchangeError;

    assert_eq!(mirror.exchange_floor().unwrap().sequence(), 1);
    assert!(
        matches!(mirror.record_reader(1).err(), Some(ExchangeError::Protocol(error))
        if error.kind() == FailureKind::HistoryGap)
    );
    assert_eq!(
        mirror.record_reader(2).unwrap().manifest().predecessor(),
        mirror.exchange_floor().unwrap()
    );
}

fn compare_exported_bodies(mirror: &SqliteProduction, snapshot: &serde_json::Value) {
    let mut bodies = std::collections::BTreeMap::<_, Vec<u8>>::new();
    mirror
        .export_checkpoint(|chunk| {
            bodies
                .entry(chunk.section().as_str())
                .or_default()
                .extend_from_slice(chunk.payload());
            Ok(())
        })
        .unwrap();
    for (section, payload) in bodies {
        let expected = snapshot["bodies"][section].as_array().unwrap();
        let mut frames = vec![];
        for value in expected {
            let body = Document::parse(&serde_json::to_vec(value).unwrap(), Limits::default())
                .unwrap()
                .canonical_bytes()
                .unwrap();
            frames.extend_from_slice(&(body.len() as u64).to_be_bytes());
            frames.extend(body);
        }
        assert!(
            payload == frames,
            "{section} body changed after import/export"
        );
    }
}
