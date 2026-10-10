use postproject_core::{Dependency, DependencyKind, DependencyTarget, RepresentationFingerprint};
use postproject_protocol::{CheckpointSection, Document, FrameDecoder, Limits};
use postproject_storage_sqlite::{CheckpointLimits, SqliteProduction};

#[test]
fn rehashed_current_dependency_contradictions_publish_no_destination() {
    let directory = tempfile::tempdir().unwrap();
    let (mut source, import) = super::media::fixture(&directory.path().join("source.pproj"));
    let owner = import.representation().id();
    let dependency = Dependency::new(
        Some(import.resources()[0].id()),
        DependencyKind::new("unknown:Exact").unwrap(),
        DependencyTarget::Representation(owner),
        None,
        true,
        "  名/EXACT%2f  ",
    )
    .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_dependency_set(owner, &[dependency.clone(), dependency])
        .unwrap();
    edit.record_representation_fingerprint(
        owner,
        &RepresentationFingerprint::new("domain", 1, vec![2]).unwrap(),
    )
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
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut frames = Vec::new();
    for chunk in chunks
        .iter()
        .filter(|chunk| chunk.section() == CheckpointSection::Dependencies)
    {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, frame) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            frames.extend(frame);
        }
    }
    decoder.finish().unwrap();
    assert_eq!(frames.len(), 3);
    let (control, body) =
        super::replace_section(&manifest, &chunks, CheckpointSection::Dependencies, &frames);
    let control = SqliteProduction::import_checkpoint(
        directory.path().join("control.pproj"),
        &control,
        body.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        source.dependency_set(owner).unwrap(),
        control.dependency_set(owner).unwrap()
    );
    let head = source.exchange_head().unwrap();
    for frames in contradictions(&frames) {
        let (rewritten, body) =
            super::replace_section(&manifest, &chunks, CheckpointSection::Dependencies, &frames);
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
        assert_eq!(
            source.dependency_set(owner).unwrap(),
            control.dependency_set(owner).unwrap()
        );
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
    let mut contradictions = vec![Vec::new(), frames[..2].to_vec()];
    let mut duplicate = frames.to_vec();
    duplicate.extend_from_slice(frames);
    contradictions.push(duplicate);
    for (index, path, value) in [
        (
            0,
            vec!["recorded_revision_sequence"],
            serde_json::json!("1"),
        ),
        (0, vec!["status"], serde_json::json!("current")),
        (0, vec!["occurrence_count"], serde_json::json!("1")),
        (1, vec!["position"], serde_json::json!("1")),
        (
            1,
            vec!["dependency", "authored_reference"],
            serde_json::json!("different"),
        ),
        (1, vec!["dependency", "required"], serde_json::json!(false)),
        (
            1,
            vec!["dependency", "source_resource_id"],
            serde_json::json!(postproject_core::ResourceId::new().to_string()),
        ),
    ] {
        let mut copy = frames.to_vec();
        let mut document: serde_json::Value =
            serde_json::from_slice(&copy[index].canonical_bytes().unwrap()).unwrap();
        let mut field = &mut document;
        for key in path {
            field = &mut field[key];
        }
        *field = value;
        copy[index] = Document::parse(document.to_string().as_bytes(), Limits::default()).unwrap();
        contradictions.push(copy);
    }
    contradictions
}
