//! Fixed independently authored records apply without recapturing authority state.

use std::path::Path;

use postproject_core::{ErrorKind, Production, RepresentationKind, Timestamp};
use postproject_protocol::{Document, Limits, RecordChunk, RecordManifest};
use postproject_storage_sqlite::{CURRENT_SCHEMA_VERSION, ReplayLimits, SqliteProduction};

fn document(value: &serde_json::Value) -> Document {
    Document::parse(value.to_string().as_bytes(), Limits::default()).unwrap()
}

#[test]
fn independent_records_preserve_original_media_identities_and_public_observations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/exchange");
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/exchange/stream-vectors.json"
    ))
    .unwrap();
    for vector in vectors.as_array().unwrap() {
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(vector["file"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let manifest = RecordManifest::from_document(&document(&fixture["manifest"])).unwrap();
        let source = Production::new(
            manifest.predecessor().scope().production(),
            CURRENT_SCHEMA_VERSION,
            Timestamp::from_unix_micros(i64::MIN),
            Some("Exact 同".into()),
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mirror.pproj");
        let mut mirror =
            SqliteProduction::create_genesis_mirror(&path, &source, manifest.predecessor())
                .unwrap();
        let chunks: Vec<_> = fixture["chunks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| RecordChunk::from_document(&document(value)).unwrap())
            .collect();
        assert!(
            mirror
                .apply_record(
                    &manifest,
                    chunks.iter().cloned().map(Ok),
                    ReplayLimits::default()
                )
                .unwrap()
        );
        assert!(
            !mirror
                .apply_record(
                    &manifest,
                    chunks.into_iter().map(Ok),
                    ReplayLimits::default()
                )
                .unwrap()
        );
        drop(mirror);
        let mut mirror = SqliteProduction::open(path).unwrap();
        assert_eq!(mirror.exchange_head().unwrap(), manifest.head().unwrap());
        let events = mirror
            .events_for_revision(manifest.revision().id())
            .unwrap();
        assert_eq!(events.len() as u64, manifest.event_count());
        for event in events {
            assert_eq!(event.revision_id(), manifest.revision().id());
        }
        if vector["file"] == "streams/media.json" {
            let asset = mirror.assets().unwrap().pop().unwrap();
            assert_eq!(
                asset.id().to_string(),
                "00000000-0000-4000-8000-000000000005"
            );
            assert_eq!(asset.created_at(), Timestamp::from_unix_micros(i64::MIN));
            assert_eq!(asset.display_name(), Some("同 😀"));
            let representations = mirror.representations(asset.id()).unwrap();
            assert_eq!(representations.len(), 2);
            assert!(
                representations
                    .iter()
                    .any(|item| item.kind() == RepresentationKind::Original)
            );
            assert!(
                representations
                    .iter()
                    .any(|item| item.kind() == RepresentationKind::Proxy)
            );
            for representation in representations {
                let resources = mirror.resources(representation.id()).unwrap();
                assert_eq!(resources.len(), 1);
                assert_eq!(
                    mirror.locators(resources[0].id()).unwrap()[0].uri(),
                    "file:///missing/Exact.mov"
                );
            }
        }
        assert_eq!(
            mirror.begin_transaction().err().unwrap().kind(),
            ErrorKind::Unsupported
        );
    }
}
