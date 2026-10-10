//! Well-hashed envelopes do not excuse contradictory domain/history facts.

#[path = "checkpoint_integrity/activity.rs"]
mod activity;
#[path = "checkpoint_integrity/archives.rs"]
mod archives;
#[path = "checkpoint_integrity/dependencies.rs"]
mod dependencies;
#[path = "checkpoint_integrity/jobs.rs"]
mod jobs;
#[path = "checkpoint_integrity/media.rs"]
mod media;

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};
use postproject_protocol::{
    CheckpointChunk, CheckpointChunkChain, CheckpointManifest, CheckpointSection, Document,
    Extensions, FrameDecoder, Limits, SectionSummary, SnapshotAssertion,
};
use postproject_storage_sqlite::{CheckpointLimits, SqliteProduction};

fn replace_section(
    manifest: &CheckpointManifest,
    chunks: &[CheckpointChunk],
    section: CheckpointSection,
    documents: &[Document],
) -> (CheckpointManifest, Vec<CheckpointChunk>) {
    let mut payload = Vec::new();
    for document in documents {
        let bytes = document.canonical_bytes().unwrap();
        payload.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        payload.extend_from_slice(&bytes);
    }
    let mut replacement = Vec::new();
    let mut chain = CheckpointChunkChain::new(manifest.head().scope(), manifest.id(), section);
    if !payload.is_empty() {
        let chunk = CheckpointChunk::new(
            manifest.head().scope(),
            manifest.id(),
            section,
            0,
            None,
            payload,
            Extensions::default(),
        )
        .unwrap();
        chain.push(&chunk).unwrap();
        replacement.push(chunk);
    }
    let mut summaries = *manifest.sections();
    let summary = summaries
        .iter_mut()
        .find(|summary| summary.section() == section)
        .unwrap();
    *summary = SectionSummary::new(
        section,
        documents
            .iter()
            .filter(|document| match section {
                CheckpointSection::Activities => document.kind().unwrap() == "activity.header",
                CheckpointSection::Dependencies => document.kind().unwrap() == "dependency.set",
                CheckpointSection::Jobs => document.kind().unwrap() == "job.header",
                _ => true,
            })
            .count() as u64,
        if replacement.is_empty() {
            None
        } else {
            Some(chain.finish().unwrap())
        },
    )
    .unwrap();
    let rewritten = CheckpointManifest::new(
        manifest.id(),
        manifest.head(),
        manifest.floor(),
        summaries,
        manifest.extensions().clone(),
    )
    .unwrap();
    let mut output = Vec::new();
    for next in CheckpointSection::ALL {
        if next == section {
            output.extend(replacement.iter().cloned());
        } else {
            output.extend(
                chunks
                    .iter()
                    .filter(|chunk| chunk.section() == next)
                    .cloned(),
            );
        }
    }
    (rewritten, output)
}

#[test]
fn unsupported_checkpoint_extensions_are_rejected_without_silent_loss() {
    use postproject_protocol::{ChunkSummary, FailureKind};
    use postproject_storage_sqlite::ExchangeError;
    let directory = tempfile::tempdir().unwrap();
    let source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let extensions = Extensions::new(
        Document::parse(br#"{"example:fact":["Exact","Exact"]}"#, Limits::default()).unwrap(),
    )
    .unwrap();
    let extended = CheckpointManifest::new(
        manifest.id(),
        manifest.head(),
        manifest.floor(),
        *manifest.sections(),
        extensions.clone(),
    )
    .unwrap();
    let first = &chunks[0];
    let chunk = CheckpointChunk::new(
        first.scope(),
        first.checkpoint(),
        first.section(),
        first.index(),
        first.previous(),
        first.payload().to_vec(),
        extensions,
    )
    .unwrap();
    let mut summaries = *manifest.sections();
    summaries[0] = SectionSummary::new(
        CheckpointSection::Production,
        1,
        Some(ChunkSummary::new(1, chunk.payload().len() as u64, chunk.digest().unwrap()).unwrap()),
    )
    .unwrap();
    let chunk_manifest = CheckpointManifest::new(
        manifest.id(),
        manifest.head(),
        manifest.floor(),
        summaries,
        Extensions::default(),
    )
    .unwrap();
    let mut extended_chunks = chunks.clone();
    extended_chunks[0] = chunk;
    for (manifest, chunks) in [(extended, chunks), (chunk_manifest, extended_chunks)] {
        let destination = directory.path().join("rejected.pproj");
        assert!(
            matches!(SqliteProduction::import_checkpoint(&destination, &manifest, chunks.into_iter().map(Ok), CheckpointLimits::default()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::Unsupported)
        );
        assert!(!destination.exists());
    }
}

fn documents(chunks: &[CheckpointChunk], section: CheckpointSection) -> Vec<Document> {
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut documents = Vec::new();
    for chunk in chunks.iter().filter(|chunk| chunk.section() == section) {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (count, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += count;
            documents.extend(document);
        }
    }
    decoder.finish().unwrap();
    documents
}

#[test]
fn contradictory_snapshots_orders_and_missing_semantic_versions_reject() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let target = ObjectRef::Production(source.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:unknown:checkpoint").unwrap(),
        PropertyId::new("Exact").unwrap(),
    );
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property, &MetadataValue::i64(1))
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
    let wrong_value = SnapshotAssertion::new(target, property.clone(), 0, MetadataValue::i64(2))
        .unwrap()
        .document()
        .unwrap();
    let wrong_order = SnapshotAssertion::new(target, property.clone(), 1, MetadataValue::i64(1))
        .unwrap()
        .document()
        .unwrap();
    let unexplained = SnapshotAssertion::new(
        target,
        MetadataProperty::new(
            property.vocabulary().clone(),
            PropertyId::new("Other").unwrap(),
        ),
        0,
        MetadataValue::i64(1),
    )
    .unwrap()
    .document()
    .unwrap();
    let mut extra = documents(&chunks, CheckpointSection::Metadata);
    extra.push(unexplained);
    for (section, body) in [
        (CheckpointSection::Metadata, vec![wrong_value]),
        (CheckpointSection::Metadata, vec![wrong_order]),
        (CheckpointSection::Metadata, extra),
        (CheckpointSection::ConflictVersions, Vec::new()),
    ] {
        let (rewritten, rewritten_chunks) = replace_section(&manifest, &chunks, section, &body);
        assert_eq!(rewritten.head(), manifest.head());
        let destination = directory.path().join("rejected.pproj");
        assert!(
            SqliteProduction::import_checkpoint(
                &destination,
                &rewritten,
                rewritten_chunks.into_iter().map(Ok),
                CheckpointLimits::default()
            )
            .is_err()
        );
        assert!(!destination.exists());
    }
    let mut extra_chunk = chunks.clone();
    extra_chunk.push(chunks[0].clone());
    let destination = directory.path().join("extra.pproj");
    assert!(
        SqliteProduction::import_checkpoint(
            &destination,
            &manifest,
            extra_chunk.into_iter().map(Ok),
            CheckpointLimits::default()
        )
        .is_err()
    );
    assert!(!destination.exists());
    let imported = SqliteProduction::import_checkpoint(
        directory.path().join("valid.pproj"),
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        imported.metadata_values(target, &property).unwrap(),
        vec![MetadataValue::i64(1)]
    );
}

#[test]
fn semantic_version_sets_accept_earlier_ordering_but_reject_repeated_domain_keys() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    for name in ["aaa", "b"] {
        let property = MetadataProperty::new(
            VocabularyId::new("urn:keys").unwrap(),
            PropertyId::new(name).unwrap(),
        );
        edit.add_metadata_value(target, &property, &MetadataValue::i64(7))
            .unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let mut versions = documents(&chunks, CheckpointSection::ConflictVersions);
    assert_eq!(versions.len(), 2);
    // The previous development exporter sorted metadata property text rather
    // than its checked native index key. Both unique sets carry the same facts.
    versions.reverse();
    let (reordered, body) = replace_section(
        &manifest,
        &chunks,
        CheckpointSection::ConflictVersions,
        &versions,
    );
    let mirror = SqliteProduction::import_checkpoint(
        directory.path().join("mirror.pproj"),
        &reordered,
        body.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    for name in ["aaa", "b"] {
        let property = MetadataProperty::new(
            VocabularyId::new("urn:keys").unwrap(),
            PropertyId::new(name).unwrap(),
        );
        assert_eq!(
            mirror.metadata_values(target, &property).unwrap(),
            source.metadata_values(target, &property).unwrap()
        );
    }
    versions[1] = versions[0].clone();
    let (duplicated, body) = replace_section(
        &manifest,
        &chunks,
        CheckpointSection::ConflictVersions,
        &versions,
    );
    let destination = directory.path().join("duplicated.pproj");
    assert!(
        matches!(SqliteProduction::import_checkpoint(&destination, &duplicated,
        body.into_iter().map(Ok), CheckpointLimits::default()), Err(postproject_storage_sqlite::ExchangeError::Protocol(error))
        if error.kind() == postproject_protocol::FailureKind::Integrity)
    );
    assert!(!destination.exists());
}

#[test]
fn rehashed_root_state_cannot_resurrect_removed_roots_or_hide_authored_changes() {
    use postproject_core::{MediaRoot, MediaRootId};
    use postproject_protocol::encode_root;
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let retained = MediaRoot::new(MediaRootId::new(), "Essence", None, None, 2, true).unwrap();
    let removed = MediaRoot::new(MediaRootId::new(), "Old", None, None, 3, true).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.add_media_root(retained.clone()).unwrap();
    edit.add_media_root(removed.clone()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.set_media_root_enabled(retained.id(), false).unwrap();
    edit.remove_media_root(removed.id()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let valid = documents(&chunks, CheckpointSection::Roots);
    let wrong_label = MediaRoot::new(
        retained.id(),
        "Essence",
        Some("Forged".into()),
        None,
        2,
        false,
    )
    .unwrap();
    let extra = MediaRoot::new(MediaRootId::new(), "Unexplained", None, None, 1, true).unwrap();
    let cases = [
        (CheckpointSection::Roots, vec![encode_root(&retained)]),
        (CheckpointSection::Roots, vec![encode_root(&wrong_label)]),
        (
            CheckpointSection::Roots,
            vec![valid[0].clone(), encode_root(&removed)],
        ),
        (
            CheckpointSection::Roots,
            vec![valid[0].clone(), encode_root(&extra)],
        ),
        (
            CheckpointSection::Roots,
            vec![valid[0].clone(), valid[0].clone()],
        ),
        (CheckpointSection::Roots, Vec::new()),
        (CheckpointSection::ConflictVersions, Vec::new()),
    ];
    for (section, documents) in cases {
        let (rewritten, body) = replace_section(&manifest, &chunks, section, &documents);
        let destination = directory.path().join("rejected-root.pproj");
        assert!(
            matches!(SqliteProduction::import_checkpoint(&destination, &rewritten,
            body.into_iter().map(Ok), CheckpointLimits::default()), Err(postproject_storage_sqlite::ExchangeError::Protocol(error))
            if error.kind() == postproject_protocol::FailureKind::Integrity)
        );
        assert!(!destination.exists());
    }
}
