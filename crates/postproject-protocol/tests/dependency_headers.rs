//! Complete empty observations and original extraction boundaries.

use postproject_core::{DependencySet, DependencySetStatus, RepresentationId};
use postproject_protocol::{DependencySetHeader, Document, Limits};

#[test]
fn headers_preserve_empty_sets_status_and_exact_revision_boundaries() {
    let source = RepresentationId::new();
    for sequence in [1, 9_007_199_254_740_993, i64::MAX as u64] {
        for status in [
            DependencySetStatus::Current,
            DependencySetStatus::NeedsExtraction,
        ] {
            let set = DependencySet::new(source, sequence, status, Vec::new()).unwrap();
            let header = DependencySetHeader::from_set(&set).unwrap();
            assert_eq!(header.source_representation_id(), source);
            assert_eq!(header.recorded_at_revision(), sequence);
            assert_eq!(header.status(), status);
            assert_eq!(header.occurrence_count(), 0);
            assert_eq!(
                DependencySetHeader::from_document(
                    &Document::parse(
                        &header.document().canonical_bytes().unwrap(),
                        Limits::default()
                    )
                    .unwrap()
                )
                .unwrap(),
                header
            );
        }
    }
}

#[test]
fn invalid_boundaries_status_and_unknown_fields_reject() {
    let header = DependencySetHeader::new(
        RepresentationId::new(),
        1,
        DependencySetStatus::Current,
        100_000,
    )
    .unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&header.document().canonical_bytes().unwrap()).unwrap();
    for (key, value) in [
        ("recorded_revision_sequence", "0"),
        ("recorded_revision_sequence", "9223372036854775808"),
        ("occurrence_count", "100001"),
        ("occurrence_count", "01"),
        ("status", "future"),
        ("unknown", "ignored"),
    ] {
        let mut altered = document.clone();
        altered[key] = value.into();
        assert!(
            DependencySetHeader::from_document(
                &Document::parse(&serde_json::to_vec(&altered).unwrap(), Limits::default())
                    .unwrap()
            )
            .is_err()
        );
    }
}
