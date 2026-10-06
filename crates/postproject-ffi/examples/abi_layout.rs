//! Generated implementation layout probe; do not edit manually.
use postproject::{
    PpActivityEdge, PpArtifactDependencyPathSegment, PpArtifactReason,
    PpArtifactReproducibilityIssue, PpAssetId, PpCommitReceipt, PpDecisionBase, PpDependency,
    PpDependencyMatch, PpFileResourceInput, PpJob, PpJobClaim, PpJobCompletion, PpJobId,
    PpLocatorId, PpMediaRootId, PpObjectRef, PpProductionId, PpRevisionEvent, PpRevisionId,
    PpSequenceNaming, PpTransactionConflict, PpTransactionId, PpUuid,
};
use std::mem::{align_of, offset_of, size_of};

macro_rules! layout {
    ($ty:ty, $name:literal, $($field:ident),+) => {{
        println!(concat!($name, ".size={}"), size_of::<$ty>());
        println!(concat!($name, ".alignment={}"), align_of::<$ty>());
        $(println!(concat!($name, ".", stringify!($field), "={}"),
                   offset_of!($ty, $field));)+
    }};
}

// One generated invocation per public struct keeps the probe auditable.
#[allow(clippy::too_many_lines)]
fn main() {
    layout!(PpUuid, "pp_uuid_t", bytes);
    layout!(PpProductionId, "pp_production_id_t", bytes);
    layout!(PpRevisionId, "pp_revision_id_t", bytes);
    layout!(PpTransactionId, "pp_transaction_id_t", bytes);
    layout!(PpAssetId, "pp_asset_id_t", bytes);
    layout!(PpMediaRootId, "pp_media_root_id_t", bytes);
    layout!(PpLocatorId, "pp_locator_id_t", bytes);
    layout!(PpJobId, "pp_job_id_t", bytes);
    layout!(
        PpCommitReceipt,
        "pp_commit_receipt_t",
        production_id,
        outcome,
        revision_id,
        revision_sequence
    );
    layout!(
        PpDecisionBase,
        "pp_decision_base_t",
        production_id,
        has_revision,
        revision_id,
        revision_sequence
    );
    layout!(PpObjectRef, "pp_object_ref_t", kind, id);
    layout!(
        PpTransactionConflict,
        "pp_transaction_conflict_t",
        kind,
        target,
        media_root_id,
        namespace_name,
        local_name,
        qualifier,
        version,
        has_base_revision,
        base_revision_id,
        base_revision_sequence,
        superseding_revision_id,
        superseding_revision_sequence
    );
    layout!(PpDependencyMatch, "pp_dependency_match_t", target, depth);
    layout!(
        PpRevisionEvent,
        "pp_revision_event_t",
        kind,
        position,
        asset_id,
        representation_id,
        resource_id,
        locator_id,
        media_root_id,
        activity_id,
        job_id,
        target,
        structural_position,
        enabled,
        identifier_scheme,
        identifier_value,
        identifier_qualifier,
        vocabulary,
        property,
        activity_kind,
        role,
        fingerprint_algorithm,
        fingerprint_version
    );
    layout!(
        PpActivityEdge,
        "pp_activity_edge_t",
        representation_id,
        role
    );
    layout!(
        PpJob,
        "pp_job_t",
        id,
        kind,
        output_asset_id,
        output_representation_kind,
        target_root,
        state,
        input_count,
        claim_id,
        claim_expires_at_unix_micros,
        claim_tool_name,
        claim_tool_version,
        claim_tool_uri,
        claim_agent_name,
        claim_agent_identifier_scheme,
        claim_agent_identifier_value,
        claim_agent_identifier_qualifier,
        completion_activity_id,
        completion_representation_id,
        failure_diagnostic
    );
    layout!(
        PpJobClaim,
        "pp_job_claim_t",
        id,
        expires_at_unix_micros,
        tool_name,
        tool_version,
        tool_uri,
        agent_name,
        agent_identifier_scheme,
        agent_identifier_value,
        agent_identifier_qualifier
    );
    layout!(
        PpJobCompletion,
        "pp_job_completion_t",
        activity_id,
        representation_id
    );
    layout!(
        PpDependency,
        "pp_dependency_t",
        has_source_resource,
        source_resource_id,
        kind,
        target,
        has_resolved_representation,
        resolved_representation_id,
        required,
        authored_reference
    );
    layout!(
        PpArtifactDependencyPathSegment,
        "pp_artifact_dependency_path_segment_t",
        source_representation_id,
        dependency_position,
        has_source_resource,
        source_resource_id,
        kind,
        target,
        has_resolved_representation,
        resolved_representation_id,
        authored_reference
    );
    layout!(
        PpArtifactReason,
        "pp_artifact_reason_t",
        kind,
        activity_id,
        representation_id,
        input_representation_id,
        edge_kind,
        upstream_state,
        traversal_limit,
        activity_count,
        dependency_issue,
        dependency_path,
        dependency_path_length,
        fingerprint_algorithm,
        fingerprint_version,
        has_snapshot_value,
        snapshot_value,
        snapshot_value_length,
        has_current_value,
        current_value,
        current_value_length
    );
    layout!(
        PpArtifactReproducibilityIssue,
        "pp_artifact_reproducibility_issue_t",
        kind,
        activity_id,
        representation_id,
        activity_count
    );
    layout!(
        PpFileResourceInput,
        "pp_file_resource_input_t",
        path,
        role,
        required
    );
    layout!(
        PpSequenceNaming,
        "pp_sequence_naming_t",
        prefix,
        suffix,
        padding
    );
}
