//! Fixed Python-authored checkpoint bytes create an ordinary readable mirror.

use std::path::Path;

use postproject_core::ErrorKind;
use postproject_protocol::{
    CheckpointChunk, CheckpointManifest, Document, Limits, Position, StoreRole,
};
use postproject_storage_sqlite::{CheckpointLimits, SqliteProduction};

#[test]
fn independent_genesis_bytes_and_digests_import_without_regenerating_source_facts() {
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
        assert_eq!(manifest.floor(), manifest.head());
        let anchor = Position::anchor(manifest.head().decision_base()).unwrap();
        assert_eq!(anchor, manifest.head());
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
        assert!(mirror.changes_since(0, 10).unwrap().is_empty());
        drop(mirror);
        let mut reopened = SqliteProduction::open(&path).unwrap();
        assert_eq!(reopened.exchange_floor().unwrap(), manifest.floor());
        assert!(
            matches!(reopened.begin_transaction(), Err(error) if error.kind() == ErrorKind::Unsupported)
        );
    }
}
