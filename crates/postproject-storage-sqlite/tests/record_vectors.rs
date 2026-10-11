//! Fixed independently authored records apply without recapturing authority state.

use std::path::Path;

use postproject_core::{ErrorKind, Production, RepresentationKind, Timestamp};
use postproject_protocol::{Document, Limits, Position, RecordChunk, RecordManifest};
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
        let mut mirror = SqliteProduction::create_genesis_mirror(
            &path,
            &source,
            Position::from_document(&document(&fixture["floor"])).unwrap(),
        )
        .unwrap();
        if let Some(prefix) = fixture["prefix"].as_array() {
            for file in prefix {
                let prior: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(root.join(file.as_str().unwrap())).unwrap(),
                )
                .unwrap();
                apply_prefix(&mut mirror, &prior);
            }
        }
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
            check_media(&mirror);
        } else if vector["file"] == "streams/access.json" {
            check_access(&mirror);
        }
        assert_eq!(
            mirror.begin_transaction().err().unwrap().kind(),
            ErrorKind::Unsupported
        );
    }
}

fn apply_prefix(mirror: &mut SqliteProduction, fixture: &serde_json::Value) {
    let manifest = RecordManifest::from_document(&document(&fixture["manifest"])).unwrap();
    assert_eq!(mirror.exchange_head().unwrap(), manifest.predecessor());
    let chunks = fixture["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| Ok(RecordChunk::from_document(&document(value)).unwrap()));
    assert!(
        mirror
            .apply_record(&manifest, chunks, ReplayLimits::default())
            .unwrap()
    );
}

fn check_media(mirror: &SqliteProduction) {
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

fn check_access(mirror: &SqliteProduction) {
    assert_eq!(mirror.media_roots().unwrap(), []);
    let asset = mirror.assets().unwrap().pop().unwrap();
    let identifiers = mirror
        .external_identifiers(postproject_core::ObjectRef::Asset(asset.id()))
        .unwrap();
    assert_eq!(identifiers.len(), 1);
    assert_eq!(identifiers[0].scheme().as_str(), "urn:vendor:EXACT");
    assert_eq!(identifiers[0].value(), "  同/😀  ");
    assert_eq!(identifiers[0].qualifier(), Some("vendor:Keep"));
    let representation = "00000000-0000-4000-8000-000000000006".parse().unwrap();
    let resources = mirror.resources(representation).unwrap();
    let facts = resources[0].file_facts().unwrap();
    assert_eq!(facts.size_bytes(), i64::MAX as u64);
    assert_eq!(
        facts.modified_at(),
        Some(Timestamp::from_unix_micros(i64::MIN))
    );
    let locators = mirror.locators(resources[0].id()).unwrap();
    assert_eq!(locators.len(), 1);
    assert_eq!(locators[0].uri(), "file:///missing/New%2fCase.mov");
    assert_eq!(locators[0].media_root(), None);
    assert_eq!(
        locators[0].last_seen(),
        Some(Timestamp::from_unix_micros(i64::MAX))
    );
}
