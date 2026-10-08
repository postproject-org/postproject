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
    assert_eq!(
        RecordChunk::from_document(&chunk.document().unwrap()).unwrap(),
        chunk
    );
}

#[test]
fn strict_decoder_rejects_tampering_and_unsupported_framing() {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let revision = RevisionId::new();
    let chunk = RecordChunk::new(scope, revision, 0, None, vec![1], Extensions::default()).unwrap();
    let document = chunk.document().unwrap();
    let bytes = document.canonical_bytes().unwrap();
    assert_eq!(
        RecordChunk::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
        chunk
    );
    let text = String::from_utf8(bytes).unwrap();
    for (before, after, kind) in [
        ("AQ==", "Ag==", FailureKind::Integrity),
        ("AQ==", "AR==", FailureKind::Malformed),
        ("AQ==", "AQ", FailureKind::Malformed),
        (
            "\"version\":\"1\"",
            "\"version\":\"2\"",
            FailureKind::Unsupported,
        ),
        (
            "record-chunks.v1",
            "future-chunks.v1",
            FailureKind::Unsupported,
        ),
        (
            "\"index\":\"0\"",
            "\"index\":\"01\"",
            FailureKind::Malformed,
        ),
    ] {
        let altered = text.replace(before, after);
        let document = Document::parse(altered.as_bytes(), Limits::default()).unwrap();
        assert_eq!(
            RecordChunk::from_document(&document).unwrap_err().kind(),
            kind
        );
    }
    let altered = text.replace(&scope.history().to_string(), &HistoryId::new().to_string());
    assert_eq!(
        RecordChunk::from_document(
            &Document::parse(altered.as_bytes(), Limits::default()).unwrap()
        )
        .unwrap_err()
        .kind(),
        FailureKind::Integrity
    );
}

#[test]
fn chain_rejects_partial_reordered_foreign_and_altered_chunks_without_advancing() {
    use postproject_protocol::{ChunkSummary, RecordChunkChain};
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    let revision = RevisionId::new();
    let first =
        RecordChunk::new(scope, revision, 0, None, vec![1, 2], Extensions::default()).unwrap();
    let next = RecordChunk::new(
        scope,
        revision,
        1,
        Some(first.digest().unwrap()),
        vec![3],
        Extensions::default(),
    )
    .unwrap();
    let mut chain = RecordChunkChain::new(scope, revision);
    assert!(chain.clone().finish().is_err());
    assert_eq!(
        chain.push(&next).unwrap_err().kind(),
        FailureKind::HistoryGap
    );
    assert_eq!(
        chain
            .push(
                &RecordChunk::new(
                    Scope::new(scope.production(), HistoryId::new()),
                    revision,
                    0,
                    None,
                    vec![1],
                    Extensions::default()
                )
                .unwrap()
            )
            .unwrap_err()
            .kind(),
        FailureKind::ScopeMismatch
    );
    chain.push(&first).unwrap();
    let boundary = chain.clone().finish().unwrap();
    assert_eq!(boundary.count(), 1);
    assert_eq!(
        chain.push(&first).unwrap_err().kind(),
        FailureKind::HistoryGap
    );
    let altered = RecordChunk::new(
        scope,
        revision,
        1,
        Some(postproject_protocol::Digest::from_bytes([0; 32])),
        vec![3],
        Extensions::default(),
    )
    .unwrap();
    assert_eq!(
        chain.push(&altered).unwrap_err().kind(),
        FailureKind::Integrity
    );
    assert_eq!(chain.clone().finish().unwrap(), boundary);
    chain.push(&next).unwrap();
    assert_eq!(
        chain.finish().unwrap(),
        ChunkSummary::new(2, 3, next.digest().unwrap()).unwrap()
    );
    assert!(ChunkSummary::new(0, 0, first.digest().unwrap()).is_err());
    assert!(ChunkSummary::new(2, 1, first.digest().unwrap()).is_err());
}
