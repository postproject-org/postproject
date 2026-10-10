use postproject_core::RevisionId;
use postproject_protocol::{ArchiveEvidence, ArchiveFamily, CheckpointSection, ProtocolBase};
use postproject_storage_sqlite::{CheckpointLimits, ResynchronizationLimits, SqliteProduction};

#[test]
fn rehashed_archives_reject_wrong_boundaries_repeated_items_and_altered_anchors() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let (mut source, _) = super::media::fixture(&source_path);
    let connection = rusqlite::Connection::open(&source_path).unwrap();
    connection
        .execute("DELETE FROM exchange_records WHERE sequence = 1", [])
        .unwrap();
    let base = ProtocolBase::new(
        source.exchange_scope().unwrap(),
        source.read_session().unwrap().decision_base(),
    )
    .unwrap();
    source
        .resynchronize_exchange_history(base, ResynchronizationLimits::default())
        .unwrap();
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let frames = super::documents(&chunks, CheckpointSection::Archives);
    let anchor = ArchiveEvidence::from_document(&frames[0]).unwrap();
    let chunk = ArchiveEvidence::from_document(&frames[1]).unwrap();
    assert_eq!(anchor.family(), ArchiveFamily::Anchor);
    assert_eq!(chunk.family(), ArchiveFamily::Chunk);
    let (rewritten, body) =
        super::replace_section(&manifest, &chunks, CheckpointSection::Archives, &frames);
    let control = SqliteProduction::import_checkpoint(
        directory.path().join("control.pproj"),
        &rewritten,
        body.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        control.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    for frames in contradictions(&frames, &manifest) {
        let (rewritten, body) =
            super::replace_section(&manifest, &chunks, CheckpointSection::Archives, &frames);
        let destination = directory.path().join("rejected.pproj");
        assert!(
            SqliteProduction::import_checkpoint(
                &destination,
                &rewritten,
                body.into_iter().map(Ok),
                CheckpointLimits::default()
            )
            .is_err()
        );
        assert!(!destination.exists());
        assert_eq!(source.exchange_head().unwrap(), manifest.head());
    }
    assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".postproject-import-")
    }));
}

fn contradictions(
    frames: &[postproject_protocol::Document],
    manifest: &postproject_protocol::CheckpointManifest,
) -> Vec<Vec<postproject_protocol::Document>> {
    let chunk = ArchiveEvidence::from_document(&frames[1]).unwrap();
    let variants = [
        [frames, frames].concat(),
        vec![frames[1].clone(), frames[0].clone()],
        vec![
            ArchiveEvidence::new(ArchiveFamily::Anchor, None, 0, 0, 0, vec![0; 32])
                .unwrap()
                .document(),
            frames[1].clone(),
        ],
        vec![
            frames[0].clone(),
            ArchiveEvidence::new(
                ArchiveFamily::Chunk,
                Some(RevisionId::new()),
                1,
                0,
                0,
                chunk.payload().to_vec(),
            )
            .unwrap()
            .document(),
        ],
        vec![
            frames[0].clone(),
            ArchiveEvidence::new(
                ArchiveFamily::Chunk,
                chunk.revision(),
                2,
                0,
                0,
                chunk.payload().to_vec(),
            )
            .unwrap()
            .document(),
        ],
        vec![
            ArchiveEvidence::new(
                ArchiveFamily::Anchor,
                chunk.revision(),
                1,
                0,
                0,
                manifest.floor().digest().as_bytes().to_vec(),
            )
            .unwrap()
            .document(),
            frames[1].clone(),
        ],
    ];
    variants.into_iter().collect()
}
