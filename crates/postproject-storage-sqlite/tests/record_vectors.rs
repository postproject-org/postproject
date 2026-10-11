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
        } else if vector["file"] == "streams/knowledge.json" {
            check_knowledge(&mirror);
        } else if vector["file"] == "streams/provenance.json" {
            check_provenance(&mirror);
        } else if vector["file"] == "streams/job-completion.json" {
            check_jobs(&mirror);
        }
        assert_eq!(
            mirror.begin_transaction().err().unwrap().kind(),
            ErrorKind::Unsupported
        );
    }
}

fn check_knowledge(mirror: &SqliteProduction) {
    let representation = "00000000-0000-4000-8000-000000000006".parse().unwrap();
    let current = mirror.representation(representation).unwrap();
    assert_eq!(current.fingerprints().len(), 1);
    let fingerprint = &current.fingerprints()[0];
    assert_eq!(fingerprint.algorithm(), "vendor-Exact");
    assert_eq!(fingerprint.version(), u16::MAX);
    assert_eq!(fingerprint.value(), b"\0B\xfe");
    let resources = mirror.resources(representation).unwrap();
    assert_eq!(resources[0].fingerprints().len(), 1);
    assert_eq!(resources[0].fingerprints()[0].value(), b"\0C\xfd");
    let dependencies = mirror.dependency_set(representation).unwrap().unwrap();
    assert_eq!(dependencies.recorded_at_revision(), 3);
    assert_eq!(
        dependencies.status(),
        postproject_core::DependencySetStatus::Current
    );
    assert_eq!(dependencies.dependencies(), []);
}

fn check_provenance(mirror: &SqliteProduction) {
    let activity = mirror.activities().unwrap().pop().unwrap();
    assert_eq!(activity.kind().as_str(), "vendor:Render");
    assert_eq!(activity.started_at().unwrap().as_unix_micros(), i64::MIN);
    assert_eq!(activity.finished_at().unwrap().as_unix_micros(), i64::MAX);
    assert_eq!(activity.tool().unwrap().name(), "Exact 同 😀");
    assert_eq!(activity.inputs().len(), 1);
    assert_eq!(activity.outputs().len(), 1);
    let captured = activity.inputs()[0].snapshot().unwrap();
    assert_eq!(captured.revision_sequence(), 4);
    assert_eq!(captured.fingerprints()[0].value(), b"\0B\xfe");
    assert_eq!(
        captured.fingerprints()[0].observed_revision_sequence(),
        Some(3)
    );
    let input = activity.inputs()[0].representation_id();
    assert_eq!(
        mirror.representation(input).unwrap().fingerprints()[0].value(),
        b"\0D\xfc"
    );
    assert_eq!(
        mirror
            .dependency_set(input)
            .unwrap()
            .unwrap()
            .dependencies(),
        []
    );
}

fn check_jobs(mirror: &SqliteProduction) {
    let job = |suffix: &str| {
        mirror
            .job(format!("00000000-0000-4000-8000-{suffix}").parse().unwrap())
            .unwrap()
    };
    assert!(matches!(
        job("000000000708").state(),
        postproject_core::JobState::Requested
    ));
    let claimed = job("000000000709");
    let postproject_core::JobState::Claimed(claim) = claimed.state() else {
        panic!("missing inert claim")
    };
    assert_eq!(claim.expires_at().as_unix_micros(), i64::MAX);
    assert_eq!(claim.tool().name(), "Exact 同 😀");
    let failed = job("00000000070a");
    let postproject_core::JobState::Failed(failure) = failed.state() else {
        panic!("missing terminal failure")
    };
    assert_eq!(failure.diagnostic(), "  vendor:Failure 同 😀\n  ");
    assert!(matches!(
        job("00000000070b").state(),
        postproject_core::JobState::Cancelled
    ));
    let succeeded = job("00000000070c");
    let postproject_core::JobState::Succeeded(completion) = succeeded.state() else {
        panic!("missing publication receipt")
    };
    assert_eq!(
        completion.representation_id().to_string(),
        "00000000-0000-4000-8000-000000000726"
    );
    let output = mirror
        .representation(completion.representation_id())
        .unwrap();
    assert_eq!(output.kind(), RepresentationKind::Derived);
    let activity = mirror
        .activities_producing(output.id())
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(activity.id(), completion.activity_id());
    assert_eq!(
        activity.inputs()[0].snapshot().unwrap().fingerprints()[0].observed_revision_sequence(),
        Some(4)
    );
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
