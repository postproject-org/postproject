//! Absent historical evidence differs from explicitly empty authored snapshots.

use postproject_core::{ActivityId, ActivityRole, RepresentationId};
use postproject_protocol::{ActivityEdgeHeader, ActivityEdgeSide, Document, Limits};

fn edge(side: ActivityEdgeSide, position: u64) -> ActivityEdgeHeader {
    ActivityEdgeHeader::new(
        ActivityId::new(),
        side,
        position,
        RepresentationId::new(),
        Some(ActivityRole::new("unknown:ExAct").unwrap()),
    )
    .unwrap()
}

#[test]
fn edges_preserve_legacy_absence_empty_observations_and_exact_counts() {
    for side in [ActivityEdgeSide::Input, ActivityEdgeSide::Output] {
        for position in [0, 99_999] {
            let absent = edge(side, position);
            let empty = absent
                .clone()
                .with_snapshot(9_007_199_254_740_993, 0)
                .unwrap();
            let observed = absent
                .clone()
                .with_snapshot(i64::MAX as u64, 9_007_199_254_740_993)
                .unwrap();
            assert_ne!(absent, empty);
            for header in [absent, empty, observed] {
                let bytes = header.document().canonical_bytes().unwrap();
                let restored = ActivityEdgeHeader::from_document(
                    &Document::parse(&bytes, Limits::default()).unwrap(),
                )
                .unwrap();
                assert_eq!(restored, header);
                assert_eq!(restored.activity_id(), header.activity_id());
                assert_eq!(restored.representation_id(), header.representation_id());
                assert_eq!(restored.role().unwrap().as_str(), "unknown:ExAct");
            }
        }
    }
    let input = edge(ActivityEdgeSide::Input, 0)
        .with_snapshot(1, 0)
        .unwrap();
    let empty = input.clone().with_dependency_snapshot(0).unwrap();
    assert!(!input.has_dependency_snapshot());
    assert!(empty.has_dependency_snapshot());
    assert_eq!(empty.dependency_path_count(), 0);
    assert_ne!(empty, input);
    let populated = input.with_dependency_snapshot(i64::MAX as u64).unwrap();
    assert_eq!(
        ActivityEdgeHeader::from_document(&populated.document()).unwrap(),
        populated
    );
    let unqualified = ActivityEdgeHeader::new(
        ActivityId::new(),
        ActivityEdgeSide::Output,
        0,
        RepresentationId::new(),
        None,
    )
    .unwrap();
    assert_eq!(
        ActivityEdgeHeader::from_document(&unqualified.document()).unwrap(),
        unqualified
    );
}

#[test]
fn malformed_edges_reject_contradictory_snapshot_and_side_facts() {
    assert!(
        edge(ActivityEdgeSide::Output, 0)
            .with_dependency_snapshot(0)
            .is_err()
    );
    assert!(
        edge(ActivityEdgeSide::Input, 0)
            .with_snapshot(0, 0)
            .is_err()
    );
    assert!(
        edge(ActivityEdgeSide::Input, 0)
            .with_snapshot(1, u64::MAX)
            .is_err()
    );
    let document: serde_json::Value = serde_json::from_slice(
        &edge(ActivityEdgeSide::Input, 0)
            .document()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    for (key, value) in [
        ("side", serde_json::json!("future")),
        ("position", serde_json::json!("100000")),
        ("position", serde_json::json!("00")),
        ("role", serde_json::json!("")),
        ("fingerprint_count", serde_json::json!("1")),
        ("snapshot_revision_sequence", serde_json::json!("0")),
        ("dependency_path_count", serde_json::json!("1")),
        ("dependency_snapshot", serde_json::json!("true")),
        ("unknown", serde_json::Value::Null),
    ] {
        let mut changed = document.clone();
        changed[key] = value;
        assert!(
            Document::parse(&serde_json::to_vec(&changed).unwrap(), Limits::default())
                .and_then(|document| ActivityEdgeHeader::from_document(&document))
                .is_err()
        );
    }
    let mut output = document;
    output["side"] = "output".into();
    output["dependency_snapshot"] = true.into();
    assert!(
        ActivityEdgeHeader::from_document(
            &Document::parse(&serde_json::to_vec(&output).unwrap(), Limits::default()).unwrap()
        )
        .is_err()
    );
}
