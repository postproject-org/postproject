//! Recompute markers retain an observation boundary without launching work.

use postproject_core::{RepresentationId, ResourceId};
use postproject_protocol::{Document, FingerprintRecomputation, Limits};

#[test]
fn markers_retain_original_identity_and_exact_revision_boundaries() {
    for sequence in [1, 9_007_199_254_740_993, i64::MAX.unsigned_abs()] {
        let marker =
            FingerprintRecomputation::new(RepresentationId::new(), ResourceId::new(), sequence)
                .unwrap();
        let restored = FingerprintRecomputation::from_document(
            &Document::parse(
                &marker.document().canonical_bytes().unwrap(),
                Limits::default(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(restored, marker);
        assert_eq!(restored.revision_sequence(), sequence);
    }
    for sequence in [0, u64::MAX] {
        assert!(
            FingerprintRecomputation::new(RepresentationId::new(), ResourceId::new(), sequence)
                .is_err()
        );
    }
}
