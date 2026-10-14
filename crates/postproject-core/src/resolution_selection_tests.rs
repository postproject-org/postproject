use super::*;
use crate::{ProductionId, ResourceMember, ResourceRole};

fn base() -> DecisionBase {
    DecisionBase::new(ProductionId::new(), None, 0).unwrap()
}

fn resolved(id: ResourceId, kind: EvidenceKind) -> ResourceResolution {
    ResourceResolution::new(
        id,
        ResourceResolutionState::ResolvedExact,
        vec![
            ResolutionCandidate::new(
                "file:///moved.mov",
                Confidence::CERTAIN,
                vec![ResolutionEvidence::new(kind, None)],
            )
            .unwrap(),
        ],
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn confidence_and_cardinality_do_not_replace_complete_verification() {
    let id = ResourceId::new();
    for kind in [
        EvidenceKind::KnownLocatorAvailable,
        EvidenceKind::FileSizeMatch,
        EvidenceKind::PartialFingerprintMatch,
        EvidenceKind::FingerprintNotVerified,
    ] {
        let result = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &ContentStructure::single_resource(id),
            vec![resolved(id, kind)],
        )
        .unwrap();
        assert_eq!(result.availability(), RepresentationAvailability::Online);
        assert_eq!(
            result.select_verified(base()).unwrap_err().kind(),
            ErrorKind::Conflict
        );
    }
    for kind in [
        EvidenceKind::FullHashMatch,
        EvidenceKind::ExactFingerprintMatch,
    ] {
        let result = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &ContentStructure::single_resource(id),
            vec![resolved(id, kind)],
        )
        .unwrap();
        let decision = base();
        let selection = result.select_verified(decision).unwrap();
        assert_eq!(selection.base(), decision);
        assert_eq!(selection.representation_id(), result.representation_id());
        assert_eq!(selection.locations()[0].0, id);
        assert_eq!(selection.locations()[0].1.uri(), "file:///moved.mov");
    }
}

#[test]
fn incomplete_search_and_contradictory_candidate_evidence_are_rejected() {
    let id = ResourceId::new();
    for kind in [
        EvidenceKind::SearchTruncated,
        EvidenceKind::DiscoveryError,
        EvidenceKind::FingerprintMismatch,
        EvidenceKind::FingerprintNotVerified,
        EvidenceKind::PartialFingerprintMatch,
        EvidenceKind::ConflictingCandidate,
    ] {
        let mut resource = resolved(id, EvidenceKind::FullHashMatch);
        resource.candidates[0]
            .evidence
            .push(ResolutionEvidence::new(kind, None));
        let result = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &ContentStructure::single_resource(id),
            vec![resource],
        )
        .unwrap();
        assert!(result.select_verified(base()).is_err(), "{kind:?}");
    }
    for kind in [EvidenceKind::SearchTruncated, EvidenceKind::DiscoveryError] {
        let mut resource = resolved(id, EvidenceKind::FullHashMatch);
        resource.evidence.push(ResolutionEvidence::new(kind, None));
        let result = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &ContentStructure::single_resource(id),
            vec![resource],
        )
        .unwrap();
        assert!(result.select_verified(base()).is_err());
    }
}

#[test]
fn optional_members_remain_diagnostics_and_required_gaps_block_selection() {
    let required = ResourceId::new();
    let optional = ResourceId::new();
    let structure = ContentStructure::package(vec![
        ResourceMember::new(
            required,
            ResourceRole::new("org.example:essence").unwrap(),
            true,
        ),
        ResourceMember::new(
            optional,
            ResourceRole::new("org.example:sidecar").unwrap(),
            false,
        ),
    ])
    .unwrap();
    let absent = ResourceResolution::new(
        optional,
        ResourceResolutionState::Offline,
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let result = RepresentationResolution::aggregate(
        RepresentationId::new(),
        &structure,
        vec![resolved(required, EvidenceKind::FullHashMatch), absent],
    )
    .unwrap();
    let selection = result.select_verified(base()).unwrap();
    assert_eq!(selection.locations().len(), 1);
    assert_eq!(selection.locations()[0].0, required);
    assert_eq!(result.issues().len(), 1);
    let missing = ResourceResolution::new(
        required,
        ResourceResolutionState::Offline,
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let partial = RepresentationResolution::aggregate(
        RepresentationId::new(),
        &structure,
        vec![missing, resolved(optional, EvidenceKind::FullHashMatch)],
    )
    .unwrap();
    assert!(partial.select_verified(base()).is_err());
}
