//! Typed dependency occurrences retain open-world spelling and order.

use postproject_core::{
    AssetId, Dependency, DependencyKind, DependencyTarget, RepresentationId, ResourceId,
};
use postproject_protocol::{DependencyOccurrence, Document, Limits};

fn dependency(
    target: DependencyTarget,
    resolved: Option<RepresentationId>,
    authored: &str,
) -> Dependency {
    Dependency::new(
        Some(ResourceId::new()),
        DependencyKind::new("unknown:ExAct").unwrap(),
        target,
        resolved,
        true,
        authored,
    )
    .unwrap()
}

#[test]
fn occurrences_preserve_floating_pinned_and_repeated_exact_facts() {
    let source = RepresentationId::new();
    for target in [
        DependencyTarget::Asset(AssetId::new()),
        DependencyTarget::Representation(RepresentationId::new()),
    ] {
        for authored in ["", "  名/EXACT%2f\n\"\\  "] {
            let resolved = matches!(target, DependencyTarget::Asset(_)).then(RepresentationId::new);
            let dependency = dependency(target, resolved, authored);
            for position in [0, 1, 99_999] {
                let occurrence =
                    DependencyOccurrence::new(source, position, dependency.clone()).unwrap();
                let bytes = occurrence.document().unwrap().canonical_bytes().unwrap();
                let restored = DependencyOccurrence::from_document(
                    &Document::parse(&bytes, Limits::default()).unwrap(),
                )
                .unwrap();
                assert_eq!(restored, occurrence);
                assert_eq!(restored.position(), position);
                assert_eq!(restored.source_representation_id(), source);
                assert_eq!(restored.dependency().authored_reference(), authored);
            }
        }
    }
    let unresolved = Dependency::new(
        None,
        DependencyKind::new("unknown:floating").unwrap(),
        DependencyTarget::Asset(AssetId::new()),
        None,
        false,
        "",
    )
    .unwrap();
    let occurrence = DependencyOccurrence::new(source, 0, unresolved).unwrap();
    assert_eq!(
        DependencyOccurrence::from_document(&occurrence.document().unwrap()).unwrap(),
        occurrence
    );
}

#[test]
fn malformed_occurrences_cannot_bypass_domain_validation() {
    let occurrence = DependencyOccurrence::new(
        RepresentationId::new(),
        0,
        dependency(
            DependencyTarget::Representation(RepresentationId::new()),
            None,
            "authored",
        ),
    )
    .unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&occurrence.document().unwrap().canonical_bytes().unwrap()).unwrap();
    let mut alternatives = Vec::new();
    for (key, value) in [
        (
            "resolved_representation_id",
            serde_json::json!(RepresentationId::new().to_string()),
        ),
        ("required", serde_json::json!("true")),
        ("authored_reference", serde_json::json!("bad\0")),
        ("authored_reference", serde_json::json!("x".repeat(4097))),
        ("kind", serde_json::json!("")),
        ("unknown", serde_json::json!(null)),
    ] {
        let mut altered = document.clone();
        altered["dependency"][key] = value;
        alternatives.push(altered);
    }
    for (key, value) in [
        ("position", "100000"),
        ("position", "-1"),
        ("position", "00"),
        ("unknown", "ignored"),
    ] {
        let mut altered = document.clone();
        altered[key] = value.into();
        alternatives.push(altered);
    }
    for altered in alternatives {
        assert!(
            DependencyOccurrence::from_document(
                &Document::parse(&serde_json::to_vec(&altered).unwrap(), Limits::default())
                    .unwrap()
            )
            .is_err()
        );
    }
}
