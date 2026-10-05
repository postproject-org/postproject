#ifndef POSTPROJECT_POSTPROJECT_H
#define POSTPROJECT_POSTPROJECT_H

#include <stdint.h>

#if defined(_WIN32) && defined(POSTPROJECT_SHARED)
#if defined(POSTPROJECT_BUILDING_LIBRARY)
#define PP_API __declspec(dllexport)
#else
#define PP_API __declspec(dllimport)
#endif
#elif defined(__GNUC__) && defined(POSTPROJECT_SHARED)
#define PP_API __attribute__((visibility("default")))
#else
#define PP_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct pp_production pp_production_t;
typedef struct pp_read_session pp_read_session_t;
typedef struct pp_transaction pp_transaction_t;
typedef struct pp_asset_set pp_asset_set_t;
typedef struct pp_media_root_set pp_media_root_set_t;
typedef struct pp_representation_set pp_representation_set_t;
typedef struct pp_resolution_set pp_resolution_set_t;
typedef struct pp_external_identifier_set pp_external_identifier_set_t;
typedef struct pp_object_ref_set pp_object_ref_set_t;
typedef struct pp_object_query_set pp_object_query_set_t;
typedef struct pp_locator_query_set pp_locator_query_set_t;
typedef struct pp_known_media_set pp_known_media_set_t;
typedef struct pp_metadata_set pp_metadata_set_t;
typedef struct pp_metadata_value pp_metadata_value_t;
typedef struct pp_metadata_input pp_metadata_input_t;
typedef struct pp_activity_set pp_activity_set_t;
typedef struct pp_job_set pp_job_set_t;
typedef struct pp_regeneration_plan_set pp_regeneration_plan_set_t;
typedef struct pp_dependency_set pp_dependency_set_t;
typedef struct pp_dependency_query_set pp_dependency_query_set_t;
typedef struct pp_artifact_evaluation pp_artifact_evaluation_t;
typedef struct pp_artifact_reproducibility pp_artifact_reproducibility_t;
typedef struct pp_revision_set pp_revision_set_t;
typedef struct pp_revision_event_set pp_revision_event_set_t;
typedef struct pp_revision_waiter pp_revision_waiter_t;
typedef struct pp_fingerprint pp_fingerprint_t;
typedef struct pp_cancel_token pp_cancel_token_t;
typedef struct pp_resolution_options pp_resolution_options_t;
typedef struct pp_media_source pp_media_source_t;
typedef struct pp_error pp_error_t;

/* Production handles may be moved between threads and called concurrently;
 * calls on one handle serialize internally. Transaction handles require
 * caller-side serialization. No handle may be released while another thread
 * is using it. Result-set and error handles are caller-serialized. Revision
 * waiters are caller-serialized except pp_revision_waiter_cancel(), which any
 * thread may call while another thread waits. A blocked wait never holds its
 * production, so other threads keep using and may release the production. */

typedef struct pp_uuid {
  uint8_t bytes[16];
} pp_uuid_t;

/* A production ID is a distinct C type, with the same stable UUID bytes.
 * Parsing accepts the core's UUID syntax, including nil. A value alone never
 * proves existence or production membership; operations validate those facts. */
typedef struct pp_production_id_value {
  uint8_t bytes[16];
} pp_production_id_t;

/* Distinct semantic identity; UUID bytes and nil policy match the core. */
typedef struct pp_revision_id_value {
  uint8_t bytes[16];
} pp_revision_id_t;

/* Distinct semantic identity; UUID bytes and nil policy match the core. */
typedef struct pp_transaction_id_value {
  uint8_t bytes[16];
} pp_transaction_id_t;

/* Distinct semantic identity; UUID bytes and nil policy match the core. */
typedef struct pp_asset_id_value {
  uint8_t bytes[16];
} pp_asset_id_t;

typedef uint32_t pp_commit_outcome_t;
#define PP_COMMIT_NO_CHANGE UINT32_C(0)
#define PP_COMMIT_REVISION_CREATED UINT32_C(1)

/* Owned stack result of one successful commit. revision_id and sequence are
 * meaningful only for PP_COMMIT_REVISION_CREATED; no-change makes no claim
 * about the current head. Failure initializes the entire output to zero. */
typedef struct pp_commit_receipt {
  pp_production_id_t production_id;
  pp_commit_outcome_t outcome;
  pp_revision_id_t revision_id;
  uint64_t revision_sequence;
} pp_commit_receipt_t;

/* Detached optimistic context. No revision with sequence zero means an empty
 * journal; it still protects edits. This value retains no view or lock. */
typedef struct pp_decision_base {
  pp_production_id_t production_id;
  uint8_t has_revision;
  pp_revision_id_t revision_id;
  uint64_t revision_sequence;
} pp_decision_base_t;
typedef uint32_t pp_object_kind_t;

#define PP_OBJECT_PRODUCTION UINT32_C(1)
#define PP_OBJECT_ASSET UINT32_C(2)
#define PP_OBJECT_REPRESENTATION UINT32_C(3)
#define PP_OBJECT_RESOURCE UINT32_C(4)
#define PP_OBJECT_ACTIVITY UINT32_C(5)
#define PP_OBJECT_JOB UINT32_C(6)

typedef uint32_t pp_representation_kind_t;

#define PP_REPRESENTATION_ORIGINAL UINT32_C(1)
#define PP_REPRESENTATION_PROXY UINT32_C(2)
#define PP_REPRESENTATION_OPTIMIZED UINT32_C(3)
#define PP_REPRESENTATION_DERIVED UINT32_C(4)

typedef uint32_t pp_job_state_t;

#define PP_JOB_REQUESTED UINT32_C(1)
#define PP_JOB_CLAIMED UINT32_C(2)
#define PP_JOB_SUCCEEDED UINT32_C(3)
#define PP_JOB_FAILED UINT32_C(4)
#define PP_JOB_CANCELLED UINT32_C(5)

typedef uint32_t pp_content_structure_kind_t;

#define PP_CONTENT_SINGLE_RESOURCE UINT32_C(1)
#define PP_CONTENT_IMAGE_SEQUENCE UINT32_C(2)
#define PP_CONTENT_ORDERED_PARTS UINT32_C(3)
#define PP_CONTENT_PACKAGE UINT32_C(4)

typedef uint32_t pp_locator_availability_t;

#define PP_LOCATOR_UNKNOWN UINT32_C(1)
#define PP_LOCATOR_ONLINE UINT32_C(2)
#define PP_LOCATOR_OFFLINE UINT32_C(3)

typedef uint32_t pp_revision_event_kind_t;

#define PP_REVISION_ASSET_IMPORTED UINT32_C(1)
#define PP_REVISION_REPRESENTATION_ADDED UINT32_C(2)
#define PP_REVISION_RESOURCE_ADDED UINT32_C(3)
#define PP_REVISION_REPRESENTATION_RESOURCE_ADDED UINT32_C(4)
#define PP_REVISION_LOCATOR_ADDED UINT32_C(5)
#define PP_REVISION_MEDIA_ROOT_ADDED UINT32_C(6)
#define PP_REVISION_EXTERNAL_IDENTIFIER_ADDED UINT32_C(7)
#define PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED UINT32_C(8)
#define PP_REVISION_METADATA_ADDED_OR_REPLACED UINT32_C(9)
#define PP_REVISION_METADATA_REMOVED UINT32_C(10)
#define PP_REVISION_ACTIVITY_CREATED UINT32_C(11)
#define PP_REVISION_ACTIVITY_INPUT_ADDED UINT32_C(12)
#define PP_REVISION_ACTIVITY_OUTPUT_ADDED UINT32_C(13)
#define PP_REVISION_LOCATOR_RETIRED UINT32_C(14)
#define PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED UINT32_C(15)
#define PP_REVISION_MEDIA_ROOT_REMOVED UINT32_C(16)
#define PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED UINT32_C(17)
#define PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED UINT32_C(18)
#define PP_REVISION_DEPENDENCY_SET_RECORDED UINT32_C(19)
#define PP_REVISION_JOB_REQUESTED UINT32_C(20)
#define PP_REVISION_JOB_CLAIMED UINT32_C(21)
#define PP_REVISION_JOB_CLAIM_RENEWED UINT32_C(22)
#define PP_REVISION_JOB_CLAIM_RELEASED UINT32_C(23)
#define PP_REVISION_JOB_SUCCEEDED UINT32_C(24)
#define PP_REVISION_JOB_FAILED UINT32_C(25)
#define PP_REVISION_JOB_CANCELLED UINT32_C(26)

typedef uint32_t pp_revision_wait_result_t;

#define PP_REVISION_WAIT_REVISIONS UINT32_C(1)
#define PP_REVISION_WAIT_TIMED_OUT UINT32_C(2)
#define PP_REVISION_WAIT_CLOSED UINT32_C(3)
#define PP_REVISION_WAIT_CANCELLED UINT32_C(4)

/* Longest timeout accepted by pp_revision_waiter_wait(). */
#define PP_REVISION_WAIT_MAX_TIMEOUT_MILLIS UINT32_C(60000)

typedef uint32_t pp_artifact_knowledge_state_t;

#define PP_ARTIFACT_CURRENT UINT32_C(1)
#define PP_ARTIFACT_STALE UINT32_C(2)
#define PP_ARTIFACT_INDETERMINATE UINT32_C(3)
#define PP_ARTIFACT_DIVERGED UINT32_C(4)

typedef uint32_t pp_artifact_edge_kind_t;

#define PP_ARTIFACT_EDGE_INPUT UINT32_C(1)
#define PP_ARTIFACT_EDGE_OUTPUT UINT32_C(2)

typedef uint32_t pp_artifact_reason_kind_t;

#define PP_ARTIFACT_REASON_PRODUCING_ACTIVITY_MISSING UINT32_C(1)
#define PP_ARTIFACT_REASON_PRODUCING_ACTIVITY_AMBIGUOUS UINT32_C(2)
#define PP_ARTIFACT_REASON_SNAPSHOT_ABSENT UINT32_C(3)
#define PP_ARTIFACT_REASON_FINGERPRINT_EVIDENCE_MISSING UINT32_C(4)
#define PP_ARTIFACT_REASON_FINGERPRINT_CHANGED UINT32_C(5)
#define PP_ARTIFACT_REASON_FINGERPRINT_RECOMPUTATION_PENDING UINT32_C(6)
#define PP_ARTIFACT_REASON_UPSTREAM_NOT_CURRENT UINT32_C(7)
#define PP_ARTIFACT_REASON_TRAVERSAL_TRUNCATED UINT32_C(8)
#define PP_ARTIFACT_REASON_DEPENDENCY_SNAPSHOT_ABSENT UINT32_C(9)
#define PP_ARTIFACT_REASON_DEPENDENCY_KNOWLEDGE_INCOMPLETE UINT32_C(10)
#define PP_ARTIFACT_REASON_DEPENDENCY_PATH_CHANGED UINT32_C(11)
#define PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_CHANGED UINT32_C(12)
#define PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_RECOMPUTATION_PENDING UINT32_C(13)
#define PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_EVIDENCE_MISSING UINT32_C(14)

typedef uint32_t pp_artifact_dependency_issue_t;

#define PP_ARTIFACT_DEPENDENCY_NEEDS_EXTRACTION UINT32_C(1)
#define PP_ARTIFACT_DEPENDENCY_UNRESOLVED UINT32_C(2)
#define PP_ARTIFACT_DEPENDENCY_DEPTH_TRUNCATED UINT32_C(3)
#define PP_ARTIFACT_DEPENDENCY_REPRESENTATIONS_TRUNCATED UINT32_C(4)

typedef uint32_t pp_dependency_set_status_t;

#define PP_DEPENDENCY_SET_CURRENT UINT32_C(1)
#define PP_DEPENDENCY_SET_NEEDS_EXTRACTION UINT32_C(2)

typedef uint32_t pp_artifact_traversal_limit_t;

#define PP_ARTIFACT_TRAVERSAL_DEPTH UINT32_C(1)
#define PP_ARTIFACT_TRAVERSAL_REPRESENTATIONS UINT32_C(2)

typedef uint32_t pp_artifact_reproducibility_issue_kind_t;

#define PP_ARTIFACT_REPRODUCIBILITY_PRODUCING_ACTIVITY_MISSING UINT32_C(1)
#define PP_ARTIFACT_REPRODUCIBILITY_PRODUCING_ACTIVITY_AMBIGUOUS UINT32_C(2)
#define PP_ARTIFACT_REPRODUCIBILITY_TOOL_IDENTITY_MISSING UINT32_C(3)
#define PP_ARTIFACT_REPRODUCIBILITY_PARAMETERS_MISSING UINT32_C(4)
#define PP_ARTIFACT_REPRODUCIBILITY_INPUT_REPRESENTATION_MISSING UINT32_C(5)

typedef uint32_t pp_metadata_value_kind_t;

#define PP_METADATA_STRING UINT32_C(1)
#define PP_METADATA_LANG_STRING UINT32_C(2)
#define PP_METADATA_I64 UINT32_C(3)
#define PP_METADATA_U64 UINT32_C(4)
#define PP_METADATA_DECIMAL UINT32_C(5)
#define PP_METADATA_BOOL UINT32_C(6)
#define PP_METADATA_TIMESTAMP UINT32_C(7)
#define PP_METADATA_URI UINT32_C(8)
#define PP_METADATA_BYTES UINT32_C(9)
#define PP_METADATA_RATIONAL UINT32_C(10)
#define PP_METADATA_LIST UINT32_C(11)
#define PP_METADATA_STRUCT UINT32_C(12)
#define PP_METADATA_REFERENCE UINT32_C(13)

typedef struct pp_object_ref {
  pp_object_kind_t kind;
  pp_uuid_t id;
} pp_object_ref_t;

typedef uint32_t pp_conflict_key_kind_t;

#define PP_CONFLICT_LOCATOR_SET UINT32_C(1)
#define PP_CONFLICT_METADATA_PROPERTY UINT32_C(2)
#define PP_CONFLICT_DEPENDENCY_SET UINT32_C(3)
#define PP_CONFLICT_MEDIA_ROOT UINT32_C(4)
#define PP_CONFLICT_EXTERNAL_IDENTIFIER UINT32_C(5)
#define PP_CONFLICT_RESOURCE_FINGERPRINT UINT32_C(6)
#define PP_CONFLICT_REPRESENTATION_FINGERPRINT UINT32_C(7)

/* Structured optimistic-conflict detail borrowed from one pp_error_t. The
 * target ID names the affected resource, representation, metadata/identifier
 * object, or media root. Media-root targets have object kind zero. Namespace
 * is a vocabulary, identifier scheme, or fingerprint algorithm; local_name is
 * a metadata property or external-identifier value. Unused fields are zero or
 * NULL. */
typedef struct pp_transaction_conflict {
  pp_conflict_key_kind_t kind;
  pp_object_ref_t target;
  const char *namespace_name;
  const char *local_name;
  const char *qualifier;
  uint16_t version;
  uint8_t has_base_revision;
  pp_revision_id_t base_revision_id;
  uint64_t base_revision_sequence;
  pp_revision_id_t superseding_revision_id;
  uint64_t superseding_revision_sequence;
} pp_transaction_conflict_t;

typedef struct pp_dependency_match {
  pp_object_ref_t target;
  uint32_t depth;
} pp_dependency_match_t;

/* Fields not used by an event kind are zero or NULL. String pointers borrow
 * the owning pp_revision_event_set_t. */
typedef struct pp_revision_event {
  pp_revision_event_kind_t kind;
  uint32_t position;
  pp_uuid_t asset_id;
  pp_uuid_t representation_id;
  pp_uuid_t resource_id;
  pp_uuid_t locator_id;
  pp_uuid_t media_root_id;
  pp_uuid_t activity_id;
  pp_uuid_t job_id;
  pp_object_ref_t target;
  uint32_t structural_position;
  uint8_t enabled;
  const char *identifier_scheme;
  const char *identifier_value;
  const char *identifier_qualifier;
  const char *vocabulary;
  const char *property;
  const char *activity_kind;
  const char *role;
  const char *fingerprint_algorithm;
  uint16_t fingerprint_version;
} pp_revision_event_t;

typedef struct pp_activity_edge {
  pp_uuid_t representation_id;
  const char *role;
} pp_activity_edge_t;

/* Strings borrow the owning pp_job_set_t. State-specific fields are zero or
 * NULL outside their applicable state. */
typedef struct pp_job {
  pp_uuid_t id;
  const char *kind;
  pp_uuid_t output_asset_id;
  pp_representation_kind_t output_representation_kind;
  const char *target_root;
  pp_job_state_t state;
  uint64_t input_count;
  pp_uuid_t claim_id;
  int64_t claim_expires_at_unix_micros;
  const char *claim_tool_name;
  const char *claim_tool_version;
  const char *claim_tool_uri;
  const char *claim_agent_name;
  const char *claim_agent_identifier_scheme;
  const char *claim_agent_identifier_value;
  const char *claim_agent_identifier_qualifier;
  pp_uuid_t completion_activity_id;
  pp_uuid_t completion_representation_id;
  const char *failure_diagnostic;
} pp_job_t;

/* Checked claimed-state detail. Strings borrow the owning pp_job_set_t. */
typedef struct pp_job_claim {
  pp_uuid_t id;
  int64_t expires_at_unix_micros;
  const char *tool_name;
  const char *tool_version;
  const char *tool_uri;
  const char *agent_name;
  const char *agent_identifier_scheme;
  const char *agent_identifier_value;
  const char *agent_identifier_qualifier;
} pp_job_claim_t;

/* Checked succeeded-state detail. */
typedef struct pp_job_completion {
  pp_uuid_t activity_id;
  pp_uuid_t representation_id;
} pp_job_completion_t;

/* Input strings are borrowed for a transaction call. Output strings borrow
 * the owning pp_dependency_set_t. */
typedef struct pp_dependency {
  uint8_t has_source_resource;
  pp_uuid_t source_resource_id;
  const char *kind;
  pp_object_ref_t target;
  uint8_t has_resolved_representation;
  pp_uuid_t resolved_representation_id;
  uint8_t required;
  const char *authored_reference;
} pp_dependency_t;

/* Strings borrow the owning artifact evaluation. */
typedef struct pp_artifact_dependency_path_segment {
  pp_uuid_t source_representation_id;
  uint32_t dependency_position;
  uint8_t has_source_resource;
  pp_uuid_t source_resource_id;
  const char *kind;
  pp_object_ref_t target;
  uint8_t has_resolved_representation;
  pp_uuid_t resolved_representation_id;
  const char *authored_reference;
} pp_artifact_dependency_path_segment_t;

/* String and byte pointers borrow the owning evaluation. Fields not used by a
 * reason kind are zero or NULL. */
typedef struct pp_artifact_reason {
  pp_artifact_reason_kind_t kind;
  pp_uuid_t activity_id;
  pp_uuid_t representation_id;
  pp_uuid_t input_representation_id;
  pp_artifact_edge_kind_t edge_kind;
  pp_artifact_knowledge_state_t upstream_state;
  pp_artifact_traversal_limit_t traversal_limit;
  uint32_t activity_count;
  pp_artifact_dependency_issue_t dependency_issue;
  const pp_artifact_dependency_path_segment_t *dependency_path;
  uint64_t dependency_path_length;
  const char *fingerprint_algorithm;
  uint16_t fingerprint_version;
  uint8_t has_snapshot_value;
  const uint8_t *snapshot_value;
  uint64_t snapshot_value_length;
  uint8_t has_current_value;
  const uint8_t *current_value;
  uint64_t current_value_length;
} pp_artifact_reason_t;

typedef struct pp_artifact_reproducibility_issue {
  pp_artifact_reproducibility_issue_kind_t kind;
  pp_uuid_t activity_id;
  pp_uuid_t representation_id;
  uint32_t activity_count;
} pp_artifact_reproducibility_issue_t;

typedef struct pp_file_resource_input {
  const char *path;
  const char *role;
  uint8_t required;
} pp_file_resource_input_t;

/* How the files of an image sequence are named in one directory: prefix, frame
 * number zero-padded to at least padding digits, and suffix. A naming belongs
 * to a locator: every locator of an image-sequence resource has one, and
 * copies of one sequence may name their files differently. Strings in an input
 * naming are borrowed UTF-8 without embedded NUL; strings in an output naming
 * borrow the owning result set. */
typedef struct pp_sequence_naming {
  const char *prefix;
  const char *suffix;
  uint8_t padding;
} pp_sequence_naming_t;

typedef uint32_t pp_error_code_t;

#define PP_OK UINT32_C(0)
#define PP_ERROR_INVALID_ARGUMENT UINT32_C(1)
#define PP_ERROR_NOT_FOUND UINT32_C(2)
#define PP_ERROR_ALREADY_EXISTS UINT32_C(3)
#define PP_ERROR_IO UINT32_C(4)
#define PP_ERROR_STORAGE UINT32_C(5)
#define PP_ERROR_MIGRATION UINT32_C(6)
#define PP_ERROR_CONFLICT UINT32_C(7)
#define PP_ERROR_AMBIGUOUS_RESOLUTION UINT32_C(8)
#define PP_ERROR_FINGERPRINT UINT32_C(9)
#define PP_ERROR_UNSUPPORTED UINT32_C(10)
#define PP_ERROR_CANCELLED UINT32_C(11)
#define PP_ERROR_INTERNAL UINT32_C(255)

typedef uint32_t pp_representation_availability_t;

#define PP_AVAILABILITY_ONLINE UINT32_C(1)
#define PP_AVAILABILITY_PARTIAL UINT32_C(2)
#define PP_AVAILABILITY_OFFLINE UINT32_C(3)
#define PP_AVAILABILITY_AMBIGUOUS UINT32_C(4)
#define PP_AVAILABILITY_ERROR UINT32_C(5)

typedef uint32_t pp_resource_resolution_state_t;

#define PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR UINT32_C(1)
#define PP_RESOURCE_RESOLVED_EXACT UINT32_C(2)
#define PP_RESOURCE_RESOLVED_PROBABLE UINT32_C(3)
#define PP_RESOURCE_OFFLINE UINT32_C(4)
#define PP_RESOURCE_AMBIGUOUS UINT32_C(5)
#define PP_RESOURCE_RESOLUTION_ERROR UINT32_C(6)

typedef uint32_t pp_availability_issue_kind_t;

#define PP_AVAILABILITY_ISSUE_OFFLINE_RESOURCE UINT32_C(1)
#define PP_AVAILABILITY_ISSUE_AMBIGUOUS_RESOURCE UINT32_C(2)
#define PP_AVAILABILITY_ISSUE_RESOURCE_ERROR UINT32_C(3)
#define PP_AVAILABILITY_ISSUE_MISSING_FRAMES UINT32_C(4)

typedef uint32_t pp_evidence_kind_t;

#define PP_EVIDENCE_KNOWN_LOCATOR_AVAILABLE UINT32_C(1)
#define PP_EVIDENCE_EXACT_FINGERPRINT_MATCH UINT32_C(2)
#define PP_EVIDENCE_FULL_HASH_MATCH UINT32_C(3)
#define PP_EVIDENCE_PARTIAL_FINGERPRINT_MATCH UINT32_C(4)
#define PP_EVIDENCE_FILE_SIZE_MATCH UINT32_C(5)
#define PP_EVIDENCE_FILE_NAME_MATCH UINT32_C(6)
#define PP_EVIDENCE_RELATIVE_PATH_SIMILARITY UINT32_C(7)
#define PP_EVIDENCE_MEDIA_ROOT_RELATION UINT32_C(8)
#define PP_EVIDENCE_CONFLICTING_CANDIDATE UINT32_C(9)
#define PP_EVIDENCE_DISCOVERY_ERROR UINT32_C(10)
#define PP_EVIDENCE_MEDIA_ROOT_UNMAPPED UINT32_C(11)
#define PP_EVIDENCE_MEDIA_ROOT_UNAVAILABLE UINT32_C(12)
#define PP_EVIDENCE_FINGERPRINT_MISMATCH UINT32_C(13)
#define PP_EVIDENCE_FINGERPRINT_NOT_VERIFIED UINT32_C(14)
#define PP_EVIDENCE_SEARCH_TRUNCATED UINT32_C(15)

typedef uint32_t pp_verification_mode_t;

#define PP_VERIFY_PRESENCE UINT32_C(1)
#define PP_VERIFY_CONTENT UINT32_C(2)

typedef uint32_t pp_content_verification_t;

#define PP_CONTENT_MATCHES UINT32_C(1)
#define PP_CONTENT_DIFFERS UINT32_C(2)
#define PP_CONTENT_NOT_COMPARABLE UINT32_C(3)

typedef uint32_t pp_content_observation_t;

#define PP_OBSERVATION_UNCHANGED UINT32_C(1)
#define PP_OBSERVATION_CHANGED UINT32_C(2)
#define PP_OBSERVATION_FIRST UINT32_C(3)

/** Parse a canonical production-scoped decision token (at most 128 bytes).
 * Store membership and revision/sequence are checked when beginning an edit. */
PP_API pp_error_code_t pp_decision_base_parse(
    const char *text, pp_decision_base_t *out_base, pp_error_t **out_error);
/** Format a validated base. Caller releases the owned text with pp_string_release. */
PP_API pp_error_code_t pp_decision_base_format(
    const pp_decision_base_t *base, char **out_text, pp_error_t **out_error);

/* Sessions own a pinned read connection and retain their production for edits.
 * All session calls are caller-serialized. Returned sets own their copied
 * values and survive session release. Release accepts NULL, exactly once. */
PP_API pp_error_code_t pp_production_read_session(
    const pp_production_t *production, pp_read_session_t **out_session,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_decision_base(
    const pp_read_session_t *session, pp_decision_base_t *out_base,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_begin_edit(
    const pp_read_session_t *session, pp_transaction_t **out_transaction,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_begin_edit(
    const pp_production_t *production, const pp_decision_base_t *base,
    pp_transaction_t **out_transaction, pp_error_t **out_error);
/* These snapshot operations inspect stored knowledge, without media I/O.
 * Returned reports own their values and survive session release. */
/* At most 100000 artifact IDs; planning does not enqueue or execute jobs. */
#define PP_MAX_REGENERATION_PLANS UINT32_C(100000)
PP_API pp_error_code_t pp_read_session_plan_regeneration(
    const pp_read_session_t *session,
    const pp_uuid_t *artifact_representation_ids, uint64_t artifact_count,
    pp_regeneration_plan_set_t **out_plans, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_evaluate_artifact(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    uint32_t max_depth, uint32_t max_representations,
    pp_artifact_evaluation_t **out_evaluation, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_artifact_reproducibility(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    pp_artifact_reproducibility_t **out_report, pp_error_t **out_error);
/* Journal reads end at the revision retained by this view. */
PP_API pp_error_code_t pp_read_session_revision_events_page(
    const pp_read_session_t *session, pp_revision_id_t revision_id,
    uint32_t limit, const char *cursor, pp_revision_event_set_t **out_events,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_changes_since_filtered(
    const pp_read_session_t *session, uint64_t sequence,
    const pp_revision_event_kind_t *kinds, uint64_t kind_count,
    uint32_t limit, pp_revision_set_t **out_revisions,
    uint64_t *out_through_sequence, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_latest_revision(
    const pp_read_session_t *session, pp_revision_set_t **out_revisions,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_changes_since(
    const pp_read_session_t *session, uint64_t sequence, uint32_t limit,
    pp_revision_set_t **out_revisions, pp_error_t **out_error);
/* Producing/consuming activity pages retain this view and cursor scope. */
PP_API pp_error_code_t pp_read_session_activities_producing_page(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    uint32_t limit, const char *cursor, pp_activity_set_t **out_activities,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_activities_consuming_page(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    uint32_t limit, const char *cursor, pp_activity_set_t **out_activities,
    pp_error_t **out_error);
/* Exact activity output filters use the same pinned view. */
PP_API pp_error_code_t pp_read_session_outputs_by_activity_kind(
    const pp_read_session_t *session, const char *kind, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_outputs_by_tool(
    const pp_read_session_t *session, const char *name, const char *version,
    const char *uri, uint32_t limit, const char *cursor,
    pp_object_query_set_t **out_objects, pp_error_t **out_error);
/* Provenance traversal and staleness use pinned knowledge and explicit bounds. */
PP_API pp_error_code_t pp_read_session_provenance_ancestors_page(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    uint32_t max_depth, uint32_t max_representations, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_provenance_descendants_page(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    uint32_t max_depth, uint32_t max_representations, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_stale_artifacts(
    const pp_read_session_t *session,
    const pp_uuid_t *source_representation_id,
    uint32_t evaluation_max_depth,
    uint32_t evaluation_max_representations, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
/* Filtered object pages retain this view and its journal head. */
PP_API pp_error_code_t pp_read_session_representations_under_media_root(
    const pp_read_session_t *session, const char *root_name,
    uint32_t limit, const char *cursor,
    pp_representation_set_t **out_representations, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_unresolved_media(
    const pp_read_session_t *session, uint32_t limit, const char *cursor,
    pp_object_query_set_t **out_objects, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_objects_changed_since(
    const pp_read_session_t *session, uint64_t sequence, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
/* Dependency knowledge and bounded traversal use the same pinned view. */
PP_API pp_error_code_t pp_read_session_dependency_set(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    pp_dependency_set_t **out_dependencies, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_dependencies(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    uint32_t max_depth, uint32_t max_representations, uint32_t limit,
    const char *cursor, pp_dependency_query_set_t **out_matches,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_dependents(
    const pp_read_session_t *session, const pp_object_ref_t *target,
    uint32_t max_depth, uint32_t max_representations, uint32_t limit,
    const char *cursor, pp_dependency_query_set_t **out_matches,
    pp_error_t **out_error);
/* Snapshot job reads return the same owned sets as live production reads.
 * Zero state and NULL kind select all jobs; pages retain this view's scope. */
PP_API pp_error_code_t pp_read_session_job(
    const pp_read_session_t *session, const pp_uuid_t *job_id,
    pp_job_set_t **out_jobs, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_jobs(
    const pp_read_session_t *session, pp_job_state_t state, const char *kind,
    uint32_t limit, const char *cursor, pp_job_set_t **out_jobs,
    pp_error_t **out_error);
PP_API void pp_read_session_release(pp_read_session_t *session);

/* Formatting returns owned text released with pp_string_release. Parse failure
 * clears out_id; format failure clears out_text. No registry lookup occurs. */
PP_API pp_error_code_t pp_asset_id_parse(
    const char *text, pp_asset_id_t *out_id, pp_error_t **out_error);
PP_API pp_error_code_t pp_asset_id_format(
    pp_asset_id_t id, char **out_text, pp_error_t **out_error);
/* Parse syntax and kind; existence is checked by domain operations. */
PP_API pp_error_code_t pp_revision_id_parse(
    const char *text, pp_revision_id_t *out_id, pp_error_t **out_error);
/* Owned canonical text; release it with pp_string_release(). */
PP_API pp_error_code_t pp_revision_id_format(
    pp_revision_id_t id, char **out_text, pp_error_t **out_error);
/* Parse syntax and kind; existence is checked by domain operations. */
PP_API pp_error_code_t pp_transaction_id_parse(
    const char *text, pp_transaction_id_t *out_id, pp_error_t **out_error);
/* Owned canonical text; release it with pp_string_release(). */
PP_API pp_error_code_t pp_transaction_id_format(
    pp_transaction_id_t id, char **out_text, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_id_parse(
    const char *text, pp_production_id_t *out_id, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_id_format(
    pp_production_id_t id, char **out_text, pp_error_t **out_error);

/* Read values and pages from the pinned view. Result sets own their data.
 * Cursor lifetime follows the existing owned sets; session scope is required. */
PP_API pp_error_code_t pp_read_session_assets_page(
    const pp_read_session_t *session, uint32_t limit, const char *cursor,
    pp_asset_set_t **out_assets, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_asset(
    const pp_read_session_t *session, const pp_uuid_t *asset_id,
    pp_asset_set_t **out_assets, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_representations_page(
    const pp_read_session_t *session, const pp_uuid_t *asset_id, uint32_t limit,
    const char *cursor, pp_representation_set_t **out_representations,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_representation(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    pp_representation_set_t **out_representations, pp_error_t **out_error);
/* Bounded storage evidence uses the pinned view. IDs/cursors borrow the input;
 * result sets own their values and survive session release. */
PP_API pp_error_code_t pp_read_session_resources_page(
    const pp_read_session_t *session, const pp_uuid_t *representation_id,
    uint32_t limit, const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_locators_page(
    const pp_read_session_t *session, const pp_uuid_t *resource_id,
    uint32_t limit, const char *cursor,
    pp_locator_query_set_t **out_locators, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_representations_using_resource(
    const pp_read_session_t *session, const pp_uuid_t *resource_id,
    uint32_t limit, const char *cursor,
    pp_representation_set_t **out_representations, pp_error_t **out_error);
/* These I/O reads use retained database facts but inspect the current
 * filesystem; they do not pin media bytes or record observations. */
PP_API pp_error_code_t pp_read_session_resolve_assets(
    const pp_read_session_t *session, const pp_uuid_t *asset_ids,
    uint64_t asset_count, const pp_resolution_options_t *options,
    pp_resolution_set_t **out_resolutions, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_verify_resource(
    const pp_read_session_t *session, const pp_uuid_t *resource_id,
    const char *path, const pp_sequence_naming_t *sequence_naming,
    pp_content_verification_t *out_verification, pp_error_t **out_error);
/* Same owned projections as the corresponding production reads, using this
 * session's pinned view. A NULL qualifier matches any qualifier. */
PP_API pp_error_code_t pp_read_session_media_roots(
    const pp_read_session_t *session, pp_media_root_set_t **out_roots,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_external_identifiers(
    const pp_read_session_t *session, const pp_object_ref_t *target,
    pp_external_identifier_set_t **out_identifiers, pp_error_t **out_error);
/* Metadata owns recursively copied values; paging retains this view's scope. */
PP_API pp_error_code_t pp_read_session_metadata(
    const pp_read_session_t *session, const pp_object_ref_t *target,
    pp_metadata_set_t **out_metadata, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_find_metadata(
    const pp_read_session_t *session, const char *vocabulary, const char *property,
    pp_metadata_set_t **out_metadata, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_query_metadata(
    const pp_read_session_t *session, const char *vocabulary, const char *property,
    const pp_metadata_input_t *exact_value, uint32_t limit, const char *cursor,
    pp_metadata_set_t **out_metadata, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_find_by_external_identifier(
    const pp_read_session_t *session, const char *scheme, const char *value,
    const char *qualifier, pp_object_ref_set_t **out_objects, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_find_known_media_by_locator(
    const pp_read_session_t *session, const char *uri,
    const pp_sequence_naming_t *sequence_naming, uint32_t limit,
    const char *cursor, pp_known_media_set_t **out_matches, pp_error_t **out_error);
PP_API pp_error_code_t pp_read_session_find_known_media_by_fingerprint(
    const pp_read_session_t *session, const char *algorithm, uint16_t version,
    const uint8_t *value, uint64_t value_length, uint32_t limit,
    const char *cursor, pp_known_media_set_t **out_matches, pp_error_t **out_error);

PP_API uint32_t pp_abi_version(void);
/* Host bindings are pure value operations and perform no network access.
 * Inputs are borrowed. On success, *out_binding is caller-owned and must be
 * released exactly once with pp_string_release(). */
PP_API pp_error_code_t pp_host_binding_format(
    pp_production_id_t production_id, const pp_object_ref_t *object,
    char **out_binding, pp_error_t **out_error);
/* binding is borrowed NUL-terminated UTF-8. Both value outputs are required
 * caller-owned storage and are cleared on failure. */
PP_API pp_error_code_t pp_host_binding_parse(
    const char *binding, pp_production_id_t *out_production_id,
    pp_object_ref_t *out_object, pp_error_t **out_error);
/* Returns the canonical file: locator URI import records for an existing path:
 * symbolic links and relative components are resolved, and the URI is spelled
 * as PostProject spells it. Compare locators only through URIs returned here or
 * read from a production. *out_uri is released with pp_string_release(). */
PP_API pp_error_code_t pp_file_path_to_locator(const char *path,
                                               char **out_uri,
                                               pp_error_t **out_error);
/* Converts a local file: URI to a native path, which need not exist.
 * *out_path is released with pp_string_release(). */
PP_API pp_error_code_t pp_locator_to_file_path(const char *uri,
                                               char **out_path,
                                               pp_error_t **out_error);
/* Releases a string returned through an owned char ** output. Accepts NULL.
 * Strings borrowed from result sets must never be passed here. */
PP_API void pp_string_release(char *value);
/* Inputs are borrowed UTF-8 without embedded NUL. A NULL display name is
 * absent. On success, *out_production is caller-owned and *out_error is NULL. On
 * failure, *out_production is NULL and a non-NULL *out_error is caller-owned.
 * out_error may itself be NULL when diagnostic text is not required. */
PP_API pp_error_code_t pp_production_create(const char *path,
                                         const char *display_name,
                                         pp_production_t **out_production,
                                         pp_error_t **out_error);
PP_API pp_error_code_t pp_production_open(const char *path,
                                       pp_production_t **out_production,
                                       pp_error_t **out_error);
PP_API pp_error_code_t pp_production_id(const pp_production_t *production,
                                     pp_production_id_t *out_id, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_asset_exists(const pp_production_t *production,
                                               const pp_uuid_t *asset_id,
                                               uint8_t *out_exists,
                                               pp_error_t **out_error);
/* Asset strings borrow the owning result set. */
PP_API pp_error_code_t pp_production_assets(
    const pp_production_t *production, pp_asset_set_t **out_assets,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_assets_page(
    const pp_production_t *production, uint32_t limit, const char *cursor,
    pp_asset_set_t **out_assets, pp_error_t **out_error);
/* Reads one asset as a one-element set. An absent asset is
 * PP_ERROR_NOT_FOUND. */
PP_API pp_error_code_t pp_production_asset(
    const pp_production_t *production, const pp_uuid_t *asset_id,
    pp_asset_set_t **out_assets, pp_error_t **out_error);
PP_API uint64_t pp_asset_set_count(const pp_asset_set_t *assets);
PP_API const char *pp_asset_set_next_cursor(const pp_asset_set_t *assets);
PP_API pp_error_code_t pp_asset_set_get(
    const pp_asset_set_t *assets, uint64_t index, pp_uuid_t *out_id,
    int64_t *out_created_at_unix_micros, const char **out_display_name,
    const char **out_import_source, pp_error_t **out_error);
PP_API void pp_asset_set_release(pp_asset_set_t *assets);
/* Media-root strings borrow the owning result set. Roots are ordered by
 * resolver priority and stable identity. */
PP_API pp_error_code_t pp_production_media_roots(
    const pp_production_t *production, pp_media_root_set_t **out_roots,
    pp_error_t **out_error);
PP_API uint64_t pp_media_root_set_count(const pp_media_root_set_t *roots);
PP_API pp_error_code_t pp_media_root_set_get(
    const pp_media_root_set_t *roots, uint64_t index, pp_uuid_t *out_id,
    const char **out_name, const char **out_label, const char **out_legacy_uri,
    int32_t *out_priority, uint8_t *out_enabled, pp_error_t **out_error);
PP_API void pp_media_root_set_release(pp_media_root_set_t *roots);
/* Representation strings borrow the owning result set. Members are returned in
 * structural order. Single-resource and image-sequence members have no role. */
PP_API pp_error_code_t pp_production_representations(
    const pp_production_t *production, const pp_uuid_t *asset_id,
    pp_representation_set_t **out_representations, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_representations_page(
    const pp_production_t *production, const pp_uuid_t *asset_id,
    uint32_t limit, const char *cursor,
    pp_representation_set_t **out_representations, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_representations_under_media_root(
    const pp_production_t *production, const char *root_name,
    uint32_t limit, const char *cursor,
    pp_representation_set_t **out_representations, pp_error_t **out_error);
/* Reads one representation as a one-element set. An absent representation is
 * PP_ERROR_NOT_FOUND. */
PP_API pp_error_code_t pp_production_representation(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    pp_representation_set_t **out_representations, pp_error_t **out_error);
/* Pages the representations that use a resource, in identity order. */
PP_API pp_error_code_t pp_production_representations_using_resource(
    const pp_production_t *production, const pp_uuid_t *resource_id,
    uint32_t limit, const char *cursor,
    pp_representation_set_t **out_representations, pp_error_t **out_error);
PP_API uint64_t pp_representation_set_count(
    const pp_representation_set_t *representations);
PP_API const char *pp_representation_set_next_cursor(
    const pp_representation_set_t *representations);
PP_API pp_error_code_t pp_representation_set_get(
    const pp_representation_set_t *representations, uint64_t index,
    pp_uuid_t *out_id, pp_uuid_t *out_asset_id,
    pp_representation_kind_t *out_kind,
    pp_content_structure_kind_t *out_structure_kind,
    uint64_t *out_member_count, uint64_t *out_resource_count,
    uint64_t *out_fingerprint_count, pp_error_t **out_error);
PP_API pp_error_code_t pp_representation_set_get_fingerprint(
    const pp_representation_set_t *representations,
    uint64_t representation_index, uint64_t fingerprint_index,
    const char **out_algorithm, uint16_t *out_version,
    const uint8_t **out_value, uint64_t *out_value_length,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_representation_set_get_member(
    const pp_representation_set_t *representations,
    uint64_t representation_index, uint64_t member_index,
    pp_uuid_t *out_resource_id, const char **out_role, uint8_t *out_required,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_representation_set_get_sequence(
    const pp_representation_set_t *representations,
    uint64_t representation_index, int64_t *out_start, int64_t *out_end,
    uint32_t *out_step, uint32_t *out_rate_numerator,
    uint32_t *out_rate_denominator, uint64_t *out_missing_count,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_representation_set_get_sequence_missing_frame(
    const pp_representation_set_t *representations,
    uint64_t representation_index, uint64_t frame_index, int64_t *out_frame,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_representation_set_get_resource(
    const pp_representation_set_t *representations,
    uint64_t representation_index, uint64_t resource_index, pp_uuid_t *out_id,
    uint8_t *out_has_file_facts, uint64_t *out_file_size,
    uint8_t *out_has_modified_at, int64_t *out_modified_at_unix_micros,
    uint64_t *out_locator_count, uint64_t *out_fingerprint_count,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_representation_set_get_resource_fingerprint(
    const pp_representation_set_t *representations,
    uint64_t representation_index, uint64_t resource_index,
    uint64_t fingerprint_index, const char **out_algorithm,
    uint16_t *out_version, const uint8_t **out_value,
    uint64_t *out_value_length, pp_error_t **out_error);
/* *out_has_sequence_naming is 1 exactly for a locator of an image-sequence
 * resource, and *out_sequence_naming then names the files in its directory. */
PP_API pp_error_code_t pp_representation_set_get_locator(
    const pp_representation_set_t *representations,
    uint64_t representation_index, uint64_t resource_index,
    uint64_t locator_index, pp_uuid_t *out_id, const char **out_uri,
    pp_locator_availability_t *out_availability, uint8_t *out_has_last_seen,
    int64_t *out_last_seen_unix_micros, uint8_t *out_has_sequence_naming,
    pp_sequence_naming_t *out_sequence_naming, pp_error_t **out_error);
PP_API void pp_representation_set_release(
    pp_representation_set_t *representations);
/* Result strings are borrowed until the owning result set is released. */
PP_API pp_error_code_t pp_production_external_identifiers(
    const pp_production_t *production, const pp_object_ref_t *target,
    pp_external_identifier_set_t **out_identifiers, pp_error_t **out_error);
/* A NULL qualifier matches any qualifier, including none; otherwise only
 * identifiers with exactly that qualifier match. */
PP_API pp_error_code_t pp_production_find_by_external_identifier(
    const pp_production_t *production, const char *scheme, const char *value,
    const char *qualifier, pp_object_ref_set_t **out_objects,
    pp_error_t **out_error);
PP_API uint64_t pp_external_identifier_set_count(
    const pp_external_identifier_set_t *identifiers);
PP_API pp_error_code_t pp_external_identifier_set_get(
    const pp_external_identifier_set_t *identifiers, uint64_t index,
    const char **out_scheme, const char **out_value, const char **out_qualifier,
    pp_error_t **out_error);
PP_API void pp_external_identifier_set_release(
    pp_external_identifier_set_t *identifiers);
PP_API uint64_t
pp_object_ref_set_count(const pp_object_ref_set_t *objects);
PP_API pp_error_code_t pp_object_ref_set_get(
    const pp_object_ref_set_t *objects, uint64_t index,
    pp_object_ref_t *out_object, pp_error_t **out_error);
PP_API void pp_object_ref_set_release(pp_object_ref_set_t *objects);
/* Paginated object queries return a depth of zero for non-traversal results.
 * Cursors and result strings borrow the owning set. */
PP_API uint64_t pp_object_query_set_count(
    const pp_object_query_set_t *objects);
PP_API pp_error_code_t pp_object_query_set_get(
    const pp_object_query_set_t *objects, uint64_t index,
    pp_object_ref_t *out_object, uint32_t *out_depth,
    pp_error_t **out_error);
PP_API const char *pp_object_query_set_next_cursor(
    const pp_object_query_set_t *objects);
PP_API uint8_t pp_object_query_set_traversal_truncated(
    const pp_object_query_set_t *objects);
PP_API void pp_object_query_set_release(pp_object_query_set_t *objects);
PP_API pp_error_code_t pp_production_resources_page(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    uint32_t limit, const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_locators_page(
    const pp_production_t *production, const pp_uuid_t *resource_id,
    uint32_t limit, const char *cursor,
    pp_locator_query_set_t **out_locators, pp_error_t **out_error);
PP_API uint64_t pp_locator_query_set_count(
    const pp_locator_query_set_t *locators);
PP_API pp_error_code_t pp_locator_query_set_get(
    const pp_locator_query_set_t *locators, uint64_t index,
    pp_uuid_t *out_id, pp_uuid_t *out_resource_id, const char **out_uri,
    pp_locator_availability_t *out_availability,
    uint8_t *out_has_last_seen, int64_t *out_last_seen_unix_micros,
    const char **out_media_root, uint8_t *out_has_sequence_naming,
    pp_sequence_naming_t *out_sequence_naming, pp_error_t **out_error);
PP_API const char *pp_locator_query_set_next_cursor(
    const pp_locator_query_set_t *locators);
PP_API void pp_locator_query_set_release(pp_locator_query_set_t *locators);
/* Finds current storage evidence only. Locator identity is the canonical URI
 * plus sequence_naming for an image sequence; a directory URI without naming
 * does not match a sequence. Fingerprint domains are open strings and values
 * are exact opaque bytes. Queries are read-only and return every ownership
 * candidate without adopting, relinking, or merging it. */
PP_API pp_error_code_t pp_production_find_known_media_by_locator(
    const pp_production_t *production, const char *uri,
    const pp_sequence_naming_t *sequence_naming, uint32_t limit,
    const char *cursor, pp_known_media_set_t **out_matches,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_find_known_media_by_fingerprint(
    const pp_production_t *production, const char *algorithm, uint16_t version,
    const uint8_t *value, uint64_t value_length, uint32_t limit,
    const char *cursor, pp_known_media_set_t **out_matches,
    pp_error_t **out_error);
PP_API uint64_t pp_known_media_set_count(
    const pp_known_media_set_t *matches);
PP_API const char *pp_known_media_set_next_cursor(
    const pp_known_media_set_t *matches);
PP_API pp_error_code_t pp_known_media_set_get(
    const pp_known_media_set_t *matches, uint64_t index,
    pp_uuid_t *out_asset_id, pp_uuid_t *out_representation_id,
    pp_uuid_t *out_resource_id, pp_error_t **out_error);
PP_API void pp_known_media_set_release(pp_known_media_set_t *matches);
PP_API pp_error_code_t pp_production_unresolved_media(
    const pp_production_t *production, uint32_t limit, const char *cursor,
    pp_object_query_set_t **out_objects, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_objects_changed_since(
    const pp_production_t *production, uint64_t sequence, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
/* Metadata result sets own every returned string and recursively typed value.
 * All pointers borrowed from a set become invalid when that set is released. */
PP_API pp_error_code_t pp_production_metadata(
    const pp_production_t *production, const pp_object_ref_t *target,
    pp_metadata_set_t **out_metadata, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_find_metadata(
    const pp_production_t *production, const char *vocabulary, const char *property,
    pp_metadata_set_t **out_metadata, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_query_metadata(
    const pp_production_t *production, const char *vocabulary,
    const char *property, const pp_metadata_input_t *exact_value,
    uint32_t limit, const char *cursor, pp_metadata_set_t **out_metadata,
    pp_error_t **out_error);
PP_API uint64_t pp_metadata_set_count(const pp_metadata_set_t *metadata);
PP_API const char *pp_metadata_set_next_cursor(
    const pp_metadata_set_t *metadata);
PP_API pp_error_code_t pp_metadata_set_get(
    const pp_metadata_set_t *metadata, uint64_t index,
    pp_object_ref_t *out_target, const char **out_vocabulary,
    const char **out_property, const pp_metadata_value_t **out_value,
    pp_error_t **out_error);
PP_API void pp_metadata_set_release(pp_metadata_set_t *metadata);
PP_API pp_metadata_value_kind_t
pp_metadata_value_kind(const pp_metadata_value_t *value);
PP_API pp_error_code_t pp_metadata_value_get_string(
    const pp_metadata_value_t *value, const char **out_text,
    const char **out_language, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_value_get_i64(
    const pp_metadata_value_t *value, int64_t *out_value,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_value_get_u64(
    const pp_metadata_value_t *value, uint64_t *out_value,
    pp_error_t **out_error);
/* Decimal coefficient is exact base-ten text borrowed from the result set. */
PP_API pp_error_code_t pp_metadata_value_get_decimal(
    const pp_metadata_value_t *value, const char **out_coefficient,
    uint32_t *out_scale, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_value_get_bool(
    const pp_metadata_value_t *value, uint8_t *out_value,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_value_get_timestamp(
    const pp_metadata_value_t *value, int64_t *out_unix_micros,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_value_get_uri(
    const pp_metadata_value_t *value, const char **out_uri,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_value_get_bytes(
    const pp_metadata_value_t *value, const uint8_t **out_bytes,
    uint64_t *out_length, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_value_get_rational(
    const pp_metadata_value_t *value, int64_t *out_numerator,
    uint64_t *out_denominator, pp_error_t **out_error);
PP_API uint64_t
pp_metadata_value_list_count(const pp_metadata_value_t *value);
PP_API pp_error_code_t pp_metadata_value_list_get(
    const pp_metadata_value_t *value, uint64_t index,
    const pp_metadata_value_t **out_item, pp_error_t **out_error);
PP_API uint64_t
pp_metadata_value_struct_count(const pp_metadata_value_t *value);
PP_API pp_error_code_t pp_metadata_value_struct_get(
    const pp_metadata_value_t *value, uint64_t index, const char **out_name,
    const pp_metadata_value_t **out_field_value, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_value_get_reference(
    const pp_metadata_value_t *value, pp_object_ref_t *out_reference,
    pp_error_t **out_error);
/* Metadata inputs are owned immutable values. Collection constructors borrow
 * children only for the call and copy them. */
PP_API pp_error_code_t pp_metadata_input_create_string(
    const char *text, const char *language, pp_metadata_input_t **out_input,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_i64(
    int64_t value, pp_metadata_input_t **out_input, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_u64(
    uint64_t value, pp_metadata_input_t **out_input, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_decimal(
    const char *coefficient, uint32_t scale, pp_metadata_input_t **out_input,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_bool(
    uint8_t value, pp_metadata_input_t **out_input, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_timestamp(
    int64_t unix_micros, pp_metadata_input_t **out_input,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_uri(
    const char *uri, pp_metadata_input_t **out_input, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_bytes(
    const uint8_t *bytes, uint64_t length, pp_metadata_input_t **out_input,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_rational(
    int64_t numerator, uint64_t denominator, pp_metadata_input_t **out_input,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_reference(
    const pp_object_ref_t *target, pp_metadata_input_t **out_input,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_list(
    const pp_metadata_input_t **items, uint64_t count,
    pp_metadata_input_t **out_input, pp_error_t **out_error);
PP_API pp_error_code_t pp_metadata_input_create_struct(
    const char **names, const pp_metadata_input_t **values, uint64_t count,
    pp_metadata_input_t **out_input, pp_error_t **out_error);
PP_API void pp_metadata_input_release(pp_metadata_input_t *input);
/* A successful read always returns a set handle. `out_present` distinguishes
 * absent knowledge from a recorded empty set. Strings borrow the set. */
PP_API pp_error_code_t pp_production_dependency_set(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    pp_dependency_set_t **out_dependencies, pp_error_t **out_error);
PP_API pp_error_code_t pp_dependency_set_get(
    const pp_dependency_set_t *dependencies, uint8_t *out_present,
    pp_uuid_t *out_source_representation_id,
    uint64_t *out_recorded_at_revision,
    pp_dependency_set_status_t *out_status, uint64_t *out_dependency_count,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_dependency_set_get_dependency(
    const pp_dependency_set_t *dependencies, uint64_t index,
    pp_dependency_t *out_dependency, pp_error_t **out_error);
PP_API void pp_dependency_set_release(pp_dependency_set_t *dependencies);
/* Dependency-query cursors and match views borrow the owning query set. A null
 * input cursor starts a query; a null next cursor marks the final page. */
PP_API pp_error_code_t pp_production_dependencies(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    uint32_t max_depth, uint32_t max_representations, uint32_t limit,
    const char *cursor, pp_dependency_query_set_t **out_matches,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_dependents(
    const pp_production_t *production, const pp_object_ref_t *target,
    uint32_t max_depth, uint32_t max_representations, uint32_t limit,
    const char *cursor, pp_dependency_query_set_t **out_matches,
    pp_error_t **out_error);
PP_API uint64_t pp_dependency_query_set_count(
    const pp_dependency_query_set_t *matches);
PP_API pp_error_code_t pp_dependency_query_set_get(
    const pp_dependency_query_set_t *matches, uint64_t index,
    pp_dependency_match_t *out_match, pp_error_t **out_error);
PP_API const char *pp_dependency_query_set_next_cursor(
    const pp_dependency_query_set_t *matches);
PP_API uint8_t pp_dependency_query_set_traversal_truncated(
    const pp_dependency_query_set_t *matches);
PP_API void pp_dependency_query_set_release(
    pp_dependency_query_set_t *matches);
/* Artifact evaluation is knowledge-only. Returned strings and byte spans
 * borrow their owning result handle. */
PP_API pp_error_code_t pp_production_evaluate_artifact(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    uint32_t max_depth, uint32_t max_representations,
    pp_artifact_evaluation_t **out_evaluation, pp_error_t **out_error);
PP_API pp_error_code_t pp_artifact_evaluation_get(
    const pp_artifact_evaluation_t *evaluation,
    pp_uuid_t *out_representation_id,
    pp_artifact_knowledge_state_t *out_state,
    uint32_t *out_visited_representations, uint8_t *out_truncated,
    uint64_t *out_reason_count, pp_error_t **out_error);
PP_API pp_error_code_t pp_artifact_evaluation_get_reason(
    const pp_artifact_evaluation_t *evaluation, uint64_t index,
    pp_artifact_reason_t *out_reason, pp_error_t **out_error);
PP_API void
pp_artifact_evaluation_release(pp_artifact_evaluation_t *evaluation);
PP_API pp_error_code_t pp_production_artifact_reproducibility(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    pp_artifact_reproducibility_t **out_report, pp_error_t **out_error);
PP_API pp_error_code_t pp_artifact_reproducibility_get(
    const pp_artifact_reproducibility_t *report,
    pp_uuid_t *out_representation_id, uint8_t *out_reproducible,
    uint8_t *out_has_producing_activity,
    pp_uuid_t *out_producing_activity_id, const char **out_activity_kind,
    uint64_t *out_issue_count, pp_error_t **out_error);
PP_API pp_error_code_t pp_artifact_reproducibility_get_issue(
    const pp_artifact_reproducibility_t *report, uint64_t index,
    pp_artifact_reproducibility_issue_t *out_issue,
    pp_error_t **out_error);
PP_API void pp_artifact_reproducibility_release(
    pp_artifact_reproducibility_t *report);
/* Activity strings are borrowed until pp_activity_set_release(). Optional
 * timestamps use explicit presence flags and zero values when absent. */
PP_API pp_error_code_t pp_production_activities(
    const pp_production_t *production, pp_activity_set_t **out_activities,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_activities_producing(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    pp_activity_set_t **out_activities, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_activities_consuming(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    pp_activity_set_t **out_activities, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_activities_producing_page(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    uint32_t limit, const char *cursor, pp_activity_set_t **out_activities,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_activities_consuming_page(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    uint32_t limit, const char *cursor, pp_activity_set_t **out_activities,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_outputs_by_activity_kind(
    const pp_production_t *production, const char *kind, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_outputs_by_tool(
    const pp_production_t *production, const char *name, const char *version,
    const char *uri, uint32_t limit, const char *cursor,
    pp_object_query_set_t **out_objects, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_provenance_ancestors(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    pp_object_ref_set_t **out_representations, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_provenance_descendants(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    pp_object_ref_set_t **out_representations, pp_error_t **out_error);
PP_API pp_error_code_t pp_production_provenance_ancestors_page(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    uint32_t max_depth, uint32_t max_representations, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_provenance_descendants_page(
    const pp_production_t *production, const pp_uuid_t *representation_id,
    uint32_t max_depth, uint32_t max_representations, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_stale_artifacts(
    const pp_production_t *production,
    const pp_uuid_t *source_representation_id,
    uint32_t evaluation_max_depth,
    uint32_t evaluation_max_representations, uint32_t limit,
    const char *cursor, pp_object_query_set_t **out_objects,
    pp_error_t **out_error);
PP_API uint64_t pp_activity_set_count(const pp_activity_set_t *activities);
PP_API const char *pp_activity_set_next_cursor(
    const pp_activity_set_t *activities);
PP_API pp_error_code_t pp_activity_set_get(
    const pp_activity_set_t *activities, uint64_t index, pp_uuid_t *out_id,
    const char **out_kind, uint8_t *out_has_started_at,
    int64_t *out_started_at_unix_micros, uint8_t *out_has_finished_at,
    int64_t *out_finished_at_unix_micros, uint64_t *out_input_count,
    uint64_t *out_output_count, pp_error_t **out_error);
PP_API pp_error_code_t pp_activity_set_get_tool(
    const pp_activity_set_t *activities, uint64_t index, const char **out_name,
    const char **out_version, const char **out_uri, pp_error_t **out_error);
PP_API pp_error_code_t pp_activity_set_get_agent(
    const pp_activity_set_t *activities, uint64_t index, const char **out_name,
    const char **out_identifier_scheme, const char **out_identifier_value,
    const char **out_identifier_qualifier, pp_error_t **out_error);
PP_API pp_error_code_t pp_activity_set_get_input(
    const pp_activity_set_t *activities, uint64_t activity_index,
    uint64_t input_index, pp_uuid_t *out_representation_id,
    const char **out_role, pp_error_t **out_error);
PP_API pp_error_code_t pp_activity_set_get_output(
    const pp_activity_set_t *activities, uint64_t activity_index,
    uint64_t output_index, pp_uuid_t *out_representation_id,
    const char **out_role, pp_error_t **out_error);
/* Edge snapshots are captured by storage at activity commit. A migrated edge
 * may report no snapshot. Returned fingerprint bytes borrow the result set. */
PP_API pp_error_code_t pp_activity_set_get_input_snapshot(
    const pp_activity_set_t *activities, uint64_t activity_index,
    uint64_t input_index, uint8_t *out_has_snapshot,
    uint64_t *out_revision_sequence, uint64_t *out_fingerprint_count,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_activity_set_get_output_snapshot(
    const pp_activity_set_t *activities, uint64_t activity_index,
    uint64_t output_index, uint8_t *out_has_snapshot,
    uint64_t *out_revision_sequence, uint64_t *out_fingerprint_count,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_activity_set_get_input_snapshot_fingerprint(
    const pp_activity_set_t *activities, uint64_t activity_index,
    uint64_t input_index, uint64_t fingerprint_index,
    const char **out_algorithm, uint16_t *out_version,
    const uint8_t **out_value, uint64_t *out_value_length,
    uint8_t *out_has_observed_revision,
    uint64_t *out_observed_revision_sequence, pp_error_t **out_error);
PP_API pp_error_code_t pp_activity_set_get_output_snapshot_fingerprint(
    const pp_activity_set_t *activities, uint64_t activity_index,
    uint64_t output_index, uint64_t fingerprint_index,
    const char **out_algorithm, uint16_t *out_version,
    const uint8_t **out_value, uint64_t *out_value_length,
    uint8_t *out_has_observed_revision,
    uint64_t *out_observed_revision_sequence, pp_error_t **out_error);
PP_API void pp_activity_set_release(pp_activity_set_t *activities);
/* Job views, their strings, and the optional next cursor borrow the owning
 * result set. State zero and a null kind select all jobs. */
PP_API pp_error_code_t pp_production_jobs(
    const pp_production_t *production, pp_job_state_t state, const char *kind,
    uint32_t limit, const char *cursor, pp_job_set_t **out_jobs,
    pp_error_t **out_error);
/* Reads one job as a one-element set. An absent job is PP_ERROR_NOT_FOUND. */
PP_API pp_error_code_t pp_production_job(
    const pp_production_t *production, const pp_uuid_t *job_id,
    pp_job_set_t **out_jobs, pp_error_t **out_error);
PP_API uint64_t pp_job_set_count(const pp_job_set_t *jobs);
PP_API const char *pp_job_set_next_cursor(const pp_job_set_t *jobs);
PP_API pp_error_code_t pp_job_set_get(
    const pp_job_set_t *jobs, uint64_t index, pp_job_t *out_job,
    pp_error_t **out_error);
/* Checked state-specific projections. Wrong state returns INVALID_ARGUMENT;
 * an absent index returns NOT_FOUND. Failure clears outputs to zero/NULL.
 * Strings borrow jobs until its release. Outputs are required writable. */
PP_API pp_error_code_t pp_job_set_get_claim(
    const pp_job_set_t *jobs, uint64_t index, pp_job_claim_t *out_claim,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_job_set_get_completion(
    const pp_job_set_t *jobs, uint64_t index,
    pp_job_completion_t *out_completion, pp_error_t **out_error);
PP_API pp_error_code_t pp_job_set_get_failure(
    const pp_job_set_t *jobs, uint64_t index, const char **out_diagnostic,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_job_set_get_input(
    const pp_job_set_t *jobs, uint64_t job_index, uint64_t input_index,
    pp_uuid_t *out_representation_id, pp_error_t **out_error);
PP_API void pp_job_set_release(pp_job_set_t *jobs);
/* Planning is read-only. Each get transfers a one-job set and its job-targeted
 * parameter metadata set; release both with their normal release functions. */
PP_API pp_error_code_t pp_production_plan_regeneration(
    const pp_production_t *production,
    const pp_uuid_t *artifact_representation_ids, uint64_t artifact_count,
    pp_regeneration_plan_set_t **out_plans, pp_error_t **out_error);
PP_API uint64_t pp_regeneration_plan_set_count(
    const pp_regeneration_plan_set_t *plans);
PP_API pp_error_code_t pp_regeneration_plan_set_get(
    const pp_regeneration_plan_set_t *plans, uint64_t index,
    pp_uuid_t *out_artifact_representation_id, pp_job_set_t **out_job,
    pp_metadata_set_t **out_parameters, pp_error_t **out_error);
PP_API void pp_regeneration_plan_set_release(
    pp_regeneration_plan_set_t *plans);
/* Revision strings are borrowed until pp_revision_set_release(). Latest
 * returns a set containing zero or one revision. */
PP_API pp_error_code_t pp_production_latest_revision(
    const pp_production_t *production, pp_revision_set_t **out_revisions,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_production_changes_since(
    const pp_production_t *production, uint64_t sequence, uint32_t limit,
    pp_revision_set_t **out_revisions, pp_error_t **out_error);
PP_API uint64_t pp_revision_set_count(const pp_revision_set_t *revisions);
PP_API pp_error_code_t pp_revision_set_get(
    const pp_revision_set_t *revisions, uint64_t index, pp_revision_id_t *out_id,
    uint64_t *out_sequence, pp_transaction_id_t *out_transaction_id,
    int64_t *out_committed_at_unix_micros, const char **out_origin_name,
    const char **out_origin_version, const char **out_origin_uri,
    const char **out_message, pp_error_t **out_error);
PP_API void pp_revision_set_release(pp_revision_set_t *revisions);
PP_API pp_error_code_t pp_production_revision_events(
    const pp_production_t *production, pp_revision_id_t revision_id,
    pp_revision_event_set_t **out_events, pp_error_t **out_error);
PP_API uint64_t
pp_revision_event_set_count(const pp_revision_event_set_t *events);
PP_API pp_error_code_t pp_revision_event_set_get(
    const pp_revision_event_set_t *events, uint64_t index,
    pp_revision_event_t *out_event, pp_error_t **out_error);
PP_API void pp_revision_event_set_release(pp_revision_event_set_t *events);
/* Returns revisions after sequence that contain at least one event of the
 * kinds array (1 to 64 PP_REVISION_* values; duplicates are ignored).
 * *out_through_sequence is the next cursor: every matching revision up to it
 * is in the set. A full page ends at its last revision; a short page ends at
 * the newest revision, or at sequence if that is newer. */
PP_API pp_error_code_t pp_production_changes_since_filtered(
    const pp_production_t *production, uint64_t sequence,
    const pp_revision_event_kind_t *kinds, uint64_t kind_count,
    uint32_t limit, pp_revision_set_t **out_revisions,
    uint64_t *out_through_sequence, pp_error_t **out_error);
/* A revision waiter owns its own read connection to the production file.
 * Commits through the same production wake it immediately; commits from other
 * processes or handles are detected by polling within about 100 ms. Releasing
 * the production closes every waiter created from it; release each waiter
 * separately with pp_revision_waiter_release(). */
PP_API pp_error_code_t pp_revision_waiter_create(
    const pp_production_t *production, pp_revision_waiter_t **out_waiter,
    pp_error_t **out_error);
/* Bounded position-ordered events; cursors retain production/revision scope. */
PP_API pp_error_code_t pp_production_revision_events_page(
    const pp_production_t *production, pp_revision_id_t revision_id,
    uint32_t limit, const char *cursor, pp_revision_event_set_t **out_events,
    pp_error_t **out_error);
/* Borrowed string, valid until events is released; null at end of page. */
PP_API const char *pp_revision_event_set_next_cursor(
    const pp_revision_event_set_t *events);
/* Blocks until a revision after after_sequence exists, then returns up to
 * limit (1 to 1,000) of them with PP_REVISION_WAIT_REVISIONS. Otherwise
 * returns PP_REVISION_WAIT_TIMED_OUT after timeout_millis (0 checks once;
 * at most PP_REVISION_WAIT_MAX_TIMEOUT_MILLIS), or PP_REVISION_WAIT_CLOSED or
 * PP_REVISION_WAIT_CANCELLED, which are terminal for the waiter. On success
 * *out_revisions is always a caller-owned set, empty unless revisions were
 * returned. A concurrent second wait on one waiter returns PP_ERROR_CONFLICT. */
PP_API pp_error_code_t pp_revision_waiter_wait(
    pp_revision_waiter_t *waiter, uint64_t after_sequence, uint32_t limit,
    uint32_t timeout_millis, pp_revision_wait_result_t *out_result,
    pp_revision_set_t **out_revisions, pp_error_t **out_error);
/* Ends the current wait and makes later waits return
 * PP_REVISION_WAIT_CANCELLED. Callable from any thread; null is a no-op. */
PP_API void pp_revision_waiter_cancel(pp_revision_waiter_t *waiter);
/* No thread may be waiting on the waiter during release. */
PP_API void pp_revision_waiter_release(pp_revision_waiter_t *waiter);
/* Computes the fingerprint import records for a regular file. Reads the file,
 * needs no production, and records nothing. *out_fingerprint is caller-owned. */
PP_API pp_error_code_t pp_fingerprint_file(const char *path,
                                           pp_fingerprint_t **out_fingerprint,
                                           pp_error_t **out_error);
/* The algorithm and value borrow the fingerprint until its release. */
PP_API pp_error_code_t pp_fingerprint_get(const pp_fingerprint_t *fingerprint,
                                          const char **out_algorithm,
                                          uint16_t *out_version,
                                          const uint8_t **out_value,
                                          uint64_t *out_value_length,
                                          pp_error_t **out_error);
PP_API void pp_fingerprint_release(pp_fingerprint_t *fingerprint);
/* Compares the content at path with the resource's stored fingerprints in the
 * domains PostProject computes. Read-only. PP_CONTENT_NOT_COMPARABLE means only
 * foreign fingerprints are stored, so nothing was compared. For an
 * image-sequence resource, path is a directory and sequence_naming names its
 * files; NULL means the naming recorded for that directory. It must be NULL
 * for any other resource. */
PP_API pp_error_code_t pp_production_verify_resource(
    const pp_production_t *production, const pp_uuid_t *resource_id,
    const char *path, const pp_sequence_naming_t *sequence_naming,
    pp_content_verification_t *out_verification, pp_error_t **out_error);
/* A cancellation token is a flag shared by the caller and running operations.
 * pp_cancel_token_cancel() may be called from any thread, including while an
 * operation observing the token runs on another; that operation then fails
 * with PP_ERROR_CANCELLED. Cancellation cannot be undone. */
PP_API pp_error_code_t pp_cancel_token_create(pp_cancel_token_t **out_token,
                                              pp_error_t **out_error);
PP_API void pp_cancel_token_cancel(const pp_cancel_token_t *token);
PP_API void pp_cancel_token_release(pp_cancel_token_t *token);
/* Resolution options are caller-owned and caller-serialized; one options
 * handle may serve many calls. Defaults: presence verification, depth 64,
 * 100000 entries per searched directory, no mappings or search directories,
 * and no cancellation token. Inputs are copied. */
PP_API pp_error_code_t pp_resolution_options_create(
    pp_resolution_options_t **out_options, pp_error_t **out_error);
PP_API void pp_resolution_options_release(pp_resolution_options_t *options);
/* Maps a logical media root to an existing directory on this machine. The
 * mapping is never recorded in the production. */
PP_API pp_error_code_t pp_resolution_options_add_root_mapping(
    pp_resolution_options_t *options, const char *name, const char *directory,
    pp_error_t **out_error);
/* Adds an unnamed, machine-local directory searched after the mapped roots.
 * Candidates found only there have no media root. */
PP_API pp_error_code_t pp_resolution_options_add_search_directory(
    pp_resolution_options_t *options, const char *directory,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_resolution_options_set_verification(
    pp_resolution_options_t *options, pp_verification_mode_t verification,
    pp_error_t **out_error);
/* A directory exceeding its entry budget is searched partially and reported
 * with PP_EVIDENCE_SEARCH_TRUNCATED; resolution continues. */
PP_API pp_error_code_t pp_resolution_options_set_limits(
    pp_resolution_options_t *options, uint32_t max_depth,
    uint64_t max_entries_per_directory, pp_error_t **out_error);
/* NULL removes the token. The options share the token's flag, so the token
 * handle may be released independently. */
PP_API pp_error_code_t pp_resolution_options_set_cancel_token(
    pp_resolution_options_t *options, const pp_cancel_token_t *token,
    pp_error_t **out_error);
/* Resolution is read-only. It resolves every representation of each asset,
 * in asset order, walking each searched directory at most once for the whole
 * call. NULL options mean the defaults. Borrowed candidate URI, media-root,
 * and evidence-detail strings remain valid until pp_resolution_set_release(). */
PP_API pp_error_code_t pp_production_resolve_assets(
    const pp_production_t *production, const pp_uuid_t *asset_ids,
    uint64_t asset_count, const pp_resolution_options_t *options,
    pp_resolution_set_t **out_resolutions, pp_error_t **out_error);
PP_API uint64_t pp_resolution_set_representation_count(
    const pp_resolution_set_t *resolutions);
PP_API pp_error_code_t pp_resolution_set_get_representation(
    const pp_resolution_set_t *resolutions, uint64_t representation_index,
    pp_uuid_t *out_asset_id, pp_uuid_t *out_representation_id,
    pp_representation_availability_t *out_availability,
    uint64_t *out_resource_count, uint64_t *out_issue_count,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_resolution_set_get_resource(
    const pp_resolution_set_t *resolutions, uint64_t representation_index,
    uint64_t resource_index, pp_uuid_t *out_resource_id,
    pp_resource_resolution_state_t *out_state,
    uint64_t *out_candidate_count, uint64_t *out_evidence_count,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_resolution_set_get_issue(
    const pp_resolution_set_t *resolutions, uint64_t representation_index,
    uint64_t issue_index, pp_uuid_t *out_resource_id, uint8_t *out_required,
    pp_availability_issue_kind_t *out_kind, uint64_t *out_frame_count,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_resolution_set_get_issue_frame(
    const pp_resolution_set_t *resolutions, uint64_t representation_index,
    uint64_t issue_index, uint64_t frame_index, int64_t *out_frame,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_resolution_set_get_candidate(
    const pp_resolution_set_t *resolutions, uint64_t representation_index,
    uint64_t resource_index, uint64_t candidate_index, const char **out_uri,
    uint16_t *out_confidence_basis_points, const char **out_media_root,
    uint8_t *out_has_sequence_naming, pp_sequence_naming_t *out_sequence_naming,
    uint64_t *out_evidence_count, pp_error_t **out_error);
PP_API pp_error_code_t pp_resolution_set_get_resource_evidence(
    const pp_resolution_set_t *resolutions, uint64_t representation_index,
    uint64_t resource_index, uint64_t evidence_index,
    pp_evidence_kind_t *out_kind, const char **out_detail,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_resolution_set_get_candidate_evidence(
    const pp_resolution_set_t *resolutions, uint64_t representation_index,
    uint64_t resource_index, uint64_t candidate_index,
    uint64_t evidence_index, pp_evidence_kind_t *out_kind,
    const char **out_detail, pp_error_t **out_error);
PP_API void pp_resolution_set_release(pp_resolution_set_t *resolutions);
/* Only one transaction may be open for a production state. The transaction keeps
 * that state alive independently of the production handle. Staging does not
 * block production reads; commit serializes with calls on the same production
 * state. Open the production again to avoid that per-handle serialization. */
PP_API pp_error_code_t pp_production_begin_transaction(
    pp_production_t *production, pp_transaction_t **out_transaction,
    pp_error_t **out_error);
/* The base revision must exist. It identifies the durable state from which the
 * caller made its decisions; it is not a lock or reservation. */
PP_API pp_error_code_t pp_production_begin_transaction_at(
    pp_production_t *production, pp_revision_id_t base_revision,
    pp_transaction_t **out_transaction, pp_error_t **out_error);
PP_API void pp_production_release(pp_production_t *production);

/* A media source describes the content structure of a representation at its
 * present location: a single file, an image sequence, ordered parts, or a
 * package. pp_transaction_import_media() creates an asset whose original
 * representation has the source's structure; pp_transaction_add_representation()
 * adds a representation of a chosen kind to an existing asset. Sources are
 * caller-owned and caller-serialized; a transaction borrows a source only for
 * the call, so one source may serve many calls. Inputs are copied and checked
 * against the content-structure rules when the source is created; the files
 * are inspected and fingerprinted when a transaction uses the source. */
PP_API pp_error_code_t pp_media_source_create_file(
    const char *path, pp_media_source_t **out_source, pp_error_t **out_error);
/* The missing-frame array is borrowed and may be NULL only when its count is
 * zero. The directory and the naming are required and borrowed. The directory
 * and naming become the sequence's first locator; the frames, rate, and
 * missing frames its descriptor. */
PP_API pp_error_code_t pp_media_source_create_image_sequence(
    const char *directory, const pp_sequence_naming_t *naming, int64_t start,
    int64_t end, uint32_t step,
    uint32_t rate_numerator, uint32_t rate_denominator,
    const int64_t *missing_frames, uint64_t missing_frame_count,
    pp_media_source_t **out_source, pp_error_t **out_error);
/* Member arrays and their strings are borrowed only for the call. Ordered
 * parts keep their order and must all be required; packages must contain a
 * required member. */
PP_API pp_error_code_t pp_media_source_create_ordered_parts(
    const pp_file_resource_input_t *members, uint64_t member_count,
    pp_media_source_t **out_source, pp_error_t **out_error);
PP_API pp_error_code_t pp_media_source_create_package(
    const pp_file_resource_input_t *members, uint64_t member_count,
    pp_media_source_t **out_source, pp_error_t **out_error);
PP_API void pp_media_source_release(pp_media_source_t *source);

/* Mutations remain in memory until commit. Transaction calls require caller-side
 * serialization. Input strings are borrowed UTF-8 without embedded NUL.
 * Nullable names/labels represent absent values. */
PP_API pp_error_code_t pp_transaction_set_revision_context(
    pp_transaction_t *transaction, const char *origin_name,
    const char *origin_version, const char *origin_uri, const char *message,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_import_media(
    pp_transaction_t *transaction, const pp_media_source_t *source,
    const char *display_name, pp_uuid_t *out_asset_id, pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_add_representation(
    pp_transaction_t *transaction, const pp_uuid_t *asset_id,
    pp_representation_kind_t kind, const pp_media_source_t *source,
    pp_uuid_t *out_representation_id, pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_add_media_root(
    pp_transaction_t *transaction, const char *name, const char *label,
    int32_t priority, pp_uuid_t *out_root_id, pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_set_media_root_enabled(
    pp_transaction_t *transaction, const pp_uuid_t *root_id, uint8_t enabled,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_remove_media_root(
    pp_transaction_t *transaction, const pp_uuid_t *root_id,
    pp_error_t **out_error);
/* Records uri as a locator of the resource. root_name may be NULL; otherwise it
 * names the logical media root the URI was found under. sequence_naming is
 * required for a resource that is an image sequence and must be NULL for any
 * other; commit rejects a mismatch. A resource may hold one directory under
 * several namings. */
PP_API pp_error_code_t pp_transaction_confirm_locator(
    pp_transaction_t *transaction, const pp_uuid_t *resource_id,
    const char *uri, const char *root_name,
    const pp_sequence_naming_t *sequence_naming, pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_retire_locator(
    pp_transaction_t *transaction, const pp_uuid_t *locator_id,
    pp_error_t **out_error);
/* Fingerprint values are borrowed only for the call and copied into the
 * transaction. Re-recording the identical current value is a successful no-op. */
PP_API pp_error_code_t pp_transaction_record_resource_fingerprint(
    pp_transaction_t *transaction, const pp_uuid_t *resource_id,
    const char *algorithm, uint16_t version, const uint8_t *value,
    uint64_t value_length, pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_record_representation_fingerprint(
    pp_transaction_t *transaction, const pp_uuid_t *representation_id,
    const char *algorithm, uint16_t version, const uint8_t *value,
    uint64_t value_length, pp_error_t **out_error);
/* Fingerprints the file or image-sequence directory at path as the present
 * content of the resource, and recomputes every representation using it. Both
 * are staged now, outside the production lock, and recorded on commit.
 * Observations staged earlier in the same transaction are taken into account.
 * out_outcome compares the content with the stored fingerprints in the domains
 * PostProject computes: PP_OBSERVATION_UNCHANGED records no fingerprint,
 * PP_OBSERVATION_CHANGED records the new content, and PP_OBSERVATION_FIRST means
 * no such fingerprint was stored before. sequence_naming is as for
 * pp_production_verify_resource(); a locator confirmed earlier in the same
 * transaction also records a naming for its directory. */
PP_API pp_error_code_t pp_transaction_observe_resource_content(
    pp_transaction_t *transaction, const pp_uuid_t *resource_id,
    const char *path, const pp_sequence_naming_t *sequence_naming,
    pp_content_observation_t *out_outcome, pp_error_t **out_error);
/* Replaces the complete ordered dependency observation. The array and strings
 * are borrowed for this call and copied into the transaction. */
PP_API pp_error_code_t pp_transaction_record_dependency_set(
    pp_transaction_t *transaction, const pp_uuid_t *representation_id,
    const pp_dependency_t *dependencies, uint64_t dependency_count,
    pp_error_t **out_error);
/* Scheme and value are required borrowed UTF-8 without embedded NUL. Qualifier
 * may be NULL. The complete mutation is validated and persisted at commit. */
PP_API pp_error_code_t pp_transaction_add_external_identifier(
    pp_transaction_t *transaction, const pp_object_ref_t *target,
    const char *scheme, const char *value, const char *qualifier,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_remove_external_identifier(
    pp_transaction_t *transaction, const pp_object_ref_t *target,
    const char *scheme, const char *value, const char *qualifier,
    pp_error_t **out_error);
/* Vocabulary and property identifiers are exact UTF-8 strings and are not
 * normalized. The input remains owned by the caller. */
PP_API pp_error_code_t pp_transaction_add_metadata_value(
    pp_transaction_t *transaction, const pp_object_ref_t *target,
    const char *vocabulary, const char *property,
    const pp_metadata_input_t *input, pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_remove_metadata_property(
    pp_transaction_t *transaction, const pp_object_ref_t *target,
    const char *vocabulary, const char *property, pp_error_t **out_error);
/* Stages one requested job. The input-ID array and strings are borrowed only
 * for this call and copied into the transaction. */
PP_API pp_error_code_t pp_transaction_request_job(
    pp_transaction_t *transaction, const char *kind,
    const pp_uuid_t *input_representation_ids, uint64_t input_count,
    const pp_uuid_t *output_asset_id,
    pp_representation_kind_t output_representation_kind,
    const char *target_root, pp_uuid_t *out_job_id,
    pp_error_t **out_error);
/* Claim returns a random token that becomes usable only after commit. Worker
 * identity strings are borrowed for the call. Lease times are caller-supplied. */
PP_API pp_error_code_t pp_transaction_claim_job(
    pp_transaction_t *transaction, const pp_uuid_t *job_id,
    const char *tool_name, const char *tool_version, const char *tool_uri,
    const char *agent_name, const char *agent_identifier_scheme,
    const char *agent_identifier_value,
    const char *agent_identifier_qualifier, int64_t now_unix_micros,
    int64_t expires_at_unix_micros, pp_uuid_t *out_claim_id,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_renew_job_claim(
    pp_transaction_t *transaction, const pp_uuid_t *job_id,
    const pp_uuid_t *claim_id, int64_t now_unix_micros,
    int64_t expires_at_unix_micros, pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_release_job_claim(
    pp_transaction_t *transaction, const pp_uuid_t *job_id,
    const pp_uuid_t *claim_id, pp_error_t **out_error);
/* Atomically completes a job with a representation and activity already
 * staged, in that order, in this transaction. */
PP_API pp_error_code_t pp_transaction_complete_job(
    pp_transaction_t *transaction, const pp_uuid_t *job_id,
    const pp_uuid_t *claim_id, int64_t now_unix_micros,
    const pp_uuid_t *output_representation_id,
    const pp_uuid_t *activity_id, pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_fail_job(
    pp_transaction_t *transaction, const pp_uuid_t *job_id,
    const pp_uuid_t *claim_id, int64_t now_unix_micros,
    const char *diagnostic, pp_error_t **out_error);
/* Cancellation is administrative and therefore does not require a claim token. */
PP_API pp_error_code_t pp_transaction_cancel_job(
    pp_transaction_t *transaction, const pp_uuid_t *job_id,
    pp_error_t **out_error);
/* Arrays and strings are borrowed only for this call. A NULL timestamp pointer
 * means absent. Tool and agent fields are independently optional subject to
 * the documented domain invariants. */
PP_API pp_error_code_t pp_transaction_create_activity(
    pp_transaction_t *transaction, const char *kind,
    const pp_activity_edge_t *inputs, uint64_t input_count,
    const pp_activity_edge_t *outputs, uint64_t output_count,
    const int64_t *started_at_unix_micros,
    const int64_t *finished_at_unix_micros, const char *tool_name,
    const char *tool_version, const char *tool_uri, const char *agent_name,
    const char *agent_identifier_scheme, const char *agent_identifier_value,
    const char *agent_identifier_qualifier, pp_uuid_t *out_activity_id,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_commit(pp_transaction_t *transaction,
                                            pp_error_t **out_error);

/* A commit attempt is terminal, including failure. Captures the produced
 * revision in the atomic write path; never queries a later head. out_receipt
 * is required. Release the closed transaction with pp_transaction_release. */
PP_API pp_error_code_t pp_transaction_commit_with_receipt(
    pp_transaction_t *transaction, pp_commit_receipt_t *out_receipt,
    pp_error_t **out_error);
PP_API pp_error_code_t pp_transaction_rollback(pp_transaction_t *transaction,
                                               pp_error_t **out_error);
/* Releasing an open transaction discards all staged work. Null is a no-op. */
PP_API void pp_transaction_release(pp_transaction_t *transaction);

PP_API pp_error_code_t pp_error_code(const pp_error_t *error);
/* The returned string is borrowed and valid until pp_error_release(error). */
PP_API const char *pp_error_message(const pp_error_t *error);
/* Returns one and writes borrowed structured detail for an optimistic
 * transaction conflict, or zero and a zeroed value for every other error.
 * The output's strings remain valid until pp_error_release(error). */
PP_API uint8_t pp_error_transaction_conflict(
    const pp_error_t *error, pp_transaction_conflict_t *out_conflict);
PP_API void pp_error_release(pp_error_t *error);

#ifdef __cplusplus
}
#endif

#endif
