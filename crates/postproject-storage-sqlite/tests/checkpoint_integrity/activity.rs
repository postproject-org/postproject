use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, Asset, AssetId,
    ContentStructure, Locator, LocatorAvailability, LocatorId, OriginalMediaImport, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationKind, Resource, ResourceId,
    Timestamp,
};
use postproject_protocol::{CheckpointChunk, CheckpointSection, Document, FrameDecoder, Limits};
use postproject_storage_sqlite::{CheckpointLimits, SqliteProduction};

#[test]
fn rehashed_activity_contradictions_cannot_change_original_evidence_or_publish_a_destination() {
    let directory = tempfile::tempdir().unwrap();
    let source = fixture(&directory.path().join("source.pproj"));
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let frames = frames(&chunks);
    let (control, body) =
        super::replace_section(&manifest, &chunks, CheckpointSection::Activities, &frames);
    let control = SqliteProduction::import_checkpoint(
        directory.path().join("control.pproj"),
        &control,
        body.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(source.activities().unwrap(), control.activities().unwrap());
    let head = source.exchange_head().unwrap();
    for rewritten_frames in contradictions(&frames) {
        let (rewritten, body) = super::replace_section(
            &manifest,
            &chunks,
            CheckpointSection::Activities,
            &rewritten_frames,
        );
        let path = directory.path().join("rejected.pproj");
        assert!(
            SqliteProduction::import_checkpoint(
                &path,
                &rewritten,
                body.into_iter().map(Ok),
                CheckpointLimits::default()
            )
            .is_err()
        );
        assert!(!path.exists());
        assert_eq!(source.exchange_head().unwrap(), head);
        assert_eq!(source.activities().unwrap(), control.activities().unwrap());
    }
    assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".postproject-import-")
    }));
}

fn contradictions(frames: &[Document]) -> Vec<Vec<Document>> {
    let mut changes = Vec::new();
    let mut duplicate = frames.to_vec();
    duplicate.extend_from_slice(frames);
    changes.push(duplicate);
    changes.push(Vec::new());
    changes.push(frames[..frames.len() - 1].to_vec());
    let input: serde_json::Value =
        serde_json::from_slice(&frames[1].canonical_bytes().unwrap()).unwrap();
    for (index, field, value) in [
        (0, "activity_kind", serde_json::json!("unknown:Other")),
        (1, "role", serde_json::json!("unknown:Other")),
        (1, "snapshot_revision_sequence", serde_json::json!("2")),
        (3, "representation_id", input["representation_id"].clone()),
    ] {
        let mut copy = frames.to_vec();
        let mut document: serde_json::Value =
            serde_json::from_slice(&copy[index].canonical_bytes().unwrap()).unwrap();
        document[field] = value;
        copy[index] = Document::parse(document.to_string().as_bytes(), Limits::default()).unwrap();
        changes.push(copy);
    }
    let mut copy = frames.to_vec();
    let mut fingerprint: serde_json::Value =
        serde_json::from_slice(&copy[2].canonical_bytes().unwrap()).unwrap();
    fingerprint["fingerprint"]["value"] = serde_json::json!("Ag==");
    copy[2] = Document::parse(fingerprint.to_string().as_bytes(), Limits::default()).unwrap();
    changes.push(copy);
    changes
}

fn frames(chunks: &[CheckpointChunk]) -> Vec<Document> {
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut frames = Vec::new();
    for chunk in chunks
        .iter()
        .filter(|chunk| chunk.section() == CheckpointSection::Activities)
    {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, frame) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            frames.extend(frame);
        }
    }
    decoder.finish().unwrap();
    assert_eq!(frames.len(), 5);
    frames
}

fn fixture(path: &std::path::Path) -> SqliteProduction {
    let mut source = SqliteProduction::create(path, None).unwrap();
    let imports = [media(), media()];
    let input = imports[0].representation().id();
    let output = imports[1].representation().id();
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Render").unwrap(),
        vec![ActivityInput::new(input, None)],
        vec![ActivityOutput::new(output, None)],
    )
    .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    edit.create_activity(&activity).unwrap();
    edit.record_representation_fingerprint(
        input,
        &RepresentationFingerprint::new("content", 1, vec![2]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    source
}

fn media() -> OriginalMediaImport {
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(0), None, None);
    let resource = ResourceId::new();
    OriginalMediaImport::new(
        asset.clone(),
        Representation::new(
            RepresentationId::new(),
            asset.id(),
            RepresentationKind::Original,
            ContentStructure::single_resource(resource),
            vec![RepresentationFingerprint::new("content", 1, vec![1]).unwrap()],
        ),
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "file:///does-not-exist",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
