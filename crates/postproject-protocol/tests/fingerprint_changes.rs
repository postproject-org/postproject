//! Original fingerprint transitions retain archive order and dirty-clear semantics.

use postproject_core::{FingerprintSnapshot, ObjectRef, RepresentationId, ResourceId};
use postproject_protocol::{Document, FingerprintChangeStart, FingerprintRecomputation, Limits};

fn snapshot(bytes: u8, sequence: u64) -> FingerprintSnapshot {
    FingerprintSnapshot::new("Exact-ALG", u16::MAX, vec![bytes], Some(sequence)).unwrap()
}

#[test]
fn changed_bytes_preserve_original_boundaries_and_same_revision_archive_order() {
    let change = FingerprintChangeStart::new(
        ObjectRef::Resource(ResourceId::new()),
        Some(snapshot(1, 7)),
        snapshot(2, 7),
        Some(9_007_199_254_740_993),
        9_007_199_254_740_993,
        None,
        false,
    )
    .unwrap();
    let encoded = change.document().unwrap().canonical_bytes().unwrap();
    let decoded = Document::parse(&encoded, Limits::default()).unwrap();
    assert_eq!(
        FingerprintChangeStart::from_document(&decoded).unwrap(),
        change
    );
    assert_eq!(
        change.previous().unwrap().observed_revision_sequence(),
        Some(7)
    );
    assert_eq!(change.current().observed_revision_sequence(), Some(7));
}

#[test]
fn clearing_dirty_unchanged_evidence_retains_its_original_observation() {
    let representation = RepresentationId::new();
    let marker = FingerprintRecomputation::new(representation, ResourceId::new(), 9).unwrap();
    let change = FingerprintChangeStart::new(
        ObjectRef::Representation(representation),
        Some(snapshot(1, 3)),
        snapshot(1, 3),
        None,
        0,
        Some(marker),
        true,
    )
    .unwrap();
    assert_eq!(
        FingerprintChangeStart::from_document(&change.document().unwrap()).unwrap(),
        change
    );
    assert_eq!(change.current().observed_revision_sequence(), Some(3));
    assert_eq!(change.cleared_marker(), Some(marker));
    assert!(change.dependency_invalidated());
    assert!(
        FingerprintChangeStart::new(
            change.target(),
            change.previous().cloned(),
            snapshot(1, 10),
            None,
            0,
            Some(marker),
            true
        )
        .is_err()
    );
    assert!(
        FingerprintChangeStart::new(
            change.target(),
            change.previous().cloned(),
            snapshot(1, 3),
            None,
            0,
            None,
            true
        )
        .is_err()
    );
}

#[test]
fn impossible_domain_marker_and_archive_combinations_reject() {
    let resource = ObjectRef::Resource(ResourceId::new());
    let representation = RepresentationId::new();
    let marker = FingerprintRecomputation::new(representation, ResourceId::new(), 1).unwrap();
    assert!(
        FingerprintChangeStart::new(resource, None, snapshot(1, 1), Some(0), 0, None, false)
            .is_err()
    );
    assert!(
        FingerprintChangeStart::new(
            resource,
            Some(snapshot(1, 1)),
            snapshot(2, 2),
            None,
            0,
            None,
            false
        )
        .is_err()
    );
    assert!(
        FingerprintChangeStart::new(resource, None, snapshot(1, 1), None, 0, Some(marker), false)
            .is_err()
    );
    assert!(
        FingerprintChangeStart::new(
            ObjectRef::Representation(representation),
            None,
            snapshot(1, 1),
            None,
            1,
            None,
            false
        )
        .is_err()
    );
    assert!(
        FingerprintChangeStart::new(
            ObjectRef::Representation(RepresentationId::new()),
            None,
            snapshot(1, 1),
            None,
            0,
            Some(marker),
            false
        )
        .is_err()
    );
}
