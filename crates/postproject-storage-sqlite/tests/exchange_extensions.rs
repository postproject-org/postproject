//! Unknown bounded submission facts survive outcomes and complete portable history.

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{
    ClientId, Command, Document, Extensions, FailureKind, Limits, Proposal, RequestId,
};
use postproject_storage_sqlite::{CheckpointLimits, ExchangeError, ReplayLimits, SqliteProduction};

#[test]
fn submission_extensions_survive_retry_record_replay_and_checkpoint_import() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&source_path, None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let extensions = Extensions::new(
        Document::parse(
            r#"{"unknown:portable":{"Exact":["名","9007199254740993",true,null]}}"#.as_bytes(),
            Limits::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let command = Command::AppendMetadata {
        target: ObjectRef::Production(source.production().id()),
        property: MetadataProperty::new(
            VocabularyId::new("unknown:metadata").unwrap(),
            PropertyId::new("Exact").unwrap(),
        ),
        value: MetadataValue::u64(u64::MAX),
    };
    let request = Proposal::new(
        source.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![command],
        extensions.clone(),
    )
    .unwrap();
    let outcome = source.submit_proposal(&request).unwrap();
    assert_eq!(outcome.extensions(), &extensions);
    drop(source);
    let mut source = SqliteProduction::open(&source_path).unwrap();
    assert_eq!(source.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(source.changes_since(0, 10).unwrap().len(), 1);
    let mut reader = source.record_reader(1).unwrap();
    let manifest = reader.manifest().clone();
    assert_eq!(manifest.extensions(), &extensions);
    mirror
        .apply_record(
            &manifest,
            std::iter::from_fn(|| reader.next_chunk().transpose()),
            ReplayLimits::default(),
        )
        .unwrap();
    assert_eq!(mirror.record_reader(1).unwrap().manifest(), &manifest);
    let mut chunks = Vec::new();
    let checkpoint = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let restored = SqliteProduction::import_checkpoint(
        directory.path().join("restored.pproj"),
        &checkpoint,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(restored.record_reader(1).unwrap().manifest(), &manifest);
    assert_eq!(
        restored.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );

    // Extensions are part of full intent; a changed extension cannot reuse the
    // accepted request's identity even when every command remains the same.
    let altered = Proposal::new(
        request.scope(),
        request.client(),
        request.request(),
        request.base(),
        request.context().clone(),
        request.commands().to_vec(),
        Extensions::default(),
    )
    .unwrap();
    assert!(
        matches!(source.submit_proposal(&altered), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::RequestIdentityMismatch)
    );
    assert_eq!(source.changes_since(0, 10).unwrap().len(), 1);
}
