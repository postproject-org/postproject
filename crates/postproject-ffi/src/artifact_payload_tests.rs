//! Kind selection must precede borrowed artifact payload access.

use postproject_core::{
    ActivityId, ArtifactDependencyIssue, ArtifactEdgeKind, ArtifactEvaluation,
    ArtifactKnowledgeReason, ArtifactKnowledgeState, ArtifactReproducibilityIssue,
    ArtifactReproducibilityReport, ArtifactTraversalLimitKind, RepresentationId,
};

use crate::*;

fn reasons() -> Vec<ArtifactKnowledgeReason> {
    let activity_id = ActivityId::new();
    let representation_id = RepresentationId::new();
    let input_representation_id = RepresentationId::new();
    let edge = ArtifactEdgeKind::Input;
    vec![
        ArtifactKnowledgeReason::ProducingActivityMissing { representation_id },
        ArtifactKnowledgeReason::ProducingActivityAmbiguous {
            representation_id,
            activity_count: 2,
        },
        ArtifactKnowledgeReason::SnapshotAbsent {
            activity_id,
            representation_id,
            edge,
        },
        ArtifactKnowledgeReason::FingerprintEvidenceMissing {
            activity_id,
            representation_id,
            edge,
            algorithm: None,
            version: None,
            snapshot_value: None,
            current_value: None,
        },
        ArtifactKnowledgeReason::FingerprintChanged {
            activity_id,
            representation_id,
            edge,
            algorithm: "foreign_digest".into(),
            version: 7,
            snapshot_value: vec![1],
            current_value: vec![2],
        },
        ArtifactKnowledgeReason::FingerprintRecomputationPending {
            activity_id,
            representation_id,
            edge,
        },
        ArtifactKnowledgeReason::UpstreamNotCurrent {
            representation_id,
            state: ArtifactKnowledgeState::Stale,
        },
        ArtifactKnowledgeReason::TraversalTruncated {
            representation_id,
            limit: ArtifactTraversalLimitKind::Depth,
        },
        ArtifactKnowledgeReason::DependencySnapshotAbsent {
            activity_id,
            representation_id,
        },
        ArtifactKnowledgeReason::DependencyKnowledgeIncomplete {
            activity_id,
            input_representation_id,
            subject_representation_id: representation_id,
            path: Vec::new(),
            issue: ArtifactDependencyIssue::NeedsExtraction,
        },
        ArtifactKnowledgeReason::DependencyPathChanged {
            activity_id,
            input_representation_id,
            path: Vec::new(),
        },
        ArtifactKnowledgeReason::DependencyFingerprintChanged {
            activity_id,
            input_representation_id,
            representation_id,
            path: Vec::new(),
            algorithm: "foreign_digest".into(),
            version: 7,
            snapshot_value: vec![1],
            current_value: vec![2],
        },
        ArtifactKnowledgeReason::DependencyFingerprintRecomputationPending {
            activity_id,
            input_representation_id,
            representation_id,
            path: Vec::new(),
        },
        ArtifactKnowledgeReason::DependencyFingerprintEvidenceMissing {
            activity_id,
            input_representation_id,
            representation_id,
            path: Vec::new(),
            algorithm: None,
            version: None,
            snapshot_value: None,
            current_value: None,
        },
    ]
}

#[test]
fn every_artifact_reason_refuses_other_payload_kinds_and_clears_outputs() {
    let evaluation = PpArtifactEvaluation::new(&ArtifactEvaluation::new(
        RepresentationId::new(),
        ArtifactKnowledgeState::Indeterminate,
        reasons(),
        1,
        false,
    ))
    .unwrap();
    for index in 0..14 {
        let mut kind = 0;
        let mut error = ptr::null_mut();
        // SAFETY: Stack-owned evaluation and outputs remain live for the call.
        assert_eq!(
            unsafe {
                pp_artifact_evaluation_get_reason_kind(
                    &raw const evaluation,
                    index,
                    &raw mut kind,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(kind, u32::try_from(index).unwrap() + 1);
        for selected in 0..=15 {
            let mut reason = zero_artifact_reason();
            reason.kind = u32::MAX;
            reason.activity_count = u32::MAX;
            // SAFETY: Evaluation and writable outputs are live; wrong kinds are intentionally tested.
            let status = unsafe {
                pp_artifact_evaluation_get_reason(
                    &raw const evaluation,
                    index,
                    selected,
                    &raw mut reason,
                    &raw mut error,
                )
            };
            if selected == kind {
                assert_eq!(status, PP_OK);
                assert_eq!(reason.kind, kind);
                assert!(error.is_null());
            } else {
                assert_eq!(status, PP_ERROR_INVALID_ARGUMENT);
                assert_eq!((reason.kind, reason.activity_count), (0, 0));
                assert_eq!(reason.representation_id.bytes, [0; 16]);
                assert!(reason.dependency_path.is_null());
                assert!(reason.fingerprint_algorithm.is_null());
                assert!(reason.snapshot_value.is_null());
                assert!(reason.current_value.is_null());
                // SAFETY: The failed call transfers exactly one live error ownership reference.
                unsafe { pp_error_release(error) };
            }
        }
    }
}

#[test]
fn every_reproducibility_issue_refuses_other_payload_kinds_and_clears_outputs() {
    let activity_id = ActivityId::new();
    let representation_id = RepresentationId::new();
    let report = PpArtifactReproducibility::new(&ArtifactReproducibilityReport::new(
        representation_id,
        None,
        None,
        vec![
            ArtifactReproducibilityIssue::ProducingActivityMissing,
            ArtifactReproducibilityIssue::ProducingActivityAmbiguous { activity_count: 2 },
            ArtifactReproducibilityIssue::ToolIdentityMissing { activity_id },
            ArtifactReproducibilityIssue::ParametersMissing { activity_id },
            ArtifactReproducibilityIssue::InputRepresentationMissing {
                activity_id,
                representation_id,
            },
        ],
    ))
    .unwrap();
    for index in 0..5 {
        let mut kind = 0;
        let mut error = ptr::null_mut();
        // SAFETY: Stack-owned report and outputs remain live for the call.
        assert_eq!(
            unsafe {
                pp_artifact_reproducibility_get_issue_kind(
                    &raw const report,
                    index,
                    &raw mut kind,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(kind, u32::try_from(index).unwrap() + 1);
        for selected in 0..=6 {
            let mut issue = zero_reproducibility_issue();
            issue.kind = u32::MAX;
            issue.activity_count = u32::MAX;
            // SAFETY: Report and writable outputs are live; wrong kinds are intentionally tested.
            let status = unsafe {
                pp_artifact_reproducibility_get_issue(
                    &raw const report,
                    index,
                    selected,
                    &raw mut issue,
                    &raw mut error,
                )
            };
            if selected == kind {
                assert_eq!(status, PP_OK);
                assert_eq!(issue.kind, kind);
                assert!(error.is_null());
            } else {
                assert_eq!(status, PP_ERROR_INVALID_ARGUMENT);
                assert_eq!((issue.kind, issue.activity_count), (0, 0));
                assert_eq!(issue.activity_id.bytes, [0; 16]);
                assert_eq!(issue.representation_id.bytes, [0; 16]);
                // SAFETY: The failed call transfers exactly one live error ownership reference.
                unsafe { pp_error_release(error) };
            }
        }
    }
}
