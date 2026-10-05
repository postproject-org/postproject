"""Generated from include/postproject/postproject.h; do not edit manually."""

from __future__ import annotations

import ctypes


class Production(ctypes.Structure):
    pass


class ReadSession(ctypes.Structure):
    pass


class Transaction(ctypes.Structure):
    pass


class AssetSet(ctypes.Structure):
    pass


class MediaRootSet(ctypes.Structure):
    pass


class RepresentationSet(ctypes.Structure):
    pass


class ResolutionSet(ctypes.Structure):
    pass


class ExternalIdentifierSet(ctypes.Structure):
    pass


class ObjectRefSet(ctypes.Structure):
    pass


class ObjectQuerySet(ctypes.Structure):
    pass


class LocatorQuerySet(ctypes.Structure):
    pass


class KnownMediaSet(ctypes.Structure):
    pass


class MetadataSet(ctypes.Structure):
    pass


class MetadataValue(ctypes.Structure):
    pass


class MetadataInput(ctypes.Structure):
    pass


class ActivitySet(ctypes.Structure):
    pass


class JobSet(ctypes.Structure):
    pass


class RegenerationPlanSet(ctypes.Structure):
    pass


class DependencySet(ctypes.Structure):
    pass


class DependencyQuerySet(ctypes.Structure):
    pass


class ArtifactEvaluation(ctypes.Structure):
    pass


class ArtifactReproducibility(ctypes.Structure):
    pass


class RevisionSet(ctypes.Structure):
    pass


class RevisionEventSet(ctypes.Structure):
    pass


class RevisionWaiter(ctypes.Structure):
    pass


class Fingerprint(ctypes.Structure):
    pass


class CancelToken(ctypes.Structure):
    pass


class ResolutionOptions(ctypes.Structure):
    pass


class MediaSource(ctypes.Structure):
    pass


class Error(ctypes.Structure):
    pass


class Uuid(ctypes.Structure):
    pass


class ProductionId(ctypes.Structure):
    pass


class CommitReceipt(ctypes.Structure):
    pass


class DecisionBase(ctypes.Structure):
    pass


class ObjectRef(ctypes.Structure):
    pass


class TransactionConflict(ctypes.Structure):
    pass


class DependencyMatch(ctypes.Structure):
    pass


class RevisionEvent(ctypes.Structure):
    pass


class ActivityEdge(ctypes.Structure):
    pass


class Job(ctypes.Structure):
    pass


class Dependency(ctypes.Structure):
    pass


class ArtifactDependencyPathSegment(ctypes.Structure):
    pass


class ArtifactReason(ctypes.Structure):
    pass


class ArtifactReproducibilityIssue(ctypes.Structure):
    pass


class FileResourceInput(ctypes.Structure):
    pass


class SequenceNaming(ctypes.Structure):
    pass


CommitOutcome = ctypes.c_uint32
ObjectKind = ctypes.c_uint32
RepresentationKind = ctypes.c_uint32
JobState = ctypes.c_uint32
ContentStructureKind = ctypes.c_uint32
LocatorAvailability = ctypes.c_uint32
RevisionEventKind = ctypes.c_uint32
RevisionWaitResult = ctypes.c_uint32
ArtifactKnowledgeState = ctypes.c_uint32
ArtifactEdgeKind = ctypes.c_uint32
ArtifactReasonKind = ctypes.c_uint32
ArtifactDependencyIssue = ctypes.c_uint32
DependencySetStatus = ctypes.c_uint32
ArtifactTraversalLimit = ctypes.c_uint32
ArtifactReproducibilityIssueKind = ctypes.c_uint32
MetadataValueKind = ctypes.c_uint32
ConflictKeyKind = ctypes.c_uint32
ErrorCode = ctypes.c_uint32
RepresentationAvailability = ctypes.c_uint32
ResourceResolutionState = ctypes.c_uint32
AvailabilityIssueKind = ctypes.c_uint32
EvidenceKind = ctypes.c_uint32
VerificationMode = ctypes.c_uint32
ContentVerification = ctypes.c_uint32
ContentObservation = ctypes.c_uint32


PP_COMMIT_NO_CHANGE = 0
PP_COMMIT_REVISION_CREATED = 1
PP_OBJECT_PRODUCTION = 1
PP_OBJECT_ASSET = 2
PP_OBJECT_REPRESENTATION = 3
PP_OBJECT_RESOURCE = 4
PP_OBJECT_ACTIVITY = 5
PP_OBJECT_JOB = 6
PP_REPRESENTATION_ORIGINAL = 1
PP_REPRESENTATION_PROXY = 2
PP_REPRESENTATION_OPTIMIZED = 3
PP_REPRESENTATION_DERIVED = 4
PP_JOB_REQUESTED = 1
PP_JOB_CLAIMED = 2
PP_JOB_SUCCEEDED = 3
PP_JOB_FAILED = 4
PP_JOB_CANCELLED = 5
PP_CONTENT_SINGLE_RESOURCE = 1
PP_CONTENT_IMAGE_SEQUENCE = 2
PP_CONTENT_ORDERED_PARTS = 3
PP_CONTENT_PACKAGE = 4
PP_LOCATOR_UNKNOWN = 1
PP_LOCATOR_ONLINE = 2
PP_LOCATOR_OFFLINE = 3
PP_REVISION_ASSET_IMPORTED = 1
PP_REVISION_REPRESENTATION_ADDED = 2
PP_REVISION_RESOURCE_ADDED = 3
PP_REVISION_REPRESENTATION_RESOURCE_ADDED = 4
PP_REVISION_LOCATOR_ADDED = 5
PP_REVISION_MEDIA_ROOT_ADDED = 6
PP_REVISION_EXTERNAL_IDENTIFIER_ADDED = 7
PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED = 8
PP_REVISION_METADATA_ADDED_OR_REPLACED = 9
PP_REVISION_METADATA_REMOVED = 10
PP_REVISION_ACTIVITY_CREATED = 11
PP_REVISION_ACTIVITY_INPUT_ADDED = 12
PP_REVISION_ACTIVITY_OUTPUT_ADDED = 13
PP_REVISION_LOCATOR_RETIRED = 14
PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED = 15
PP_REVISION_MEDIA_ROOT_REMOVED = 16
PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED = 17
PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED = 18
PP_REVISION_DEPENDENCY_SET_RECORDED = 19
PP_REVISION_JOB_REQUESTED = 20
PP_REVISION_JOB_CLAIMED = 21
PP_REVISION_JOB_CLAIM_RENEWED = 22
PP_REVISION_JOB_CLAIM_RELEASED = 23
PP_REVISION_JOB_SUCCEEDED = 24
PP_REVISION_JOB_FAILED = 25
PP_REVISION_JOB_CANCELLED = 26
PP_REVISION_WAIT_REVISIONS = 1
PP_REVISION_WAIT_TIMED_OUT = 2
PP_REVISION_WAIT_CLOSED = 3
PP_REVISION_WAIT_CANCELLED = 4
PP_REVISION_WAIT_MAX_TIMEOUT_MILLIS = 60000
PP_ARTIFACT_CURRENT = 1
PP_ARTIFACT_STALE = 2
PP_ARTIFACT_INDETERMINATE = 3
PP_ARTIFACT_DIVERGED = 4
PP_ARTIFACT_EDGE_INPUT = 1
PP_ARTIFACT_EDGE_OUTPUT = 2
PP_ARTIFACT_REASON_PRODUCING_ACTIVITY_MISSING = 1
PP_ARTIFACT_REASON_PRODUCING_ACTIVITY_AMBIGUOUS = 2
PP_ARTIFACT_REASON_SNAPSHOT_ABSENT = 3
PP_ARTIFACT_REASON_FINGERPRINT_EVIDENCE_MISSING = 4
PP_ARTIFACT_REASON_FINGERPRINT_CHANGED = 5
PP_ARTIFACT_REASON_FINGERPRINT_RECOMPUTATION_PENDING = 6
PP_ARTIFACT_REASON_UPSTREAM_NOT_CURRENT = 7
PP_ARTIFACT_REASON_TRAVERSAL_TRUNCATED = 8
PP_ARTIFACT_REASON_DEPENDENCY_SNAPSHOT_ABSENT = 9
PP_ARTIFACT_REASON_DEPENDENCY_KNOWLEDGE_INCOMPLETE = 10
PP_ARTIFACT_REASON_DEPENDENCY_PATH_CHANGED = 11
PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_CHANGED = 12
PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_RECOMPUTATION_PENDING = 13
PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_EVIDENCE_MISSING = 14
PP_ARTIFACT_DEPENDENCY_NEEDS_EXTRACTION = 1
PP_ARTIFACT_DEPENDENCY_UNRESOLVED = 2
PP_ARTIFACT_DEPENDENCY_DEPTH_TRUNCATED = 3
PP_ARTIFACT_DEPENDENCY_REPRESENTATIONS_TRUNCATED = 4
PP_DEPENDENCY_SET_CURRENT = 1
PP_DEPENDENCY_SET_NEEDS_EXTRACTION = 2
PP_ARTIFACT_TRAVERSAL_DEPTH = 1
PP_ARTIFACT_TRAVERSAL_REPRESENTATIONS = 2
PP_ARTIFACT_REPRODUCIBILITY_PRODUCING_ACTIVITY_MISSING = 1
PP_ARTIFACT_REPRODUCIBILITY_PRODUCING_ACTIVITY_AMBIGUOUS = 2
PP_ARTIFACT_REPRODUCIBILITY_TOOL_IDENTITY_MISSING = 3
PP_ARTIFACT_REPRODUCIBILITY_PARAMETERS_MISSING = 4
PP_ARTIFACT_REPRODUCIBILITY_INPUT_REPRESENTATION_MISSING = 5
PP_METADATA_STRING = 1
PP_METADATA_LANG_STRING = 2
PP_METADATA_I64 = 3
PP_METADATA_U64 = 4
PP_METADATA_DECIMAL = 5
PP_METADATA_BOOL = 6
PP_METADATA_TIMESTAMP = 7
PP_METADATA_URI = 8
PP_METADATA_BYTES = 9
PP_METADATA_RATIONAL = 10
PP_METADATA_LIST = 11
PP_METADATA_STRUCT = 12
PP_METADATA_REFERENCE = 13
PP_CONFLICT_LOCATOR_SET = 1
PP_CONFLICT_METADATA_PROPERTY = 2
PP_CONFLICT_DEPENDENCY_SET = 3
PP_CONFLICT_MEDIA_ROOT = 4
PP_CONFLICT_EXTERNAL_IDENTIFIER = 5
PP_CONFLICT_RESOURCE_FINGERPRINT = 6
PP_CONFLICT_REPRESENTATION_FINGERPRINT = 7
PP_OK = 0
PP_ERROR_INVALID_ARGUMENT = 1
PP_ERROR_NOT_FOUND = 2
PP_ERROR_ALREADY_EXISTS = 3
PP_ERROR_IO = 4
PP_ERROR_STORAGE = 5
PP_ERROR_MIGRATION = 6
PP_ERROR_CONFLICT = 7
PP_ERROR_AMBIGUOUS_RESOLUTION = 8
PP_ERROR_FINGERPRINT = 9
PP_ERROR_UNSUPPORTED = 10
PP_ERROR_CANCELLED = 11
PP_ERROR_INTERNAL = 255
PP_AVAILABILITY_ONLINE = 1
PP_AVAILABILITY_PARTIAL = 2
PP_AVAILABILITY_OFFLINE = 3
PP_AVAILABILITY_AMBIGUOUS = 4
PP_AVAILABILITY_ERROR = 5
PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR = 1
PP_RESOURCE_RESOLVED_EXACT = 2
PP_RESOURCE_RESOLVED_PROBABLE = 3
PP_RESOURCE_OFFLINE = 4
PP_RESOURCE_AMBIGUOUS = 5
PP_RESOURCE_RESOLUTION_ERROR = 6
PP_AVAILABILITY_ISSUE_OFFLINE_RESOURCE = 1
PP_AVAILABILITY_ISSUE_AMBIGUOUS_RESOURCE = 2
PP_AVAILABILITY_ISSUE_RESOURCE_ERROR = 3
PP_AVAILABILITY_ISSUE_MISSING_FRAMES = 4
PP_EVIDENCE_KNOWN_LOCATOR_AVAILABLE = 1
PP_EVIDENCE_EXACT_FINGERPRINT_MATCH = 2
PP_EVIDENCE_FULL_HASH_MATCH = 3
PP_EVIDENCE_PARTIAL_FINGERPRINT_MATCH = 4
PP_EVIDENCE_FILE_SIZE_MATCH = 5
PP_EVIDENCE_FILE_NAME_MATCH = 6
PP_EVIDENCE_RELATIVE_PATH_SIMILARITY = 7
PP_EVIDENCE_MEDIA_ROOT_RELATION = 8
PP_EVIDENCE_CONFLICTING_CANDIDATE = 9
PP_EVIDENCE_DISCOVERY_ERROR = 10
PP_EVIDENCE_MEDIA_ROOT_UNMAPPED = 11
PP_EVIDENCE_MEDIA_ROOT_UNAVAILABLE = 12
PP_EVIDENCE_FINGERPRINT_MISMATCH = 13
PP_EVIDENCE_FINGERPRINT_NOT_VERIFIED = 14
PP_EVIDENCE_SEARCH_TRUNCATED = 15
PP_VERIFY_PRESENCE = 1
PP_VERIFY_CONTENT = 2
PP_CONTENT_MATCHES = 1
PP_CONTENT_DIFFERS = 2
PP_CONTENT_NOT_COMPARABLE = 3
PP_OBSERVATION_UNCHANGED = 1
PP_OBSERVATION_CHANGED = 2
PP_OBSERVATION_FIRST = 3


Uuid._fields_ = [
    ("bytes", ctypes.c_uint8 * 16),
]

ProductionId._fields_ = [
    ("bytes", ctypes.c_uint8 * 16),
]

CommitReceipt._fields_ = [
    ("production_id", ProductionId),
    ("outcome", CommitOutcome),
    ("revision_id", Uuid),
    ("revision_sequence", ctypes.c_uint64),
]

DecisionBase._fields_ = [
    ("production_id", ProductionId),
    ("has_revision", ctypes.c_uint8),
    ("revision_id", Uuid),
    ("revision_sequence", ctypes.c_uint64),
]

ObjectRef._fields_ = [
    ("kind", ObjectKind),
    ("id", Uuid),
]

TransactionConflict._fields_ = [
    ("kind", ConflictKeyKind),
    ("target", ObjectRef),
    ("namespace_name", ctypes.c_char_p),
    ("local_name", ctypes.c_char_p),
    ("qualifier", ctypes.c_char_p),
    ("version", ctypes.c_uint16),
    ("has_base_revision", ctypes.c_uint8),
    ("base_revision_id", Uuid),
    ("base_revision_sequence", ctypes.c_uint64),
    ("superseding_revision_id", Uuid),
    ("superseding_revision_sequence", ctypes.c_uint64),
]

DependencyMatch._fields_ = [
    ("target", ObjectRef),
    ("depth", ctypes.c_uint32),
]

RevisionEvent._fields_ = [
    ("kind", RevisionEventKind),
    ("position", ctypes.c_uint32),
    ("asset_id", Uuid),
    ("representation_id", Uuid),
    ("resource_id", Uuid),
    ("locator_id", Uuid),
    ("media_root_id", Uuid),
    ("activity_id", Uuid),
    ("job_id", Uuid),
    ("target", ObjectRef),
    ("structural_position", ctypes.c_uint32),
    ("enabled", ctypes.c_uint8),
    ("identifier_scheme", ctypes.c_char_p),
    ("identifier_value", ctypes.c_char_p),
    ("identifier_qualifier", ctypes.c_char_p),
    ("vocabulary", ctypes.c_char_p),
    ("property", ctypes.c_char_p),
    ("activity_kind", ctypes.c_char_p),
    ("role", ctypes.c_char_p),
    ("fingerprint_algorithm", ctypes.c_char_p),
    ("fingerprint_version", ctypes.c_uint16),
]

ActivityEdge._fields_ = [
    ("representation_id", Uuid),
    ("role", ctypes.c_char_p),
]

Job._fields_ = [
    ("id", Uuid),
    ("kind", ctypes.c_char_p),
    ("output_asset_id", Uuid),
    ("output_representation_kind", RepresentationKind),
    ("target_root", ctypes.c_char_p),
    ("state", JobState),
    ("input_count", ctypes.c_uint64),
    ("claim_id", Uuid),
    ("claim_expires_at_unix_micros", ctypes.c_int64),
    ("claim_tool_name", ctypes.c_char_p),
    ("claim_tool_version", ctypes.c_char_p),
    ("claim_tool_uri", ctypes.c_char_p),
    ("claim_agent_name", ctypes.c_char_p),
    ("claim_agent_identifier_scheme", ctypes.c_char_p),
    ("claim_agent_identifier_value", ctypes.c_char_p),
    ("claim_agent_identifier_qualifier", ctypes.c_char_p),
    ("completion_activity_id", Uuid),
    ("completion_representation_id", Uuid),
    ("failure_diagnostic", ctypes.c_char_p),
]

Dependency._fields_ = [
    ("has_source_resource", ctypes.c_uint8),
    ("source_resource_id", Uuid),
    ("kind", ctypes.c_char_p),
    ("target", ObjectRef),
    ("has_resolved_representation", ctypes.c_uint8),
    ("resolved_representation_id", Uuid),
    ("required", ctypes.c_uint8),
    ("authored_reference", ctypes.c_char_p),
]

ArtifactDependencyPathSegment._fields_ = [
    ("source_representation_id", Uuid),
    ("dependency_position", ctypes.c_uint32),
    ("has_source_resource", ctypes.c_uint8),
    ("source_resource_id", Uuid),
    ("kind", ctypes.c_char_p),
    ("target", ObjectRef),
    ("has_resolved_representation", ctypes.c_uint8),
    ("resolved_representation_id", Uuid),
    ("authored_reference", ctypes.c_char_p),
]

ArtifactReason._fields_ = [
    ("kind", ArtifactReasonKind),
    ("activity_id", Uuid),
    ("representation_id", Uuid),
    ("input_representation_id", Uuid),
    ("edge_kind", ArtifactEdgeKind),
    ("upstream_state", ArtifactKnowledgeState),
    ("traversal_limit", ArtifactTraversalLimit),
    ("activity_count", ctypes.c_uint32),
    ("dependency_issue", ArtifactDependencyIssue),
    ("dependency_path", ctypes.POINTER(ArtifactDependencyPathSegment)),
    ("dependency_path_length", ctypes.c_uint64),
    ("fingerprint_algorithm", ctypes.c_char_p),
    ("fingerprint_version", ctypes.c_uint16),
    ("has_snapshot_value", ctypes.c_uint8),
    ("snapshot_value", ctypes.POINTER(ctypes.c_uint8)),
    ("snapshot_value_length", ctypes.c_uint64),
    ("has_current_value", ctypes.c_uint8),
    ("current_value", ctypes.POINTER(ctypes.c_uint8)),
    ("current_value_length", ctypes.c_uint64),
]

ArtifactReproducibilityIssue._fields_ = [
    ("kind", ArtifactReproducibilityIssueKind),
    ("activity_id", Uuid),
    ("representation_id", Uuid),
    ("activity_count", ctypes.c_uint32),
]

FileResourceInput._fields_ = [
    ("path", ctypes.c_char_p),
    ("role", ctypes.c_char_p),
    ("required", ctypes.c_uint8),
]

SequenceNaming._fields_ = [
    ("prefix", ctypes.c_char_p),
    ("suffix", ctypes.c_char_p),
    ("padding", ctypes.c_uint8),
]


PUBLIC_STRUCTS = {
    "pp_uuid_t": (Uuid, ("bytes",)),
    "pp_production_id_t": (ProductionId, ("bytes",)),
    "pp_commit_receipt_t": (CommitReceipt, ("production_id", "outcome", "revision_id", "revision_sequence")),
    "pp_decision_base_t": (DecisionBase, ("production_id", "has_revision", "revision_id", "revision_sequence")),
    "pp_object_ref_t": (ObjectRef, ("kind", "id")),
    "pp_transaction_conflict_t": (TransactionConflict, ("kind", "target", "namespace_name", "local_name", "qualifier", "version", "has_base_revision", "base_revision_id", "base_revision_sequence", "superseding_revision_id", "superseding_revision_sequence")),
    "pp_dependency_match_t": (DependencyMatch, ("target", "depth")),
    "pp_revision_event_t": (RevisionEvent, ("kind", "position", "asset_id", "representation_id", "resource_id", "locator_id", "media_root_id", "activity_id", "job_id", "target", "structural_position", "enabled", "identifier_scheme", "identifier_value", "identifier_qualifier", "vocabulary", "property", "activity_kind", "role", "fingerprint_algorithm", "fingerprint_version")),
    "pp_activity_edge_t": (ActivityEdge, ("representation_id", "role")),
    "pp_job_t": (Job, ("id", "kind", "output_asset_id", "output_representation_kind", "target_root", "state", "input_count", "claim_id", "claim_expires_at_unix_micros", "claim_tool_name", "claim_tool_version", "claim_tool_uri", "claim_agent_name", "claim_agent_identifier_scheme", "claim_agent_identifier_value", "claim_agent_identifier_qualifier", "completion_activity_id", "completion_representation_id", "failure_diagnostic")),
    "pp_dependency_t": (Dependency, ("has_source_resource", "source_resource_id", "kind", "target", "has_resolved_representation", "resolved_representation_id", "required", "authored_reference")),
    "pp_artifact_dependency_path_segment_t": (ArtifactDependencyPathSegment, ("source_representation_id", "dependency_position", "has_source_resource", "source_resource_id", "kind", "target", "has_resolved_representation", "resolved_representation_id", "authored_reference")),
    "pp_artifact_reason_t": (ArtifactReason, ("kind", "activity_id", "representation_id", "input_representation_id", "edge_kind", "upstream_state", "traversal_limit", "activity_count", "dependency_issue", "dependency_path", "dependency_path_length", "fingerprint_algorithm", "fingerprint_version", "has_snapshot_value", "snapshot_value", "snapshot_value_length", "has_current_value", "current_value", "current_value_length")),
    "pp_artifact_reproducibility_issue_t": (ArtifactReproducibilityIssue, ("kind", "activity_id", "representation_id", "activity_count")),
    "pp_file_resource_input_t": (FileResourceInput, ("path", "role", "required")),
    "pp_sequence_naming_t": (SequenceNaming, ("prefix", "suffix", "padding")),
}


EXPORTED_SYMBOLS = (
    "pp_abi_version",
    "pp_activity_set_count",
    "pp_activity_set_get",
    "pp_activity_set_get_agent",
    "pp_activity_set_get_input",
    "pp_activity_set_get_input_snapshot",
    "pp_activity_set_get_input_snapshot_fingerprint",
    "pp_activity_set_get_output",
    "pp_activity_set_get_output_snapshot",
    "pp_activity_set_get_output_snapshot_fingerprint",
    "pp_activity_set_get_tool",
    "pp_activity_set_next_cursor",
    "pp_activity_set_release",
    "pp_artifact_evaluation_get",
    "pp_artifact_evaluation_get_reason",
    "pp_artifact_evaluation_release",
    "pp_artifact_reproducibility_get",
    "pp_artifact_reproducibility_get_issue",
    "pp_artifact_reproducibility_release",
    "pp_asset_set_count",
    "pp_asset_set_get",
    "pp_asset_set_next_cursor",
    "pp_asset_set_release",
    "pp_cancel_token_cancel",
    "pp_cancel_token_create",
    "pp_cancel_token_release",
    "pp_decision_base_format",
    "pp_decision_base_parse",
    "pp_dependency_query_set_count",
    "pp_dependency_query_set_get",
    "pp_dependency_query_set_next_cursor",
    "pp_dependency_query_set_release",
    "pp_dependency_query_set_traversal_truncated",
    "pp_dependency_set_get",
    "pp_dependency_set_get_dependency",
    "pp_dependency_set_release",
    "pp_error_code",
    "pp_error_message",
    "pp_error_release",
    "pp_error_transaction_conflict",
    "pp_external_identifier_set_count",
    "pp_external_identifier_set_get",
    "pp_external_identifier_set_release",
    "pp_file_path_to_locator",
    "pp_fingerprint_file",
    "pp_fingerprint_get",
    "pp_fingerprint_release",
    "pp_host_binding_format",
    "pp_host_binding_parse",
    "pp_job_set_count",
    "pp_job_set_get",
    "pp_job_set_get_input",
    "pp_job_set_next_cursor",
    "pp_job_set_release",
    "pp_known_media_set_count",
    "pp_known_media_set_get",
    "pp_known_media_set_next_cursor",
    "pp_known_media_set_release",
    "pp_locator_query_set_count",
    "pp_locator_query_set_get",
    "pp_locator_query_set_next_cursor",
    "pp_locator_query_set_release",
    "pp_locator_to_file_path",
    "pp_media_root_set_count",
    "pp_media_root_set_get",
    "pp_media_root_set_release",
    "pp_media_source_create_file",
    "pp_media_source_create_image_sequence",
    "pp_media_source_create_ordered_parts",
    "pp_media_source_create_package",
    "pp_media_source_release",
    "pp_metadata_input_create_bool",
    "pp_metadata_input_create_bytes",
    "pp_metadata_input_create_decimal",
    "pp_metadata_input_create_i64",
    "pp_metadata_input_create_list",
    "pp_metadata_input_create_rational",
    "pp_metadata_input_create_reference",
    "pp_metadata_input_create_string",
    "pp_metadata_input_create_struct",
    "pp_metadata_input_create_timestamp",
    "pp_metadata_input_create_u64",
    "pp_metadata_input_create_uri",
    "pp_metadata_input_release",
    "pp_metadata_set_count",
    "pp_metadata_set_get",
    "pp_metadata_set_next_cursor",
    "pp_metadata_set_release",
    "pp_metadata_value_get_bool",
    "pp_metadata_value_get_bytes",
    "pp_metadata_value_get_decimal",
    "pp_metadata_value_get_i64",
    "pp_metadata_value_get_rational",
    "pp_metadata_value_get_reference",
    "pp_metadata_value_get_string",
    "pp_metadata_value_get_timestamp",
    "pp_metadata_value_get_u64",
    "pp_metadata_value_get_uri",
    "pp_metadata_value_kind",
    "pp_metadata_value_list_count",
    "pp_metadata_value_list_get",
    "pp_metadata_value_struct_count",
    "pp_metadata_value_struct_get",
    "pp_object_query_set_count",
    "pp_object_query_set_get",
    "pp_object_query_set_next_cursor",
    "pp_object_query_set_release",
    "pp_object_query_set_traversal_truncated",
    "pp_object_ref_set_count",
    "pp_object_ref_set_get",
    "pp_object_ref_set_release",
    "pp_production_activities",
    "pp_production_activities_consuming",
    "pp_production_activities_consuming_page",
    "pp_production_activities_producing",
    "pp_production_activities_producing_page",
    "pp_production_artifact_reproducibility",
    "pp_production_asset",
    "pp_production_asset_exists",
    "pp_production_assets",
    "pp_production_assets_page",
    "pp_production_begin_edit",
    "pp_production_begin_transaction",
    "pp_production_begin_transaction_at",
    "pp_production_changes_since",
    "pp_production_changes_since_filtered",
    "pp_production_create",
    "pp_production_dependencies",
    "pp_production_dependency_set",
    "pp_production_dependents",
    "pp_production_evaluate_artifact",
    "pp_production_external_identifiers",
    "pp_production_find_by_external_identifier",
    "pp_production_find_known_media_by_fingerprint",
    "pp_production_find_known_media_by_locator",
    "pp_production_find_metadata",
    "pp_production_id",
    "pp_production_id_format",
    "pp_production_id_parse",
    "pp_production_job",
    "pp_production_jobs",
    "pp_production_latest_revision",
    "pp_production_locators_page",
    "pp_production_media_roots",
    "pp_production_metadata",
    "pp_production_objects_changed_since",
    "pp_production_open",
    "pp_production_outputs_by_activity_kind",
    "pp_production_outputs_by_tool",
    "pp_production_plan_regeneration",
    "pp_production_provenance_ancestors",
    "pp_production_provenance_ancestors_page",
    "pp_production_provenance_descendants",
    "pp_production_provenance_descendants_page",
    "pp_production_query_metadata",
    "pp_production_read_session",
    "pp_production_release",
    "pp_production_representation",
    "pp_production_representations",
    "pp_production_representations_page",
    "pp_production_representations_under_media_root",
    "pp_production_representations_using_resource",
    "pp_production_resolve_assets",
    "pp_production_resources_page",
    "pp_production_revision_events",
    "pp_production_stale_artifacts",
    "pp_production_unresolved_media",
    "pp_production_verify_resource",
    "pp_read_session_activities_consuming_page",
    "pp_read_session_activities_producing_page",
    "pp_read_session_artifact_reproducibility",
    "pp_read_session_asset",
    "pp_read_session_assets_page",
    "pp_read_session_begin_edit",
    "pp_read_session_decision_base",
    "pp_read_session_dependencies",
    "pp_read_session_dependency_set",
    "pp_read_session_dependents",
    "pp_read_session_evaluate_artifact",
    "pp_read_session_external_identifiers",
    "pp_read_session_find_by_external_identifier",
    "pp_read_session_find_known_media_by_fingerprint",
    "pp_read_session_find_known_media_by_locator",
    "pp_read_session_find_metadata",
    "pp_read_session_job",
    "pp_read_session_jobs",
    "pp_read_session_locators_page",
    "pp_read_session_media_roots",
    "pp_read_session_metadata",
    "pp_read_session_objects_changed_since",
    "pp_read_session_outputs_by_activity_kind",
    "pp_read_session_outputs_by_tool",
    "pp_read_session_provenance_ancestors_page",
    "pp_read_session_provenance_descendants_page",
    "pp_read_session_query_metadata",
    "pp_read_session_release",
    "pp_read_session_representation",
    "pp_read_session_representations_page",
    "pp_read_session_representations_under_media_root",
    "pp_read_session_representations_using_resource",
    "pp_read_session_resolve_assets",
    "pp_read_session_resources_page",
    "pp_read_session_stale_artifacts",
    "pp_read_session_unresolved_media",
    "pp_read_session_verify_resource",
    "pp_regeneration_plan_set_count",
    "pp_regeneration_plan_set_get",
    "pp_regeneration_plan_set_release",
    "pp_representation_set_count",
    "pp_representation_set_get",
    "pp_representation_set_get_fingerprint",
    "pp_representation_set_get_locator",
    "pp_representation_set_get_member",
    "pp_representation_set_get_resource",
    "pp_representation_set_get_resource_fingerprint",
    "pp_representation_set_get_sequence",
    "pp_representation_set_get_sequence_missing_frame",
    "pp_representation_set_next_cursor",
    "pp_representation_set_release",
    "pp_resolution_options_add_root_mapping",
    "pp_resolution_options_add_search_directory",
    "pp_resolution_options_create",
    "pp_resolution_options_release",
    "pp_resolution_options_set_cancel_token",
    "pp_resolution_options_set_limits",
    "pp_resolution_options_set_verification",
    "pp_resolution_set_get_candidate",
    "pp_resolution_set_get_candidate_evidence",
    "pp_resolution_set_get_issue",
    "pp_resolution_set_get_issue_frame",
    "pp_resolution_set_get_representation",
    "pp_resolution_set_get_resource",
    "pp_resolution_set_get_resource_evidence",
    "pp_resolution_set_release",
    "pp_resolution_set_representation_count",
    "pp_revision_event_set_count",
    "pp_revision_event_set_get",
    "pp_revision_event_set_release",
    "pp_revision_set_count",
    "pp_revision_set_get",
    "pp_revision_set_release",
    "pp_revision_waiter_cancel",
    "pp_revision_waiter_create",
    "pp_revision_waiter_release",
    "pp_revision_waiter_wait",
    "pp_string_release",
    "pp_transaction_add_external_identifier",
    "pp_transaction_add_media_root",
    "pp_transaction_add_metadata_value",
    "pp_transaction_add_representation",
    "pp_transaction_cancel_job",
    "pp_transaction_claim_job",
    "pp_transaction_commit",
    "pp_transaction_commit_with_receipt",
    "pp_transaction_complete_job",
    "pp_transaction_confirm_locator",
    "pp_transaction_create_activity",
    "pp_transaction_fail_job",
    "pp_transaction_import_media",
    "pp_transaction_observe_resource_content",
    "pp_transaction_record_dependency_set",
    "pp_transaction_record_representation_fingerprint",
    "pp_transaction_record_resource_fingerprint",
    "pp_transaction_release",
    "pp_transaction_release_job_claim",
    "pp_transaction_remove_external_identifier",
    "pp_transaction_remove_media_root",
    "pp_transaction_remove_metadata_property",
    "pp_transaction_renew_job_claim",
    "pp_transaction_request_job",
    "pp_transaction_retire_locator",
    "pp_transaction_rollback",
    "pp_transaction_set_media_root_enabled",
    "pp_transaction_set_revision_context",
)


def configure_api(lib: ctypes.CDLL) -> None:
    """Configure every function declared by the public C header."""

    lib.pp_decision_base_parse.argtypes = [ctypes.c_char_p, ctypes.POINTER(DecisionBase), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_decision_base_parse.restype = ErrorCode
    lib.pp_decision_base_format.argtypes = [ctypes.POINTER(DecisionBase), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_decision_base_format.restype = ErrorCode
    lib.pp_production_read_session.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ctypes.POINTER(ReadSession)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_read_session.restype = ErrorCode
    lib.pp_read_session_decision_base.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(DecisionBase), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_decision_base.restype = ErrorCode
    lib.pp_read_session_begin_edit.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(ctypes.POINTER(Transaction)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_begin_edit.restype = ErrorCode
    lib.pp_production_begin_edit.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(DecisionBase), ctypes.POINTER(ctypes.POINTER(Transaction)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_begin_edit.restype = ErrorCode
    lib.pp_read_session_evaluate_artifact.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.POINTER(ctypes.POINTER(ArtifactEvaluation)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_evaluate_artifact.restype = ErrorCode
    lib.pp_read_session_artifact_reproducibility.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(ArtifactReproducibility)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_artifact_reproducibility.restype = ErrorCode
    lib.pp_read_session_activities_producing_page.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ActivitySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_activities_producing_page.restype = ErrorCode
    lib.pp_read_session_activities_consuming_page.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ActivitySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_activities_consuming_page.restype = ErrorCode
    lib.pp_read_session_outputs_by_activity_kind.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_outputs_by_activity_kind.restype = ErrorCode
    lib.pp_read_session_outputs_by_tool.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_outputs_by_tool.restype = ErrorCode
    lib.pp_read_session_provenance_ancestors_page.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_provenance_ancestors_page.restype = ErrorCode
    lib.pp_read_session_provenance_descendants_page.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_provenance_descendants_page.restype = ErrorCode
    lib.pp_read_session_stale_artifacts.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_stale_artifacts.restype = ErrorCode
    lib.pp_read_session_representations_under_media_root.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_representations_under_media_root.restype = ErrorCode
    lib.pp_read_session_unresolved_media.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_unresolved_media.restype = ErrorCode
    lib.pp_read_session_objects_changed_since.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_uint64, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_objects_changed_since.restype = ErrorCode
    lib.pp_read_session_dependency_set.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(DependencySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_dependency_set.restype = ErrorCode
    lib.pp_read_session_dependencies.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(DependencyQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_dependencies.restype = ErrorCode
    lib.pp_read_session_dependents.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(ObjectRef), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(DependencyQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_dependents.restype = ErrorCode
    lib.pp_read_session_job.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(JobSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_job.restype = ErrorCode
    lib.pp_read_session_jobs.argtypes = [ctypes.POINTER(ReadSession), JobState, ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(JobSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_jobs.restype = ErrorCode
    lib.pp_read_session_release.argtypes = [ctypes.POINTER(ReadSession)]
    lib.pp_read_session_release.restype = None
    lib.pp_production_id_parse.argtypes = [ctypes.c_char_p, ctypes.POINTER(ProductionId), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_id_parse.restype = ErrorCode
    lib.pp_production_id_format.argtypes = [ProductionId, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_id_format.restype = ErrorCode
    lib.pp_read_session_assets_page.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(AssetSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_assets_page.restype = ErrorCode
    lib.pp_read_session_asset.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(AssetSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_asset.restype = ErrorCode
    lib.pp_read_session_representations_page.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_representations_page.restype = ErrorCode
    lib.pp_read_session_representation.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_representation.restype = ErrorCode
    lib.pp_read_session_resources_page.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_resources_page.restype = ErrorCode
    lib.pp_read_session_locators_page.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(LocatorQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_locators_page.restype = ErrorCode
    lib.pp_read_session_representations_using_resource.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_representations_using_resource.restype = ErrorCode
    lib.pp_read_session_resolve_assets.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_uint64, ctypes.POINTER(ResolutionOptions), ctypes.POINTER(ctypes.POINTER(ResolutionSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_resolve_assets.restype = ErrorCode
    lib.pp_read_session_verify_resource.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(Uuid), ctypes.c_char_p, ctypes.POINTER(SequenceNaming), ctypes.POINTER(ContentVerification), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_verify_resource.restype = ErrorCode
    lib.pp_read_session_media_roots.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(ctypes.POINTER(MediaRootSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_media_roots.restype = ErrorCode
    lib.pp_read_session_external_identifiers.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.POINTER(ExternalIdentifierSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_external_identifiers.restype = ErrorCode
    lib.pp_read_session_metadata.argtypes = [ctypes.POINTER(ReadSession), ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.POINTER(MetadataSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_metadata.restype = ErrorCode
    lib.pp_read_session_find_metadata.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(MetadataSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_find_metadata.restype = ErrorCode
    lib.pp_read_session_query_metadata.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(MetadataInput), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(MetadataSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_query_metadata.restype = ErrorCode
    lib.pp_read_session_find_by_external_identifier.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectRefSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_find_by_external_identifier.restype = ErrorCode
    lib.pp_read_session_find_known_media_by_locator.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_char_p, ctypes.POINTER(SequenceNaming), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(KnownMediaSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_find_known_media_by_locator.restype = ErrorCode
    lib.pp_read_session_find_known_media_by_fingerprint.argtypes = [ctypes.POINTER(ReadSession), ctypes.c_char_p, ctypes.c_uint16, ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint64, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(KnownMediaSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_read_session_find_known_media_by_fingerprint.restype = ErrorCode
    lib.pp_abi_version.argtypes = []
    lib.pp_abi_version.restype = ctypes.c_uint32
    lib.pp_host_binding_format.argtypes = [ProductionId, ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_host_binding_format.restype = ErrorCode
    lib.pp_host_binding_parse.argtypes = [ctypes.c_char_p, ctypes.POINTER(ProductionId), ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_host_binding_parse.restype = ErrorCode
    lib.pp_file_path_to_locator.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_file_path_to_locator.restype = ErrorCode
    lib.pp_locator_to_file_path.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_locator_to_file_path.restype = ErrorCode
    lib.pp_string_release.argtypes = [ctypes.c_char_p]
    lib.pp_string_release.restype = None
    lib.pp_production_create.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Production)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_create.restype = ErrorCode
    lib.pp_production_open.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Production)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_open.restype = ErrorCode
    lib.pp_production_id.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ProductionId), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_id.restype = ErrorCode
    lib.pp_production_asset_exists.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_asset_exists.restype = ErrorCode
    lib.pp_production_assets.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ctypes.POINTER(AssetSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_assets.restype = ErrorCode
    lib.pp_production_assets_page.argtypes = [ctypes.POINTER(Production), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(AssetSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_assets_page.restype = ErrorCode
    lib.pp_production_asset.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(AssetSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_asset.restype = ErrorCode
    lib.pp_asset_set_count.argtypes = [ctypes.POINTER(AssetSet)]
    lib.pp_asset_set_count.restype = ctypes.c_uint64
    lib.pp_asset_set_next_cursor.argtypes = [ctypes.POINTER(AssetSet)]
    lib.pp_asset_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_asset_set_get.argtypes = [ctypes.POINTER(AssetSet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_asset_set_get.restype = ErrorCode
    lib.pp_asset_set_release.argtypes = [ctypes.POINTER(AssetSet)]
    lib.pp_asset_set_release.restype = None
    lib.pp_production_media_roots.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ctypes.POINTER(MediaRootSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_media_roots.restype = ErrorCode
    lib.pp_media_root_set_count.argtypes = [ctypes.POINTER(MediaRootSet)]
    lib.pp_media_root_set_count.restype = ctypes.c_uint64
    lib.pp_media_root_set_get.argtypes = [ctypes.POINTER(MediaRootSet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_int32), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_media_root_set_get.restype = ErrorCode
    lib.pp_media_root_set_release.argtypes = [ctypes.POINTER(MediaRootSet)]
    lib.pp_media_root_set_release.restype = None
    lib.pp_production_representations.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_representations.restype = ErrorCode
    lib.pp_production_representations_page.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_representations_page.restype = ErrorCode
    lib.pp_production_representations_under_media_root.argtypes = [ctypes.POINTER(Production), ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_representations_under_media_root.restype = ErrorCode
    lib.pp_production_representation.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_representation.restype = ErrorCode
    lib.pp_production_representations_using_resource.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(RepresentationSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_representations_using_resource.restype = ErrorCode
    lib.pp_representation_set_count.argtypes = [ctypes.POINTER(RepresentationSet)]
    lib.pp_representation_set_count.restype = ctypes.c_uint64
    lib.pp_representation_set_next_cursor.argtypes = [ctypes.POINTER(RepresentationSet)]
    lib.pp_representation_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_representation_set_get.argtypes = [ctypes.POINTER(RepresentationSet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.POINTER(RepresentationKind), ctypes.POINTER(ContentStructureKind), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_representation_set_get.restype = ErrorCode
    lib.pp_representation_set_get_fingerprint.argtypes = [ctypes.POINTER(RepresentationSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint16), ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_representation_set_get_fingerprint.restype = ErrorCode
    lib.pp_representation_set_get_member.argtypes = [ctypes.POINTER(RepresentationSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_representation_set_get_member.restype = ErrorCode
    lib.pp_representation_set_get_sequence.argtypes = [ctypes.POINTER(RepresentationSet), ctypes.c_uint64, ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_representation_set_get_sequence.restype = ErrorCode
    lib.pp_representation_set_get_sequence_missing_frame.argtypes = [ctypes.POINTER(RepresentationSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_representation_set_get_sequence_missing_frame.restype = ErrorCode
    lib.pp_representation_set_get_resource.argtypes = [ctypes.POINTER(RepresentationSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_representation_set_get_resource.restype = ErrorCode
    lib.pp_representation_set_get_resource_fingerprint.argtypes = [ctypes.POINTER(RepresentationSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint16), ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_representation_set_get_resource_fingerprint.restype = ErrorCode
    lib.pp_representation_set_get_locator.argtypes = [ctypes.POINTER(RepresentationSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(LocatorAvailability), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(SequenceNaming), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_representation_set_get_locator.restype = ErrorCode
    lib.pp_representation_set_release.argtypes = [ctypes.POINTER(RepresentationSet)]
    lib.pp_representation_set_release.restype = None
    lib.pp_production_external_identifiers.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.POINTER(ExternalIdentifierSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_external_identifiers.restype = ErrorCode
    lib.pp_production_find_by_external_identifier.argtypes = [ctypes.POINTER(Production), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectRefSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_find_by_external_identifier.restype = ErrorCode
    lib.pp_external_identifier_set_count.argtypes = [ctypes.POINTER(ExternalIdentifierSet)]
    lib.pp_external_identifier_set_count.restype = ctypes.c_uint64
    lib.pp_external_identifier_set_get.argtypes = [ctypes.POINTER(ExternalIdentifierSet), ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_external_identifier_set_get.restype = ErrorCode
    lib.pp_external_identifier_set_release.argtypes = [ctypes.POINTER(ExternalIdentifierSet)]
    lib.pp_external_identifier_set_release.restype = None
    lib.pp_object_ref_set_count.argtypes = [ctypes.POINTER(ObjectRefSet)]
    lib.pp_object_ref_set_count.restype = ctypes.c_uint64
    lib.pp_object_ref_set_get.argtypes = [ctypes.POINTER(ObjectRefSet), ctypes.c_uint64, ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_object_ref_set_get.restype = ErrorCode
    lib.pp_object_ref_set_release.argtypes = [ctypes.POINTER(ObjectRefSet)]
    lib.pp_object_ref_set_release.restype = None
    lib.pp_object_query_set_count.argtypes = [ctypes.POINTER(ObjectQuerySet)]
    lib.pp_object_query_set_count.restype = ctypes.c_uint64
    lib.pp_object_query_set_get.argtypes = [ctypes.POINTER(ObjectQuerySet), ctypes.c_uint64, ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_object_query_set_get.restype = ErrorCode
    lib.pp_object_query_set_next_cursor.argtypes = [ctypes.POINTER(ObjectQuerySet)]
    lib.pp_object_query_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_object_query_set_traversal_truncated.argtypes = [ctypes.POINTER(ObjectQuerySet)]
    lib.pp_object_query_set_traversal_truncated.restype = ctypes.c_uint8
    lib.pp_object_query_set_release.argtypes = [ctypes.POINTER(ObjectQuerySet)]
    lib.pp_object_query_set_release.restype = None
    lib.pp_production_resources_page.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_resources_page.restype = ErrorCode
    lib.pp_production_locators_page.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(LocatorQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_locators_page.restype = ErrorCode
    lib.pp_locator_query_set_count.argtypes = [ctypes.POINTER(LocatorQuerySet)]
    lib.pp_locator_query_set_count.restype = ctypes.c_uint64
    lib.pp_locator_query_set_get.argtypes = [ctypes.POINTER(LocatorQuerySet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(LocatorAvailability), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(SequenceNaming), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_locator_query_set_get.restype = ErrorCode
    lib.pp_locator_query_set_next_cursor.argtypes = [ctypes.POINTER(LocatorQuerySet)]
    lib.pp_locator_query_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_locator_query_set_release.argtypes = [ctypes.POINTER(LocatorQuerySet)]
    lib.pp_locator_query_set_release.restype = None
    lib.pp_production_find_known_media_by_locator.argtypes = [ctypes.POINTER(Production), ctypes.c_char_p, ctypes.POINTER(SequenceNaming), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(KnownMediaSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_find_known_media_by_locator.restype = ErrorCode
    lib.pp_production_find_known_media_by_fingerprint.argtypes = [ctypes.POINTER(Production), ctypes.c_char_p, ctypes.c_uint16, ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint64, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(KnownMediaSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_find_known_media_by_fingerprint.restype = ErrorCode
    lib.pp_known_media_set_count.argtypes = [ctypes.POINTER(KnownMediaSet)]
    lib.pp_known_media_set_count.restype = ctypes.c_uint64
    lib.pp_known_media_set_next_cursor.argtypes = [ctypes.POINTER(KnownMediaSet)]
    lib.pp_known_media_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_known_media_set_get.argtypes = [ctypes.POINTER(KnownMediaSet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_known_media_set_get.restype = ErrorCode
    lib.pp_known_media_set_release.argtypes = [ctypes.POINTER(KnownMediaSet)]
    lib.pp_known_media_set_release.restype = None
    lib.pp_production_unresolved_media.argtypes = [ctypes.POINTER(Production), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_unresolved_media.restype = ErrorCode
    lib.pp_production_objects_changed_since.argtypes = [ctypes.POINTER(Production), ctypes.c_uint64, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_objects_changed_since.restype = ErrorCode
    lib.pp_production_metadata.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.POINTER(MetadataSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_metadata.restype = ErrorCode
    lib.pp_production_find_metadata.argtypes = [ctypes.POINTER(Production), ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(MetadataSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_find_metadata.restype = ErrorCode
    lib.pp_production_query_metadata.argtypes = [ctypes.POINTER(Production), ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(MetadataInput), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(MetadataSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_query_metadata.restype = ErrorCode
    lib.pp_metadata_set_count.argtypes = [ctypes.POINTER(MetadataSet)]
    lib.pp_metadata_set_count.restype = ctypes.c_uint64
    lib.pp_metadata_set_next_cursor.argtypes = [ctypes.POINTER(MetadataSet)]
    lib.pp_metadata_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_metadata_set_get.argtypes = [ctypes.POINTER(MetadataSet), ctypes.c_uint64, ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(MetadataValue)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_set_get.restype = ErrorCode
    lib.pp_metadata_set_release.argtypes = [ctypes.POINTER(MetadataSet)]
    lib.pp_metadata_set_release.restype = None
    lib.pp_metadata_value_kind.argtypes = [ctypes.POINTER(MetadataValue)]
    lib.pp_metadata_value_kind.restype = MetadataValueKind
    lib.pp_metadata_value_get_string.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_string.restype = ErrorCode
    lib.pp_metadata_value_get_i64.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_i64.restype = ErrorCode
    lib.pp_metadata_value_get_u64.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_u64.restype = ErrorCode
    lib.pp_metadata_value_get_decimal.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_decimal.restype = ErrorCode
    lib.pp_metadata_value_get_bool.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_bool.restype = ErrorCode
    lib.pp_metadata_value_get_timestamp.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_timestamp.restype = ErrorCode
    lib.pp_metadata_value_get_uri.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_uri.restype = ErrorCode
    lib.pp_metadata_value_get_bytes.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_bytes.restype = ErrorCode
    lib.pp_metadata_value_get_rational.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_rational.restype = ErrorCode
    lib.pp_metadata_value_list_count.argtypes = [ctypes.POINTER(MetadataValue)]
    lib.pp_metadata_value_list_count.restype = ctypes.c_uint64
    lib.pp_metadata_value_list_get.argtypes = [ctypes.POINTER(MetadataValue), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MetadataValue)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_list_get.restype = ErrorCode
    lib.pp_metadata_value_struct_count.argtypes = [ctypes.POINTER(MetadataValue)]
    lib.pp_metadata_value_struct_count.restype = ctypes.c_uint64
    lib.pp_metadata_value_struct_get.argtypes = [ctypes.POINTER(MetadataValue), ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(MetadataValue)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_struct_get.restype = ErrorCode
    lib.pp_metadata_value_get_reference.argtypes = [ctypes.POINTER(MetadataValue), ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_value_get_reference.restype = ErrorCode
    lib.pp_metadata_input_create_string.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_string.restype = ErrorCode
    lib.pp_metadata_input_create_i64.argtypes = [ctypes.c_int64, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_i64.restype = ErrorCode
    lib.pp_metadata_input_create_u64.argtypes = [ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_u64.restype = ErrorCode
    lib.pp_metadata_input_create_decimal.argtypes = [ctypes.c_char_p, ctypes.c_uint32, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_decimal.restype = ErrorCode
    lib.pp_metadata_input_create_bool.argtypes = [ctypes.c_uint8, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_bool.restype = ErrorCode
    lib.pp_metadata_input_create_timestamp.argtypes = [ctypes.c_int64, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_timestamp.restype = ErrorCode
    lib.pp_metadata_input_create_uri.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_uri.restype = ErrorCode
    lib.pp_metadata_input_create_bytes.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_bytes.restype = ErrorCode
    lib.pp_metadata_input_create_rational.argtypes = [ctypes.c_int64, ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_rational.restype = ErrorCode
    lib.pp_metadata_input_create_reference.argtypes = [ctypes.POINTER(ObjectRef), ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_reference.restype = ErrorCode
    lib.pp_metadata_input_create_list.argtypes = [ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_list.restype = ErrorCode
    lib.pp_metadata_input_create_struct.argtypes = [ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MetadataInput)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_metadata_input_create_struct.restype = ErrorCode
    lib.pp_metadata_input_release.argtypes = [ctypes.POINTER(MetadataInput)]
    lib.pp_metadata_input_release.restype = None
    lib.pp_production_dependency_set.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(DependencySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_dependency_set.restype = ErrorCode
    lib.pp_dependency_set_get.argtypes = [ctypes.POINTER(DependencySet), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(DependencySetStatus), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_dependency_set_get.restype = ErrorCode
    lib.pp_dependency_set_get_dependency.argtypes = [ctypes.POINTER(DependencySet), ctypes.c_uint64, ctypes.POINTER(Dependency), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_dependency_set_get_dependency.restype = ErrorCode
    lib.pp_dependency_set_release.argtypes = [ctypes.POINTER(DependencySet)]
    lib.pp_dependency_set_release.restype = None
    lib.pp_production_dependencies.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(DependencyQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_dependencies.restype = ErrorCode
    lib.pp_production_dependents.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ObjectRef), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(DependencyQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_dependents.restype = ErrorCode
    lib.pp_dependency_query_set_count.argtypes = [ctypes.POINTER(DependencyQuerySet)]
    lib.pp_dependency_query_set_count.restype = ctypes.c_uint64
    lib.pp_dependency_query_set_get.argtypes = [ctypes.POINTER(DependencyQuerySet), ctypes.c_uint64, ctypes.POINTER(DependencyMatch), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_dependency_query_set_get.restype = ErrorCode
    lib.pp_dependency_query_set_next_cursor.argtypes = [ctypes.POINTER(DependencyQuerySet)]
    lib.pp_dependency_query_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_dependency_query_set_traversal_truncated.argtypes = [ctypes.POINTER(DependencyQuerySet)]
    lib.pp_dependency_query_set_traversal_truncated.restype = ctypes.c_uint8
    lib.pp_dependency_query_set_release.argtypes = [ctypes.POINTER(DependencyQuerySet)]
    lib.pp_dependency_query_set_release.restype = None
    lib.pp_production_evaluate_artifact.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.POINTER(ctypes.POINTER(ArtifactEvaluation)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_evaluate_artifact.restype = ErrorCode
    lib.pp_artifact_evaluation_get.argtypes = [ctypes.POINTER(ArtifactEvaluation), ctypes.POINTER(Uuid), ctypes.POINTER(ArtifactKnowledgeState), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_artifact_evaluation_get.restype = ErrorCode
    lib.pp_artifact_evaluation_get_reason.argtypes = [ctypes.POINTER(ArtifactEvaluation), ctypes.c_uint64, ctypes.POINTER(ArtifactReason), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_artifact_evaluation_get_reason.restype = ErrorCode
    lib.pp_artifact_evaluation_release.argtypes = [ctypes.POINTER(ArtifactEvaluation)]
    lib.pp_artifact_evaluation_release.restype = None
    lib.pp_production_artifact_reproducibility.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(ArtifactReproducibility)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_artifact_reproducibility.restype = ErrorCode
    lib.pp_artifact_reproducibility_get.argtypes = [ctypes.POINTER(ArtifactReproducibility), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_artifact_reproducibility_get.restype = ErrorCode
    lib.pp_artifact_reproducibility_get_issue.argtypes = [ctypes.POINTER(ArtifactReproducibility), ctypes.c_uint64, ctypes.POINTER(ArtifactReproducibilityIssue), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_artifact_reproducibility_get_issue.restype = ErrorCode
    lib.pp_artifact_reproducibility_release.argtypes = [ctypes.POINTER(ArtifactReproducibility)]
    lib.pp_artifact_reproducibility_release.restype = None
    lib.pp_production_activities.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ctypes.POINTER(ActivitySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_activities.restype = ErrorCode
    lib.pp_production_activities_producing.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(ActivitySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_activities_producing.restype = ErrorCode
    lib.pp_production_activities_consuming.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(ActivitySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_activities_consuming.restype = ErrorCode
    lib.pp_production_activities_producing_page.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ActivitySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_activities_producing_page.restype = ErrorCode
    lib.pp_production_activities_consuming_page.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ActivitySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_activities_consuming_page.restype = ErrorCode
    lib.pp_production_outputs_by_activity_kind.argtypes = [ctypes.POINTER(Production), ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_outputs_by_activity_kind.restype = ErrorCode
    lib.pp_production_outputs_by_tool.argtypes = [ctypes.POINTER(Production), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_outputs_by_tool.restype = ErrorCode
    lib.pp_production_provenance_ancestors.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(ObjectRefSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_provenance_ancestors.restype = ErrorCode
    lib.pp_production_provenance_descendants.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(ObjectRefSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_provenance_descendants.restype = ErrorCode
    lib.pp_production_provenance_ancestors_page.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_provenance_ancestors_page.restype = ErrorCode
    lib.pp_production_provenance_descendants_page.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_provenance_descendants_page.restype = ErrorCode
    lib.pp_production_stale_artifacts.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(ObjectQuerySet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_stale_artifacts.restype = ErrorCode
    lib.pp_activity_set_count.argtypes = [ctypes.POINTER(ActivitySet)]
    lib.pp_activity_set_count.restype = ctypes.c_uint64
    lib.pp_activity_set_next_cursor.argtypes = [ctypes.POINTER(ActivitySet)]
    lib.pp_activity_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_activity_set_get.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get.restype = ErrorCode
    lib.pp_activity_set_get_tool.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get_tool.restype = ErrorCode
    lib.pp_activity_set_get_agent.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get_agent.restype = ErrorCode
    lib.pp_activity_set_get_input.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get_input.restype = ErrorCode
    lib.pp_activity_set_get_output.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get_output.restype = ErrorCode
    lib.pp_activity_set_get_input_snapshot.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get_input_snapshot.restype = ErrorCode
    lib.pp_activity_set_get_output_snapshot.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get_output_snapshot.restype = ErrorCode
    lib.pp_activity_set_get_input_snapshot_fingerprint.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint16), ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get_input_snapshot_fingerprint.restype = ErrorCode
    lib.pp_activity_set_get_output_snapshot_fingerprint.argtypes = [ctypes.POINTER(ActivitySet), ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint16), ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_activity_set_get_output_snapshot_fingerprint.restype = ErrorCode
    lib.pp_activity_set_release.argtypes = [ctypes.POINTER(ActivitySet)]
    lib.pp_activity_set_release.restype = None
    lib.pp_production_jobs.argtypes = [ctypes.POINTER(Production), JobState, ctypes.c_char_p, ctypes.c_uint32, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(JobSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_jobs.restype = ErrorCode
    lib.pp_production_job.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(JobSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_job.restype = ErrorCode
    lib.pp_job_set_count.argtypes = [ctypes.POINTER(JobSet)]
    lib.pp_job_set_count.restype = ctypes.c_uint64
    lib.pp_job_set_next_cursor.argtypes = [ctypes.POINTER(JobSet)]
    lib.pp_job_set_next_cursor.restype = ctypes.c_char_p
    lib.pp_job_set_get.argtypes = [ctypes.POINTER(JobSet), ctypes.c_uint64, ctypes.POINTER(Job), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_job_set_get.restype = ErrorCode
    lib.pp_job_set_get_input.argtypes = [ctypes.POINTER(JobSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_job_set_get_input.restype = ErrorCode
    lib.pp_job_set_release.argtypes = [ctypes.POINTER(JobSet)]
    lib.pp_job_set_release.restype = None
    lib.pp_production_plan_regeneration.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(RegenerationPlanSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_plan_regeneration.restype = ErrorCode
    lib.pp_regeneration_plan_set_count.argtypes = [ctypes.POINTER(RegenerationPlanSet)]
    lib.pp_regeneration_plan_set_count.restype = ctypes.c_uint64
    lib.pp_regeneration_plan_set_get.argtypes = [ctypes.POINTER(RegenerationPlanSet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(JobSet)), ctypes.POINTER(ctypes.POINTER(MetadataSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_regeneration_plan_set_get.restype = ErrorCode
    lib.pp_regeneration_plan_set_release.argtypes = [ctypes.POINTER(RegenerationPlanSet)]
    lib.pp_regeneration_plan_set_release.restype = None
    lib.pp_production_latest_revision.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ctypes.POINTER(RevisionSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_latest_revision.restype = ErrorCode
    lib.pp_production_changes_since.argtypes = [ctypes.POINTER(Production), ctypes.c_uint64, ctypes.c_uint32, ctypes.POINTER(ctypes.POINTER(RevisionSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_changes_since.restype = ErrorCode
    lib.pp_revision_set_count.argtypes = [ctypes.POINTER(RevisionSet)]
    lib.pp_revision_set_count.restype = ctypes.c_uint64
    lib.pp_revision_set_get.argtypes = [ctypes.POINTER(RevisionSet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_revision_set_get.restype = ErrorCode
    lib.pp_revision_set_release.argtypes = [ctypes.POINTER(RevisionSet)]
    lib.pp_revision_set_release.restype = None
    lib.pp_production_revision_events.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(RevisionEventSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_revision_events.restype = ErrorCode
    lib.pp_revision_event_set_count.argtypes = [ctypes.POINTER(RevisionEventSet)]
    lib.pp_revision_event_set_count.restype = ctypes.c_uint64
    lib.pp_revision_event_set_get.argtypes = [ctypes.POINTER(RevisionEventSet), ctypes.c_uint64, ctypes.POINTER(RevisionEvent), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_revision_event_set_get.restype = ErrorCode
    lib.pp_revision_event_set_release.argtypes = [ctypes.POINTER(RevisionEventSet)]
    lib.pp_revision_event_set_release.restype = None
    lib.pp_production_changes_since_filtered.argtypes = [ctypes.POINTER(Production), ctypes.c_uint64, ctypes.POINTER(RevisionEventKind), ctypes.c_uint64, ctypes.c_uint32, ctypes.POINTER(ctypes.POINTER(RevisionSet)), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_changes_since_filtered.restype = ErrorCode
    lib.pp_revision_waiter_create.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ctypes.POINTER(RevisionWaiter)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_revision_waiter_create.restype = ErrorCode
    lib.pp_revision_waiter_wait.argtypes = [ctypes.POINTER(RevisionWaiter), ctypes.c_uint64, ctypes.c_uint32, ctypes.c_uint32, ctypes.POINTER(RevisionWaitResult), ctypes.POINTER(ctypes.POINTER(RevisionSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_revision_waiter_wait.restype = ErrorCode
    lib.pp_revision_waiter_cancel.argtypes = [ctypes.POINTER(RevisionWaiter)]
    lib.pp_revision_waiter_cancel.restype = None
    lib.pp_revision_waiter_release.argtypes = [ctypes.POINTER(RevisionWaiter)]
    lib.pp_revision_waiter_release.restype = None
    lib.pp_fingerprint_file.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Fingerprint)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_fingerprint_file.restype = ErrorCode
    lib.pp_fingerprint_get.argtypes = [ctypes.POINTER(Fingerprint), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint16), ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_fingerprint_get.restype = ErrorCode
    lib.pp_fingerprint_release.argtypes = [ctypes.POINTER(Fingerprint)]
    lib.pp_fingerprint_release.restype = None
    lib.pp_production_verify_resource.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_char_p, ctypes.POINTER(SequenceNaming), ctypes.POINTER(ContentVerification), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_verify_resource.restype = ErrorCode
    lib.pp_cancel_token_create.argtypes = [ctypes.POINTER(ctypes.POINTER(CancelToken)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_cancel_token_create.restype = ErrorCode
    lib.pp_cancel_token_cancel.argtypes = [ctypes.POINTER(CancelToken)]
    lib.pp_cancel_token_cancel.restype = None
    lib.pp_cancel_token_release.argtypes = [ctypes.POINTER(CancelToken)]
    lib.pp_cancel_token_release.restype = None
    lib.pp_resolution_options_create.argtypes = [ctypes.POINTER(ctypes.POINTER(ResolutionOptions)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_options_create.restype = ErrorCode
    lib.pp_resolution_options_release.argtypes = [ctypes.POINTER(ResolutionOptions)]
    lib.pp_resolution_options_release.restype = None
    lib.pp_resolution_options_add_root_mapping.argtypes = [ctypes.POINTER(ResolutionOptions), ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_options_add_root_mapping.restype = ErrorCode
    lib.pp_resolution_options_add_search_directory.argtypes = [ctypes.POINTER(ResolutionOptions), ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_options_add_search_directory.restype = ErrorCode
    lib.pp_resolution_options_set_verification.argtypes = [ctypes.POINTER(ResolutionOptions), VerificationMode, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_options_set_verification.restype = ErrorCode
    lib.pp_resolution_options_set_limits.argtypes = [ctypes.POINTER(ResolutionOptions), ctypes.c_uint32, ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_options_set_limits.restype = ErrorCode
    lib.pp_resolution_options_set_cancel_token.argtypes = [ctypes.POINTER(ResolutionOptions), ctypes.POINTER(CancelToken), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_options_set_cancel_token.restype = ErrorCode
    lib.pp_production_resolve_assets.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.c_uint64, ctypes.POINTER(ResolutionOptions), ctypes.POINTER(ctypes.POINTER(ResolutionSet)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_resolve_assets.restype = ErrorCode
    lib.pp_resolution_set_representation_count.argtypes = [ctypes.POINTER(ResolutionSet)]
    lib.pp_resolution_set_representation_count.restype = ctypes.c_uint64
    lib.pp_resolution_set_get_representation.argtypes = [ctypes.POINTER(ResolutionSet), ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.POINTER(RepresentationAvailability), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_set_get_representation.restype = ErrorCode
    lib.pp_resolution_set_get_resource.argtypes = [ctypes.POINTER(ResolutionSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ResourceResolutionState), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_set_get_resource.restype = ErrorCode
    lib.pp_resolution_set_get_issue.argtypes = [ctypes.POINTER(ResolutionSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(AvailabilityIssueKind), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_set_get_issue.restype = ErrorCode
    lib.pp_resolution_set_get_issue_frame.argtypes = [ctypes.POINTER(ResolutionSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_set_get_issue_frame.restype = ErrorCode
    lib.pp_resolution_set_get_candidate.argtypes = [ctypes.POINTER(ResolutionSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint16), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(SequenceNaming), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_set_get_candidate.restype = ErrorCode
    lib.pp_resolution_set_get_resource_evidence.argtypes = [ctypes.POINTER(ResolutionSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(EvidenceKind), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_set_get_resource_evidence.restype = ErrorCode
    lib.pp_resolution_set_get_candidate_evidence.argtypes = [ctypes.POINTER(ResolutionSet), ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.c_uint64, ctypes.POINTER(EvidenceKind), ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_resolution_set_get_candidate_evidence.restype = ErrorCode
    lib.pp_resolution_set_release.argtypes = [ctypes.POINTER(ResolutionSet)]
    lib.pp_resolution_set_release.restype = None
    lib.pp_production_begin_transaction.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(ctypes.POINTER(Transaction)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_begin_transaction.restype = ErrorCode
    lib.pp_production_begin_transaction_at.argtypes = [ctypes.POINTER(Production), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Transaction)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_production_begin_transaction_at.restype = ErrorCode
    lib.pp_production_release.argtypes = [ctypes.POINTER(Production)]
    lib.pp_production_release.restype = None
    lib.pp_media_source_create_file.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(MediaSource)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_media_source_create_file.restype = ErrorCode
    lib.pp_media_source_create_image_sequence.argtypes = [ctypes.c_char_p, ctypes.POINTER(SequenceNaming), ctypes.c_int64, ctypes.c_int64, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32, ctypes.POINTER(ctypes.c_int64), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MediaSource)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_media_source_create_image_sequence.restype = ErrorCode
    lib.pp_media_source_create_ordered_parts.argtypes = [ctypes.POINTER(FileResourceInput), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MediaSource)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_media_source_create_ordered_parts.restype = ErrorCode
    lib.pp_media_source_create_package.argtypes = [ctypes.POINTER(FileResourceInput), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(MediaSource)), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_media_source_create_package.restype = ErrorCode
    lib.pp_media_source_release.argtypes = [ctypes.POINTER(MediaSource)]
    lib.pp_media_source_release.restype = None
    lib.pp_transaction_set_revision_context.argtypes = [ctypes.POINTER(Transaction), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_set_revision_context.restype = ErrorCode
    lib.pp_transaction_import_media.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(MediaSource), ctypes.c_char_p, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_import_media.restype = ErrorCode
    lib.pp_transaction_add_representation.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), RepresentationKind, ctypes.POINTER(MediaSource), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_add_representation.restype = ErrorCode
    lib.pp_transaction_add_media_root.argtypes = [ctypes.POINTER(Transaction), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_add_media_root.restype = ErrorCode
    lib.pp_transaction_set_media_root_enabled.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.c_uint8, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_set_media_root_enabled.restype = ErrorCode
    lib.pp_transaction_remove_media_root.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_remove_media_root.restype = ErrorCode
    lib.pp_transaction_confirm_locator.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(SequenceNaming), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_confirm_locator.restype = ErrorCode
    lib.pp_transaction_retire_locator.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_retire_locator.restype = ErrorCode
    lib.pp_transaction_record_resource_fingerprint.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.c_char_p, ctypes.c_uint16, ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_record_resource_fingerprint.restype = ErrorCode
    lib.pp_transaction_record_representation_fingerprint.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.c_char_p, ctypes.c_uint16, ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_record_representation_fingerprint.restype = ErrorCode
    lib.pp_transaction_observe_resource_content.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.c_char_p, ctypes.POINTER(SequenceNaming), ctypes.POINTER(ContentObservation), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_observe_resource_content.restype = ErrorCode
    lib.pp_transaction_record_dependency_set.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.POINTER(Dependency), ctypes.c_uint64, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_record_dependency_set.restype = ErrorCode
    lib.pp_transaction_add_external_identifier.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(ObjectRef), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_add_external_identifier.restype = ErrorCode
    lib.pp_transaction_remove_external_identifier.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(ObjectRef), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_remove_external_identifier.restype = ErrorCode
    lib.pp_transaction_add_metadata_value.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(ObjectRef), ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(MetadataInput), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_add_metadata_value.restype = ErrorCode
    lib.pp_transaction_remove_metadata_property.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(ObjectRef), ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_remove_metadata_property.restype = ErrorCode
    lib.pp_transaction_request_job.argtypes = [ctypes.POINTER(Transaction), ctypes.c_char_p, ctypes.POINTER(Uuid), ctypes.c_uint64, ctypes.POINTER(Uuid), RepresentationKind, ctypes.c_char_p, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_request_job.restype = ErrorCode
    lib.pp_transaction_claim_job.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int64, ctypes.c_int64, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_claim_job.restype = ErrorCode
    lib.pp_transaction_renew_job_claim.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.c_int64, ctypes.c_int64, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_renew_job_claim.restype = ErrorCode
    lib.pp_transaction_release_job_claim.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_release_job_claim.restype = ErrorCode
    lib.pp_transaction_complete_job.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.c_int64, ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_complete_job.restype = ErrorCode
    lib.pp_transaction_fail_job.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.POINTER(Uuid), ctypes.c_int64, ctypes.c_char_p, ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_fail_job.restype = ErrorCode
    lib.pp_transaction_cancel_job.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_cancel_job.restype = ErrorCode
    lib.pp_transaction_create_activity.argtypes = [ctypes.POINTER(Transaction), ctypes.c_char_p, ctypes.POINTER(ActivityEdge), ctypes.c_uint64, ctypes.POINTER(ActivityEdge), ctypes.c_uint64, ctypes.POINTER(ctypes.c_int64), ctypes.POINTER(ctypes.c_int64), ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(Uuid), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_create_activity.restype = ErrorCode
    lib.pp_transaction_commit.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_commit.restype = ErrorCode
    lib.pp_transaction_commit_with_receipt.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(CommitReceipt), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_commit_with_receipt.restype = ErrorCode
    lib.pp_transaction_rollback.argtypes = [ctypes.POINTER(Transaction), ctypes.POINTER(ctypes.POINTER(Error))]
    lib.pp_transaction_rollback.restype = ErrorCode
    lib.pp_transaction_release.argtypes = [ctypes.POINTER(Transaction)]
    lib.pp_transaction_release.restype = None
    lib.pp_error_code.argtypes = [ctypes.POINTER(Error)]
    lib.pp_error_code.restype = ErrorCode
    lib.pp_error_message.argtypes = [ctypes.POINTER(Error)]
    lib.pp_error_message.restype = ctypes.c_char_p
    lib.pp_error_transaction_conflict.argtypes = [ctypes.POINTER(Error), ctypes.POINTER(TransactionConflict)]
    lib.pp_error_transaction_conflict.restype = ctypes.c_uint8
    lib.pp_error_release.argtypes = [ctypes.POINTER(Error)]
    lib.pp_error_release.restype = None
