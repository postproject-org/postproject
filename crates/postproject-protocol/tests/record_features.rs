//! Required body features are explicit, canonical and integrity-covered.

use postproject_core::{ProductionId, Revision, RevisionId, Timestamp, TransactionId};
use postproject_protocol::{
    ChunkSummary, Digest, Document, Extensions, FailureKind, HistoryId, Limits, Position,
    RecordFeature, RecordManifest, Scope,
};

fn metadata_record() -> RecordManifest {
    let scope = Scope::new(ProductionId::new(), HistoryId::new());
    RecordManifest::new(
        Position::new(scope, None, 0, Digest::from_bytes([7; 32])).unwrap(),
        Revision::new(
            RevisionId::new(),
            1,
            TransactionId::new(),
            Timestamp::from_unix_micros(-123),
            None,
            None,
        )
        .unwrap(),
        ChunkSummary::new(1, 1, Digest::from_bytes([9; 32])).unwrap(),
        1,
        1,
        Extensions::default(),
    )
    .unwrap()
}

#[test]
fn earlier_metadata_profile_is_unchanged_and_media_changes_the_commitment() {
    let original = metadata_record();
    let bytes = original.document().unwrap().canonical_bytes().unwrap();
    assert!(
        String::from_utf8(bytes.clone())
            .unwrap()
            .contains(r#""required_features":["metadata.v1","record-chunks.v1"]"#)
    );
    assert_eq!(
        RecordManifest::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
            .unwrap(),
        original
    );
    let media = original
        .clone()
        .with_required_features([
            RecordFeature::RecordChunks,
            RecordFeature::Metadata,
            RecordFeature::Media,
        ])
        .unwrap();
    assert_ne!(
        media.record_digest().unwrap(),
        original.record_digest().unwrap()
    );
    assert_eq!(
        media.required_features().collect::<Vec<_>>(),
        [
            RecordFeature::Media,
            RecordFeature::Metadata,
            RecordFeature::RecordChunks
        ]
    );
    assert_eq!(
        RecordManifest::from_document(&media.document().unwrap()).unwrap(),
        media
    );
    let creation = original
        .clone()
        .with_required_features([RecordFeature::Media, RecordFeature::RecordChunks])
        .unwrap();
    assert_eq!(
        RecordManifest::from_document(&creation.document().unwrap()).unwrap(),
        creation
    );
    assert!(
        original
            .clone()
            .with_required_features([RecordFeature::Media])
            .is_err()
    );
    assert!(
        original
            .with_required_features([RecordFeature::RecordChunks])
            .is_err()
    );
}

#[test]
fn unknown_duplicate_unordered_or_missing_requirements_reject() {
    let original = metadata_record()
        .document()
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let text = String::from_utf8(original).unwrap();
    for (features, kind) in [
        (
            r#"["future.v1","record-chunks.v1"]"#,
            FailureKind::Unsupported,
        ),
        (
            r#"["metadata.v1","metadata.v1","record-chunks.v1"]"#,
            FailureKind::Malformed,
        ),
        (
            r#"["record-chunks.v1","metadata.v1"]"#,
            FailureKind::Malformed,
        ),
        (r#"["metadata.v1"]"#, FailureKind::Malformed),
        (r#"["record-chunks.v1"]"#, FailureKind::Malformed),
        ("[]", FailureKind::Malformed),
        (
            r#"["media.v1","metadata.v1","record-chunks.v1"]"#,
            FailureKind::Integrity,
        ),
    ] {
        let changed = text.replace(r#"["metadata.v1","record-chunks.v1"]"#, features);
        let document = Document::parse(changed.as_bytes(), Limits::default()).unwrap();
        assert_eq!(
            RecordManifest::from_document(&document).unwrap_err().kind(),
            kind
        );
    }
}

#[test]
fn dependency_requirements_are_sorted_and_integrity_covered() {
    let original = metadata_record();
    let changed = original
        .clone()
        .with_required_features([
            RecordFeature::RecordChunks,
            RecordFeature::Media,
            RecordFeature::Dependencies,
        ])
        .unwrap();
    assert_eq!(
        changed.required_features().collect::<Vec<_>>(),
        [
            RecordFeature::Dependencies,
            RecordFeature::Media,
            RecordFeature::RecordChunks
        ]
    );
    assert_ne!(
        changed.record_digest().unwrap(),
        original.record_digest().unwrap()
    );
    assert_eq!(
        RecordManifest::from_document(&changed.document().unwrap()).unwrap(),
        changed
    );
}

#[test]
fn provenance_requirements_are_sorted_and_integrity_covered() {
    let original = metadata_record();
    let changed = original
        .clone()
        .with_required_features([
            RecordFeature::RecordChunks,
            RecordFeature::Provenance,
            RecordFeature::Media,
            RecordFeature::Dependencies,
        ])
        .unwrap();
    assert_eq!(
        changed.required_features().collect::<Vec<_>>(),
        [
            RecordFeature::Dependencies,
            RecordFeature::Media,
            RecordFeature::Provenance,
            RecordFeature::RecordChunks,
        ]
    );
    assert_ne!(
        changed.record_digest().unwrap(),
        original.record_digest().unwrap()
    );
    assert_eq!(
        RecordManifest::from_document(&changed.document().unwrap()).unwrap(),
        changed
    );
}

#[test]
fn job_requirements_are_sorted_and_integrity_covered() {
    let original = metadata_record();
    let changed = original
        .clone()
        .with_required_features([
            RecordFeature::RecordChunks,
            RecordFeature::Media,
            RecordFeature::Jobs,
            RecordFeature::Dependencies,
        ])
        .unwrap();
    assert_eq!(
        changed.required_features().collect::<Vec<_>>(),
        [
            RecordFeature::Dependencies,
            RecordFeature::Jobs,
            RecordFeature::Media,
            RecordFeature::RecordChunks,
        ]
    );
    assert_ne!(
        changed.record_digest().unwrap(),
        original.record_digest().unwrap()
    );
    assert_eq!(
        RecordManifest::from_document(&changed.document().unwrap()).unwrap(),
        changed
    );
}
