use postproject_core::{
    Asset, AssetId, ContentStructure, Dependency, DependencyKind, DependencySetStatus,
    DependencyTarget, Locator, LocatorAvailability, LocatorId, OriginalMediaImport, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationKind, Resource, ResourceId,
    Timestamp,
};
use postproject_protocol::{
    CheckpointId, CheckpointSection, DependencySetHeader, Document, FrameDecoder, Limits,
};

use crate::{
    SqliteProduction,
    exchange::{
        checkpoint::{sections, writer::SectionWriter},
        records::DependencyApply,
    },
};

#[test]
fn complete_dependency_stream_preserves_repeated_occurrences_empty_sets_and_dirty_state() {
    let directory = tempfile::tempdir().unwrap();
    let (source, imports) = fixture(directory.path());
    let input = imports[0].representation().id();
    let output = imports[1].representation().id();
    let header = super::header(&source.connection, input).unwrap();
    assert_eq!(header.status(), DependencySetStatus::NeedsExtraction);
    assert_eq!(header.recorded_at_revision(), 2);
    assert_eq!(header.occurrence_count(), 1001);
    assert_eq!(
        super::header(&source.connection, output)
            .unwrap()
            .occurrence_count(),
        0
    );
    let frames = frames(&source);
    assert_eq!(frames.len(), 1003);
    let mut destination =
        SqliteProduction::create(directory.path().join("stage.pproj"), None).unwrap();
    let mut edit = destination.begin_transaction().unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    let transaction = destination.connection.transaction().unwrap();
    let mut pending = None;
    for frame in &frames {
        if let Some(state) = pending.as_mut() {
            if DependencyApply::push(state, &transaction, frame).unwrap() {
                pending = None;
            }
        } else {
            let header = DependencySetHeader::from_document(frame).unwrap();
            let state = DependencyApply::begin_checkpoint(&transaction, 2, header).unwrap();
            if header.occurrence_count() != 0 {
                pending = Some(state);
            }
        }
    }
    assert!(pending.is_none());
    for owner in [input, output] {
        assert_eq!(
            crate::load_dependency_set(&source.connection, owner).unwrap(),
            crate::load_dependency_set(&transaction, owner).unwrap()
        );
    }
    transaction.rollback().unwrap();
    source.export_checkpoint(|_| Ok(())).unwrap();
}

fn frames(source: &SqliteProduction) -> Vec<Document> {
    let mut chunks = Vec::new();
    let mut sink = |chunk| {
        chunks.push(chunk);
        Ok(())
    };
    let mut writer = SectionWriter::new(
        source.exchange_scope().unwrap(),
        CheckpointId::new(),
        CheckpointSection::Dependencies,
        &mut sink,
    );
    sections::dependencies(
        source,
        &mut writer,
        source.exchange_head().unwrap().sequence(),
    )
    .unwrap();
    assert_eq!(writer.finish().unwrap().items(), 2);
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut frames = Vec::new();
    for chunk in chunks {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, frame) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            frames.extend(frame);
        }
    }
    decoder.finish().unwrap();
    frames
}

#[test]
fn future_boundaries_unknown_sources_missing_targets_and_skipped_occurrences_reject() {
    let directory = tempfile::tempdir().unwrap();
    let (source, imports) = fixture(directory.path());
    let input = imports[0].representation().id();
    let header = super::header(&source.connection, input).unwrap();
    let transaction = source.connection.unchecked_transaction().unwrap();
    assert!(DependencyApply::begin_checkpoint(&transaction, 1, header).is_err());
    let unknown =
        DependencySetHeader::new(RepresentationId::new(), 2, DependencySetStatus::Current, 0)
            .unwrap();
    assert!(DependencyApply::begin_checkpoint(&transaction, 2, unknown).is_err());
    let dependency = crate::load_dependency_set(&transaction, input)
        .unwrap()
        .unwrap()
        .dependencies()[0]
        .clone();
    transaction
        .execute(
            "DELETE FROM dependency_sets WHERE source_representation_id = ?1",
            [input.as_bytes().as_slice()],
        )
        .unwrap();
    let mut pending = DependencyApply::begin_checkpoint(
        &transaction,
        2,
        DependencySetHeader::new(input, 2, DependencySetStatus::Current, 1).unwrap(),
    )
    .unwrap();
    let skipped = postproject_protocol::DependencyOccurrence::new(input, 1, dependency)
        .unwrap()
        .document()
        .unwrap();
    assert!(pending.push(&transaction, &skipped).is_err());
    let missing = Dependency::new(
        None,
        DependencyKind::new("unknown:Reference").unwrap(),
        DependencyTarget::Representation(RepresentationId::new()),
        None,
        true,
        " exact ",
    )
    .unwrap();
    let missing = postproject_protocol::DependencyOccurrence::new(input, 0, missing)
        .unwrap()
        .document()
        .unwrap();
    assert!(pending.push(&transaction, &missing).is_err());
    transaction.rollback().unwrap();
    assert_eq!(super::header(&source.connection, input).unwrap(), header);
}

fn fixture(directory: &std::path::Path) -> (SqliteProduction, [OriginalMediaImport; 2]) {
    let mut source = SqliteProduction::create(directory.join("source.pproj"), None).unwrap();
    let imports = [media(), media()];
    let input = imports[0].representation().id();
    let output = imports[1].representation().id();
    let mut edit = source.begin_transaction().unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    let dependencies: Vec<_> = (0..1001)
        .map(|position| {
            Dependency::new(
                Some(imports[0].resources()[0].id()),
                DependencyKind::new("unknown:Reference").unwrap(),
                DependencyTarget::Asset(imports[1].asset().id()),
                if position % 3 == 0 {
                    None
                } else {
                    Some(output)
                },
                position % 2 == 0,
                "  /名/../A.%04d.exr  ",
            )
            .unwrap()
        })
        .collect();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_dependency_set(input, &dependencies).unwrap();
    edit.record_dependency_set(output, &[]).unwrap();
    edit.record_representation_fingerprint(
        input,
        &RepresentationFingerprint::new("domain", 1, vec![2]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    (source, imports)
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
            Vec::new(),
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
