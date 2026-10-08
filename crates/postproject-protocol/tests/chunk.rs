//! Chunk integrity includes source, order, bytes and preserved extensions.

use postproject_core::{ProductionId, RevisionId};
use postproject_protocol::{
    DigestDomain, Document, Extensions, FailureKind, HistoryId, Limits, MAX_RECORD_CHUNK_BYTES,
    MAX_RECORD_CHUNK_PAYLOAD, RecordChunk, Scope,
};

#[test]
fn continued_bytes_are_bounded_without_requiring_each_fragment_to_be_json() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let revision = RevisionId::new();
    let first = RecordChunk::new(
        scope,
        revision,
        0,
        None,
        vec![0xf0, 0x9f],
        Extensions::default(),
    )
    .unwrap();
    let second = RecordChunk::new(
        scope,
        revision,
        1,
        Some(first.digest().unwrap()),
        vec![0x8c, 0x8d],
        Extensions::default(),
    )
    .unwrap();
    assert_eq!(second.previous(), Some(first.digest().unwrap()));
    assert_eq!(
        [first.payload(), second.payload()].concat(),
        "🌍".as_bytes()
    );
    assert_eq!(
        first
            .document()
            .unwrap()
            .digest(DigestDomain::Chunk)
            .unwrap(),
        first.digest().unwrap()
    );
    assert!(RecordChunk::new(scope, revision, 1, None, vec![1], Extensions::default()).is_err());
    assert!(
        RecordChunk::new(
            scope,
            revision,
            0,
            Some(first.digest().unwrap()),
            vec![1],
            Extensions::default()
        )
        .is_err()
    );
    assert!(RecordChunk::new(scope, revision, 0, None, vec![], Extensions::default()).is_err());
    assert_eq!(
        RecordChunk::new(
            scope,
            revision,
            0,
            None,
            vec![0; MAX_RECORD_CHUNK_PAYLOAD + 1],
            Extensions::default()
        )
        .unwrap_err()
        .kind(),
        FailureKind::LimitExceeded
    );
}

#[test]
fn maximum_payload_and_extensions_fit_the_encoded_stream_limit() {
    let extensions = Extensions::new(
        Document::parse(
            br#"{"urn:extra":["exact","9007199254740993"]}"#,
            Limits::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let chunk = RecordChunk::new(
        Scope::new(ProductionId::new(), HistoryId::new()),
        RevisionId::new(),
        0,
        None,
        vec![255; MAX_RECORD_CHUNK_PAYLOAD],
        extensions,
    )
    .unwrap();
    let bytes = chunk.document().unwrap().canonical_bytes().unwrap();
    assert!(bytes.len() < MAX_RECORD_CHUNK_BYTES);
    Document::parse(
        &bytes,
        Limits::new(MAX_RECORD_CHUNK_BYTES, 192, 8192).unwrap(),
    )
    .unwrap();
}
