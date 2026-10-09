//! Captured dependency evidence retains its historical status and authored text.

use postproject_core::{
    ArtifactDependencyPathSegment, AssetId, Dependency, DependencyKind, DependencyTarget,
    RepresentationId, ResourceId,
};
use postproject_protocol::{
    ActivityPathHeader, ActivityPathSegment, ActivityPathStatus, DependencyOccurrence, Document,
    Limits,
};

#[test]
fn path_statuses_and_large_exact_counts_round_trip() {
    for (status, segments, fingerprints) in [
        (ActivityPathStatus::Recorded, 1, 0),
        (ActivityPathStatus::Recorded, 64, 9_007_199_254_740_993),
        (ActivityPathStatus::NeedsExtraction, 0, 0),
        (ActivityPathStatus::NeedsExtraction, 64, 0),
        (ActivityPathStatus::Unresolved, 1, 0),
        (ActivityPathStatus::DepthTruncated, 64, 0),
        (ActivityPathStatus::RepresentationsTruncated, 1, 0),
    ] {
        let header = ActivityPathHeader::new(
            9_007_199_254_740_993,
            status,
            RepresentationId::new(),
            segments,
            fingerprints,
        )
        .unwrap();
        assert_eq!(
            ActivityPathHeader::from_document(&header.document()).unwrap(),
            header
        );
        assert_eq!(header.status(), status);
        assert_eq!(header.segment_count(), segments);
        assert_eq!(header.fingerprint_count(), fingerprints);
    }
}

#[test]
fn path_segments_preserve_both_positions_and_checked_required_dependencies() {
    for target in [
        DependencyTarget::Asset(AssetId::new()),
        DependencyTarget::Representation(RepresentationId::new()),
    ] {
        let resolved = matches!(target, DependencyTarget::Asset(_)).then(RepresentationId::new);
        let historical = ArtifactDependencyPathSegment::new(
            RepresentationId::new(),
            99_999,
            Some(ResourceId::new()),
            DependencyKind::new("unknown:Exact").unwrap(),
            target,
            resolved,
            "  /镜头/../A.%04d.exr  ".into(),
        );
        let segment = ActivityPathSegment::from_segment(63, &historical).unwrap();
        let restored = ActivityPathSegment::from_document(&segment.document().unwrap()).unwrap();
        assert_eq!(restored, segment);
        assert_eq!(restored.position(), 63);
        assert_eq!(restored.occurrence().position(), 99_999);
        assert_eq!(restored.occurrence().dependency().target(), target);
        assert_eq!(
            restored.occurrence().dependency().authored_reference(),
            historical.authored_reference()
        );
    }
    let optional = DependencyOccurrence::new(
        RepresentationId::new(),
        0,
        Dependency::new(
            None,
            DependencyKind::new("unknown:optional").unwrap(),
            DependencyTarget::Asset(AssetId::new()),
            None,
            false,
            "",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(ActivityPathSegment::new(0, optional).is_err());
}

#[test]
fn contradictory_path_evidence_and_unchecked_segment_bodies_are_rejected() {
    let header = ActivityPathHeader::new(
        0,
        ActivityPathStatus::Recorded,
        RepresentationId::new(),
        1,
        1,
    )
    .unwrap();
    let original: serde_json::Value =
        serde_json::from_slice(&header.document().canonical_bytes().unwrap()).unwrap();
    for (field, value) in [
        ("position", serde_json::json!(u64::MAX.to_string())),
        ("status", serde_json::json!("future")),
        ("status", serde_json::json!("unresolved")),
        ("status", serde_json::json!("depth-truncated")),
        ("segment_count", serde_json::json!("0")),
        ("segment_count", serde_json::json!("65")),
        ("fingerprint_count", serde_json::json!(u64::MAX.to_string())),
        ("subject_representation_id", serde_json::json!("invalid")),
        ("unknown", serde_json::Value::Null),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        assert!(
            Document::parse(&serde_json::to_vec(&changed).unwrap(), Limits::default())
                .and_then(|document| ActivityPathHeader::from_document(&document))
                .is_err()
        );
    }
    let historical = ArtifactDependencyPathSegment::new(
        RepresentationId::new(),
        0,
        None,
        DependencyKind::new("unknown:exact").unwrap(),
        DependencyTarget::Asset(AssetId::new()),
        None,
        String::new(),
    );
    assert!(ActivityPathSegment::from_segment(64, &historical).is_err());
    let segment = ActivityPathSegment::from_segment(0, &historical).unwrap();
    let mut changed: serde_json::Value =
        serde_json::from_slice(&segment.document().unwrap().canonical_bytes().unwrap()).unwrap();
    changed["occurrence"]["dependency"]["required"] = false.into();
    assert!(
        ActivityPathSegment::from_document(
            &Document::parse(&serde_json::to_vec(&changed).unwrap(), Limits::default()).unwrap()
        )
        .is_err()
    );
}
