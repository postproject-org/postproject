//! Generated implementation layout probe; do not edit manually.
use postproject::{
    PpActivityEdge, PpActivityId, PpActivityParameter, PpActivityRecipe,
    PpArtifactDependencyPathSegment, PpArtifactReason, PpArtifactReproducibilityIssue, PpAssetId,
    PpClientId, PpCommitReceipt, PpDecisionBase, PpDependency, PpDependencyMatch, PpEditContext,
    PpExchangeLimits, PpExchangePosition, PpExchangeState, PpFileResourceInput, PpHistoryId, PpJob,
    PpJobClaim, PpJobCompletion, PpJobId, PpLocatorId, PpMediaRootId, PpObjectRef,
    PpOutputIdentity, PpOutputSource, PpPrivateLeaseToken, PpProductionId, PpProtocolBase,
    PpRelinkLocation, PpRelinkSelection, PpRepresentationId, PpRequestId, PpRequestIdentity,
    PpResolutionConfig, PpResourceId, PpRevisionEvent, PpRevisionId, PpRootMapping,
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
    layout!(PpActivityId, "pp_activity_id_t", bytes);
    layout!(PpRepresentationId, "pp_representation_id_t", bytes);
    layout!(PpResourceId, "pp_resource_id_t", bytes);
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
    layout!(PpHistoryId, "pp_history_id_t", bytes);
    layout!(PpClientId, "pp_client_id_t", bytes);
    layout!(PpRequestId, "pp_request_id_t", bytes);
    layout!(PpProtocolBase, "pp_protocol_base_t", decision, history_id);
    layout!(PpExchangePosition, "pp_exchange_position_t", base, digest);
    layout!(
        PpExchangeLimits,
        "pp_exchange_limits_t",
        file_bytes,
        disk_bytes,
        frames,
        document_bytes,
        document_depth,
        document_nodes
    );
    layout!(
        PpExchangeState,
        "pp_exchange_state_t",
        protocol_version,
        role,
        supported_features,
        observed_base,
        floor,
        has_head,
        head,
        limits
    );
    layout!(
        PpRequestIdentity,
        "pp_request_identity_t",
        production_id,
        history_id,
        client_id,
        request_id
    );
    layout!(
        PpPrivateLeaseToken,
        "pp_private_lease_token_t",
        bytes,
        length
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
        PpActivityParameter,
        "pp_activity_parameter_t",
        vocabulary,
        property,
        value
    );
    layout!(
        PpActivityRecipe,
        "pp_activity_recipe_t",
        kind,
        inputs,
        input_count,
        started_at_unix_micros,
        finished_at_unix_micros,
        tool_name,
        tool_version,
        tool_uri,
        agent_name,
        agent_identifier_scheme,
        agent_identifier_value,
        agent_identifier_qualifier,
        parameters,
        parameter_count
    );
    layout!(
        PpOutputSource,
        "pp_output_source_t",
        asset_id,
        kind,
        path,
        source,
        role,
        display_name
    );
    layout!(
        PpOutputIdentity,
        "pp_output_identity_t",
        asset_id,
        representation_id
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
    layout!(
        PpRelinkLocation,
        "pp_relink_location_t",
        resource_id,
        uri,
        media_root,
        has_sequence_naming,
        sequence_naming
    );
    layout!(
        PpRelinkSelection,
        "pp_relink_selection_t",
        base,
        representation_id,
        locations,
        location_count
    );
    layout!(
        PpEditContext,
        "pp_edit_context_t",
        origin_name,
        origin_version,
        origin_uri,
        message
    );
    layout!(PpRootMapping, "pp_root_mapping_t", name, directory);
    layout!(
        PpResolutionConfig,
        "pp_resolution_config_t",
        root_mappings,
        root_mapping_count,
        search_directories,
        search_directory_count,
        verification,
        max_depth,
        max_entries_per_directory,
        cancel_token
    );
}
