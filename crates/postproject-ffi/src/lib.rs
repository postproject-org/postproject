//! Public C ABI for `PostProject`.
//!
//! All exported calls contain Rust panics and translate domain errors into stable
//! numeric codes plus owned error objects. Native consumers should include the
//! shipped `postproject.h` rather than depending on Rust declarations.

mod abi_trace;
mod artifact;
mod content;
mod dependency;
mod identities;
mod job_state;
#[cfg(test)]
mod job_state_tests;
mod jobs;
mod known_media;
mod media_source;
mod metadata;
mod metadata_input;
mod provenance;
mod read_queries;
mod read_session;
mod representations;
mod resolution;
mod revision_events;
mod revision_waits;
mod revisions;
mod sequence_naming;

use std::{
    any::Any,
    ffi::{CStr, CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    ptr,
    str::FromStr,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityOutputQuery,
    ActivityRole, AgentIdentity, ArtifactEvaluationLimits, Asset, AssetId, AvailabilityIssue,
    AvailabilityIssueKind, CommitReceipt, DecisionBase, Dependency, DependencyKind,
    DependencyQueryLimits, DependencyTarget, Error, ErrorKind, EvidenceKind, ExternalIdentifier,
    HostObjectBinding, IdentifierScheme, Job, JobClaimId, JobFailure, JobId, JobKind, JobQuery,
    JobStateKind, Locator, LocatorAvailability, LocatorId, MAX_ACTIVITY_EDGES,
    MAX_DEPENDENCIES_PER_SET, MAX_JOB_INPUTS, MediaRoot, MediaRootId, MetadataProperty,
    MetadataQuery, MetadataValue, ObjectRef, OriginIdentity, OriginalMediaImport, ProductionId,
    PropertyId, ProvenanceQueryLimits, ProvenanceQueryMatch, QueryCursor, QueryPageRequest,
    RepresentationAvailability, RepresentationFingerprint, RepresentationId, RepresentationImport,
    RepresentationKind, RepresentationResolution, RequestedJobOutput, ResolutionEvidence,
    ResourceFingerprint, ResourceId, ResourceResolutionState, RevisionContext, RevisionId,
    SemanticConflictKey, StaleArtifactQuery, Timestamp, ToolIdentity, TransactionConflict,
    TransactionLifecycle, VocabularyId,
};
use postproject_media::{
    canonical_file_uri, local_file_path, prepare_confirmed_locator, prepare_original_media,
    prepare_representation,
};
use postproject_storage_sqlite::SqliteProduction;

pub use artifact::{
    PpArtifactDependencyPathSegment, PpArtifactEvaluation, PpArtifactReason,
    PpArtifactReproducibility, PpArtifactReproducibilityIssue,
};
pub use content::PpFingerprint;
pub use dependency::{PpDependency, PpDependencyMatch, PpDependencyQuerySet, PpDependencySet};
pub use identities::{
    PpActivityId, PpAssetId, PpJobId, PpLocatorId, PpMediaRootId, PpProductionId,
    PpRepresentationId, PpResourceId, PpRevisionId, PpTransactionId,
};
pub use jobs::{PpJob, PpJobClaim, PpJobCompletion, PpJobSet, PpRegenerationPlanSet};
pub use known_media::PpKnownMediaSet;
pub use media_source::PpMediaSource;
use metadata::AbiMetadataValue;
pub use metadata::{PpMetadataSet, PpMetadataValue};
pub use metadata_input::PpMetadataInput;
pub use provenance::PpActivitySet;
use provenance::{AbiActivityEdge, AbiActivityEdgeSnapshot, AbiFingerprintSnapshot};
pub use read_session::{PpDecisionBase, PpReadSession};
pub use representations::PpRepresentationSet;
pub use resolution::{PpCancelToken, PpResolutionOptions};
pub use revision_events::PpRevisionEventSet;
pub use revision_waits::PpRevisionWaiter;
pub use revisions::PpRevisionSet;
pub use sequence_naming::PpSequenceNaming;
use sequence_naming::{
    AbiSequenceNaming, initialize_naming_output, optional_naming, write_naming_output,
};

const PP_OK: u32 = 0;
const PP_ERROR_INVALID_ARGUMENT: u32 = 1;
const PP_ERROR_NOT_FOUND: u32 = 2;
const PP_ERROR_ALREADY_EXISTS: u32 = 3;
const PP_ERROR_IO: u32 = 4;
const PP_ERROR_STORAGE: u32 = 5;
const PP_ERROR_MIGRATION: u32 = 6;
const PP_ERROR_CONFLICT: u32 = 7;
const PP_ERROR_AMBIGUOUS_RESOLUTION: u32 = 8;
const PP_ERROR_FINGERPRINT: u32 = 9;
const PP_ERROR_UNSUPPORTED: u32 = 10;
const PP_ERROR_CANCELLED: u32 = 11;
const PP_ERROR_INTERNAL: u32 = 255;

const PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR: u32 = 1;
const PP_RESOURCE_RESOLVED_EXACT: u32 = 2;
const PP_RESOURCE_RESOLVED_PROBABLE: u32 = 3;
const PP_RESOURCE_OFFLINE: u32 = 4;
const PP_RESOURCE_AMBIGUOUS: u32 = 5;
const PP_RESOURCE_RESOLUTION_ERROR: u32 = 6;

const PP_AVAILABILITY_ONLINE: u32 = 1;
const PP_AVAILABILITY_PARTIAL: u32 = 2;
const PP_AVAILABILITY_OFFLINE: u32 = 3;
const PP_AVAILABILITY_AMBIGUOUS: u32 = 4;
const PP_AVAILABILITY_ERROR: u32 = 5;

const PP_AVAILABILITY_ISSUE_OFFLINE_RESOURCE: u32 = 1;
const PP_AVAILABILITY_ISSUE_AMBIGUOUS_RESOURCE: u32 = 2;
const PP_AVAILABILITY_ISSUE_RESOURCE_ERROR: u32 = 3;
const PP_AVAILABILITY_ISSUE_MISSING_FRAMES: u32 = 4;

const PP_EVIDENCE_KNOWN_LOCATOR_AVAILABLE: u32 = 1;
const PP_EVIDENCE_EXACT_FINGERPRINT_MATCH: u32 = 2;
const PP_EVIDENCE_FULL_HASH_MATCH: u32 = 3;
const PP_EVIDENCE_PARTIAL_FINGERPRINT_MATCH: u32 = 4;
const PP_EVIDENCE_FILE_SIZE_MATCH: u32 = 5;
const PP_EVIDENCE_FILE_NAME_MATCH: u32 = 6;
const PP_EVIDENCE_RELATIVE_PATH_SIMILARITY: u32 = 7;
const PP_EVIDENCE_MEDIA_ROOT_RELATION: u32 = 8;
const PP_EVIDENCE_CONFLICTING_CANDIDATE: u32 = 9;
const PP_EVIDENCE_DISCOVERY_ERROR: u32 = 10;
const PP_EVIDENCE_MEDIA_ROOT_UNMAPPED: u32 = 11;
const PP_EVIDENCE_MEDIA_ROOT_UNAVAILABLE: u32 = 12;
const PP_EVIDENCE_FINGERPRINT_MISMATCH: u32 = 13;
const PP_EVIDENCE_FINGERPRINT_NOT_VERIFIED: u32 = 14;
const PP_EVIDENCE_SEARCH_TRUNCATED: u32 = 15;

const PP_OBJECT_PRODUCTION: u32 = 1;
const PP_OBJECT_ASSET: u32 = 2;
const PP_OBJECT_REPRESENTATION: u32 = 3;
const PP_OBJECT_RESOURCE: u32 = 4;
const PP_OBJECT_ACTIVITY: u32 = 5;
const PP_OBJECT_JOB: u32 = 6;

const PP_CONFLICT_LOCATOR_SET: u32 = 1;
const PP_CONFLICT_METADATA_PROPERTY: u32 = 2;
const PP_CONFLICT_DEPENDENCY_SET: u32 = 3;
const PP_CONFLICT_MEDIA_ROOT: u32 = 4;
const PP_CONFLICT_EXTERNAL_IDENTIFIER: u32 = 5;
const PP_CONFLICT_RESOURCE_FINGERPRINT: u32 = 6;
const PP_CONFLICT_REPRESENTATION_FINGERPRINT: u32 = 7;
const PP_CONFLICT_RESOURCE_FILE_FACTS: u32 = 8;

const PP_REPRESENTATION_ORIGINAL: u32 = 1;
const PP_REPRESENTATION_PROXY: u32 = 2;
const PP_REPRESENTATION_OPTIMIZED: u32 = 3;
const PP_REPRESENTATION_DERIVED: u32 = 4;

const PP_REVISION_ASSET_IMPORTED: u32 = 1;
const PP_REVISION_REPRESENTATION_ADDED: u32 = 2;
const PP_REVISION_RESOURCE_ADDED: u32 = 3;
const PP_REVISION_REPRESENTATION_RESOURCE_ADDED: u32 = 4;
const PP_REVISION_LOCATOR_ADDED: u32 = 5;
const PP_REVISION_MEDIA_ROOT_ADDED: u32 = 6;
const PP_REVISION_EXTERNAL_IDENTIFIER_ADDED: u32 = 7;
const PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED: u32 = 8;
const PP_REVISION_METADATA_ADDED_OR_REPLACED: u32 = 9;
const PP_REVISION_METADATA_REMOVED: u32 = 10;
const PP_REVISION_ACTIVITY_CREATED: u32 = 11;
const PP_REVISION_ACTIVITY_INPUT_ADDED: u32 = 12;
const PP_REVISION_ACTIVITY_OUTPUT_ADDED: u32 = 13;
const PP_REVISION_LOCATOR_RETIRED: u32 = 14;
const PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED: u32 = 15;
const PP_REVISION_MEDIA_ROOT_REMOVED: u32 = 16;
const PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED: u32 = 17;
const PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED: u32 = 18;
const PP_REVISION_RESOURCE_FILE_FACTS_OBSERVED: u32 = 27;
const PP_REVISION_DEPENDENCY_SET_RECORDED: u32 = 19;
const PP_REVISION_JOB_REQUESTED: u32 = 20;
const PP_REVISION_JOB_CLAIMED: u32 = 21;
const PP_REVISION_JOB_CLAIM_RENEWED: u32 = 22;
const PP_REVISION_JOB_CLAIM_RELEASED: u32 = 23;
const PP_REVISION_JOB_SUCCEEDED: u32 = 24;
const PP_REVISION_JOB_FAILED: u32 = 25;
const PP_REVISION_JOB_CANCELLED: u32 = 26;

/// Current pre-1.0 ABI version.
pub const ABI_VERSION: u32 = 47;

/// Fixed-layout UUID-compatible public identifier.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpUuid {
    /// UUID bytes in network order.
    pub bytes: [u8; 16],
}

/// Fixed-layout stack result of a successful atomic commit.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpCommitReceipt {
    /// Production whose commit succeeded.
    pub production_id: PpProductionId,
    /// Zero for no change, one when this commit created a revision.
    pub outcome: u32,
    /// Created revision, meaningful only when outcome is one.
    pub revision_id: PpRevisionId,
    /// Created revision's production-local sequence, otherwise zero.
    pub revision_sequence: u64,
}

impl From<&CommitReceipt> for PpCommitReceipt {
    fn from(receipt: &CommitReceipt) -> Self {
        Self {
            production_id: PpProductionId {
                bytes: receipt.production_id().into_bytes(),
            },
            outcome: u32::from(receipt.revision().is_some()),
            revision_id: PpRevisionId {
                bytes: receipt
                    .revision()
                    .map_or([0; 16], |revision| revision.id().into_bytes()),
            },
            revision_sequence: receipt
                .revision()
                .map_or(0, postproject_core::Revision::sequence),
        }
    }
}

/// Fixed-layout typed reference to a `PostProject` object.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpObjectRef {
    /// One of the `PP_OBJECT_*` constants from the public header.
    pub kind: u32,
    /// Stable ID whose interpretation is selected by `kind`.
    pub id: PpUuid,
}

/// Borrowed fixed-layout detail for one optimistic transaction conflict.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpTransactionConflict {
    /// One of the `PP_CONFLICT_*` constants from the public header.
    pub kind: u32,
    /// Affected object for non-root keys; zero for media-root conflicts.
    pub target: PpObjectRef,
    /// Root identity for media-root conflicts; zero for other keys.
    pub media_root_id: PpMediaRootId,
    /// Vocabulary, identifier scheme, or fingerprint algorithm when applicable.
    pub namespace_name: *const c_char,
    /// Metadata property or external-identifier value when applicable.
    pub local_name: *const c_char,
    /// External-identifier qualifier when present.
    pub qualifier: *const c_char,
    /// Fingerprint algorithm version, or zero when not applicable.
    pub version: u16,
    /// Whether a base revision exists (zero for an empty-journal decision).
    pub has_base_revision: u8,
    /// Base revision supplied by the caller when present.
    pub base_revision_id: PpRevisionId,
    /// Production-local base revision sequence.
    pub base_revision_sequence: u64,
    /// Revision that changed the key after the base.
    pub superseding_revision_id: PpRevisionId,
    /// Production-local superseding revision sequence.
    pub superseding_revision_sequence: u64,
}

/// Borrowed activity edge supplied by a C caller.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpActivityEdge {
    /// Representation consumed or produced by the activity.
    pub representation_id: PpRepresentationId,
    /// Optional NUL-terminated namespaced role.
    pub role: *const c_char,
}

/// Borrowed file and membership description supplied by a C caller.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpFileResourceInput {
    /// Required NUL-terminated filesystem path.
    pub path: *const c_char,
    /// Required NUL-terminated namespaced resource role.
    pub role: *const c_char,
    /// Exactly zero or one.
    pub required: u8,
}

/// Borrowed, fixed-layout semantic revision event.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpRevisionEvent {
    /// One of the `PP_REVISION_*` constants from the public header.
    pub kind: u32,
    /// Stable zero-based position within the owning revision.
    pub position: u32,
    /// Event asset identity, or zero when not applicable.
    pub asset_id: PpAssetId,
    /// Event representation identity, or zero when not applicable.
    pub representation_id: PpRepresentationId,
    /// Event resource identity, or zero when not applicable.
    pub resource_id: PpResourceId,
    /// Event locator identity, or zero when not applicable.
    pub locator_id: PpLocatorId,
    /// Event media-root identity, or zero when not applicable.
    pub media_root_id: PpMediaRootId,
    /// Event activity identity, or zero when not applicable.
    pub activity_id: PpActivityId,
    /// Event job identity, or zero when not applicable.
    pub job_id: PpJobId,
    /// Metadata/identifier target, with kind zero when not applicable.
    pub target: PpObjectRef,
    /// Structural member position for representation-resource events.
    pub structural_position: u32,
    /// New enabled state for media-root enablement events; zero otherwise.
    pub enabled: u8,
    /// Borrowed identifier scheme, or null when not applicable.
    pub identifier_scheme: *const c_char,
    /// Borrowed identifier value, or null when not applicable.
    pub identifier_value: *const c_char,
    /// Borrowed optional identifier qualifier.
    pub identifier_qualifier: *const c_char,
    /// Borrowed metadata vocabulary, or null when not applicable.
    pub vocabulary: *const c_char,
    /// Borrowed metadata property, or null when not applicable.
    pub property: *const c_char,
    /// Borrowed activity kind, or null when not applicable.
    pub activity_kind: *const c_char,
    /// Borrowed activity-edge role, or null when absent/not applicable.
    pub role: *const c_char,
    /// Borrowed fingerprint algorithm, or null when not applicable.
    pub fingerprint_algorithm: *const c_char,
    /// Fingerprint algorithm version, or zero when not applicable.
    pub fingerprint_version: u16,
}

/// Opaque production handle owned by the C caller.
pub struct PpProduction {
    state: Arc<ProductionState>,
}

impl Drop for PpProduction {
    fn drop(&mut self) {
        revision_waits::close_production_waiters(self);
    }
}

struct ProductionState {
    inner: Mutex<SqliteProduction>,
    transaction_open: AtomicBool,
}

/// Opaque transaction handle owned by the C caller.
pub struct PpTransaction {
    state: Arc<ProductionState>,
    lifecycle: TransactionLifecycle,
    revision_context: RevisionContext,
    base_revision: Option<RevisionId>,
    decision_base: Option<DecisionBase>,
    decision_reader: Option<Arc<ProductionState>>,
    mutations: Vec<StagedMutation>,
}

/// Opaque immutable asset result set owned by the C caller.
pub struct PpAssetSet {
    assets: Vec<AbiAsset>,
    next_cursor: Option<CString>,
}

/// Opaque immutable media-root result set owned by the C caller.
pub struct PpMediaRootSet {
    roots: Vec<AbiMediaRoot>,
    next_cursor: Option<CString>,
}

struct AbiMediaRoot {
    id: MediaRootId,
    name: CString,
    label: Option<CString>,
    legacy_uri: Option<CString>,
    priority: i32,
    enabled: bool,
}

impl TryFrom<&MediaRoot> for AbiMediaRoot {
    type Error = Error;

    fn try_from(root: &MediaRoot) -> Result<Self, Self::Error> {
        Ok(Self {
            id: root.id(),
            name: exact_cstring(root.name(), "media root name")?,
            label: root
                .label()
                .map(|value| exact_cstring(value, "media root label"))
                .transpose()?,
            legacy_uri: root
                .legacy_uri()
                .map(|value| exact_cstring(value, "legacy media root URI"))
                .transpose()?,
            priority: root.priority(),
            enabled: root.is_enabled(),
        })
    }
}

struct AbiAsset {
    id: AssetId,
    created_at_unix_micros: i64,
    display_name: Option<CString>,
    import_source: Option<CString>,
}

impl TryFrom<&Asset> for AbiAsset {
    type Error = Error;

    fn try_from(asset: &Asset) -> Result<Self, Self::Error> {
        Ok(Self {
            id: asset.id(),
            created_at_unix_micros: asset.created_at().as_unix_micros(),
            display_name: asset
                .display_name()
                .map(|value| exact_cstring(value, "asset display name"))
                .transpose()?,
            import_source: asset
                .import_source()
                .map(|value| exact_cstring(value, "asset import source"))
                .transpose()?,
        })
    }
}

enum StagedMutation {
    Import(OriginalMediaImport),
    Representation(RepresentationImport),
    MediaRoot(MediaRoot),
    SetMediaRootEnabled(MediaRootId, bool),
    RemoveMediaRoot(MediaRootId),
    Locator(Locator),
    RetireLocator(postproject_core::LocatorId),
    RecordResourceFingerprint(ResourceId, ResourceFingerprint),
    RecordResourceFileFacts(ResourceId, postproject_core::FileFacts),
    RecordRepresentationFingerprint(RepresentationId, RepresentationFingerprint),
    RecordDependencySet(RepresentationId, Vec<Dependency>),
    AddExternalIdentifier(ObjectRef, ExternalIdentifier),
    RemoveExternalIdentifier(ObjectRef, ExternalIdentifier),
    AddMetadataValue(ObjectRef, MetadataProperty, MetadataValue),
    RemoveMetadataProperty(ObjectRef, MetadataProperty),
    Activity(Activity),
    RequestJob(Job),
    ClaimJob {
        job_id: JobId,
        claim_id: JobClaimId,
        tool: ToolIdentity,
        agent: Option<AgentIdentity>,
        now: Timestamp,
        expires_at: Timestamp,
    },
    RenewJobClaim(JobId, JobClaimId, Timestamp, Timestamp),
    ReleaseJobClaim(JobId, JobClaimId),
    CompleteJob {
        job_id: JobId,
        claim_id: JobClaimId,
        now: Timestamp,
        output: RepresentationImport,
        activity: Box<Activity>,
    },
    FailJob(JobId, JobClaimId, Timestamp, JobFailure),
    CancelJob(JobId),
}

/// Opaque immutable external-identifier result set owned by the C caller.
pub struct PpExternalIdentifierSet {
    identifiers: Vec<AbiExternalIdentifier>,
}

struct AbiExternalIdentifier {
    scheme: CString,
    value: CString,
    qualifier: Option<CString>,
}

/// Opaque immutable object-reference result set owned by the C caller.
pub struct PpObjectRefSet {
    objects: Vec<PpObjectRef>,
}

/// Opaque paginated object-query result set owned by the C caller.
pub struct PpObjectQuerySet {
    objects: Vec<(PpObjectRef, u32)>,
    next_cursor: Option<CString>,
    traversal_truncated: bool,
}

/// Opaque paginated locator result set owned by the C caller.
pub struct PpLocatorQuerySet {
    locators: Vec<AbiQueryLocator>,
    next_cursor: Option<CString>,
}

struct AbiQueryLocator {
    id: LocatorId,
    resource_id: ResourceId,
    uri: CString,
    availability: u32,
    last_seen: Option<i64>,
    media_root: Option<CString>,
    sequence_naming: Option<AbiSequenceNaming>,
}

/// Opaque set of immutable media-resolution results owned by the C caller.
pub struct PpResolutionSet {
    representations: Vec<AbiRepresentationResolution>,
}

struct AbiRepresentationResolution {
    asset_id: AssetId,
    representation_id: RepresentationId,
    availability: RepresentationAvailability,
    resources: Vec<AbiResolution>,
    issues: Vec<AvailabilityIssue>,
}

struct AbiResolution {
    resource_id: ResourceId,
    state: u32,
    candidates: Vec<AbiCandidate>,
    evidence: Vec<AbiEvidence>,
}

struct AbiCandidate {
    uri: CString,
    confidence: u16,
    media_root: Option<CString>,
    sequence_naming: Option<AbiSequenceNaming>,
    evidence: Vec<AbiEvidence>,
}

struct AbiEvidence {
    kind: u32,
    detail: Option<CString>,
}

/// Opaque error object owned by the C caller.
pub struct PpError {
    code: u32,
    message: CString,
    transaction_conflict: Option<AbiTransactionConflict>,
}

struct AbiTransactionConflict {
    kind: u32,
    target: PpObjectRef,
    media_root_id: PpMediaRootId,
    namespace_name: Option<CString>,
    local_name: Option<CString>,
    qualifier: Option<CString>,
    version: u16,
    base_revision_id: PpRevisionId,
    base_revision_sequence: u64,
    superseding_revision_id: PpRevisionId,
    superseding_revision_sequence: u64,
}

impl AbiTransactionConflict {
    fn from_domain(conflict: &TransactionConflict) -> Option<Self> {
        let mut value = Self {
            kind: 0,
            target: empty_object_ref(),
            media_root_id: PpMediaRootId { bytes: [0; 16] },
            namespace_name: None,
            local_name: None,
            qualifier: None,
            version: 0,
            base_revision_id: PpRevisionId {
                bytes: conflict
                    .base_revision()
                    .map_or([0; 16], RevisionId::into_bytes),
            },
            base_revision_sequence: conflict.base_sequence(),
            superseding_revision_id: PpRevisionId {
                bytes: conflict.superseding_revision().into_bytes(),
            },
            superseding_revision_sequence: conflict.superseding_sequence(),
        };
        match conflict.key() {
            SemanticConflictKey::ResourceFileFacts(resource_id) => {
                value.kind = PP_CONFLICT_RESOURCE_FILE_FACTS;
                value.target = resource_target(*resource_id);
            }
            SemanticConflictKey::LocatorSet(resource_id) => {
                value.kind = PP_CONFLICT_LOCATOR_SET;
                value.target = resource_target(*resource_id);
            }
            SemanticConflictKey::MetadataProperty { target, property } => {
                value.kind = PP_CONFLICT_METADATA_PROPERTY;
                value.target = object_ref_to_abi(*target).ok()?;
                value.namespace_name = Some(lossy_cstring(property.vocabulary().as_str()));
                value.local_name = Some(lossy_cstring(property.property().as_str()));
            }
            SemanticConflictKey::DependencySet(representation_id) => {
                value.kind = PP_CONFLICT_DEPENDENCY_SET;
                value.target = representation_target(*representation_id);
            }
            SemanticConflictKey::MediaRoot(root_id) => {
                value.kind = PP_CONFLICT_MEDIA_ROOT;
                value.media_root_id.bytes = root_id.into_bytes();
            }
            SemanticConflictKey::ExternalIdentifier { target, identifier } => {
                value.kind = PP_CONFLICT_EXTERNAL_IDENTIFIER;
                value.target = object_ref_to_abi(*target).ok()?;
                value.namespace_name = Some(lossy_cstring(identifier.scheme().as_str()));
                value.local_name = Some(lossy_cstring(identifier.value()));
                value.qualifier = identifier.qualifier().map(lossy_cstring);
            }
            SemanticConflictKey::ResourceFingerprint {
                resource_id,
                algorithm,
                version,
            } => {
                value.kind = PP_CONFLICT_RESOURCE_FINGERPRINT;
                value.target = resource_target(*resource_id);
                value.namespace_name = Some(lossy_cstring(algorithm));
                value.version = *version;
            }
            SemanticConflictKey::RepresentationFingerprint {
                representation_id,
                algorithm,
                version,
            } => {
                value.kind = PP_CONFLICT_REPRESENTATION_FINGERPRINT;
                value.target = representation_target(*representation_id);
                value.namespace_name = Some(lossy_cstring(algorithm));
                value.version = *version;
            }
            _ => return None,
        }
        Some(value)
    }

    fn borrowed(&self) -> PpTransactionConflict {
        PpTransactionConflict {
            kind: self.kind,
            target: self.target,
            media_root_id: self.media_root_id,
            namespace_name: self
                .namespace_name
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
            local_name: self
                .local_name
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
            qualifier: self
                .qualifier
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
            version: self.version,
            has_base_revision: u8::from(self.base_revision_sequence != 0),
            base_revision_id: self.base_revision_id,
            base_revision_sequence: self.base_revision_sequence,
            superseding_revision_id: self.superseding_revision_id,
            superseding_revision_sequence: self.superseding_revision_sequence,
        }
    }
}

/// Returns the ABI version implemented by this shared library.
#[postproject_ffi_macros::ffi_export]
pub extern "C" fn pp_abi_version() -> u32 {
    ABI_VERSION
}

/// Formats a caller-owned portable host-object binding.
///
/// # Safety
///
/// `object` must be readable. `out_binding` must be
/// writable and receives a string that must be released exactly once with
/// [`pp_string_release`]. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_host_binding_format(
    production_id: PpProductionId,
    object: *const PpObjectRef,
    out_binding: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and inputs checked before dereference.
    unsafe {
        initialize_output(out_binding);
        ffi_call(out_error, || {
            let object = object
                .as_ref()
                .ok_or_else(|| invalid_argument("object must not be null"))?;
            require_output(out_binding, "out_binding")?;
            let binding = HostObjectBinding::new(
                ProductionId::from_bytes(production_id.bytes),
                object_ref_from_abi(*object)?,
            )?;
            let binding = exact_cstring(&binding.to_string(), "host binding")?;
            out_binding.write(binding.into_raw());
            Ok(())
        })
    }
}

/// Parses a portable host-object binding into caller-owned value outputs.
///
/// # Safety
///
/// `binding` must be NUL-terminated UTF-8 for this call. Both value outputs
/// must be writable. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_host_binding_parse(
    binding: *const c_char,
    out_production_id: *mut PpProductionId,
    out_object: *mut PpObjectRef,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and all pointers checked before use.
    unsafe {
        initialize_value(out_production_id, PpProductionId { bytes: [0; 16] });
        initialize_object_ref(out_object);
        ffi_call(out_error, || {
            require_output(out_production_id, "out_production_id")?;
            require_output(out_object, "out_object")?;
            let binding = HostObjectBinding::from_str(required_utf8(binding, "binding")?)?;
            out_production_id.write(uuid(binding.production_id()));
            out_object.write(object_ref_to_abi(binding.object())?);
            Ok(())
        })
    }
}

/// Returns the canonical `file:` locator URI import records for an existing
/// path.
///
/// # Safety
///
/// `path` must be NUL-terminated UTF-8. `out_uri` must be writable and receives
/// a string that must be released exactly once with [`pp_string_release`].
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_file_path_to_locator(
    path: *const c_char,
    out_uri: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and inputs checked before dereference.
    unsafe {
        initialize_output(out_uri);
        ffi_call(out_error, || {
            require_output(out_uri, "out_uri")?;
            let uri = canonical_file_uri(Path::new(required_utf8(path, "path")?))?;
            out_uri.write(exact_cstring(&uri, "locator URI")?.into_raw());
            Ok(())
        })
    }
}

/// Converts a local `file:` locator URI to a native path. The path need not
/// exist.
///
/// # Safety
///
/// `uri` must be NUL-terminated UTF-8. `out_path` must be writable and receives
/// a string that must be released exactly once with [`pp_string_release`].
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_locator_to_file_path(
    uri: *const c_char,
    out_path: *mut *mut c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and inputs checked before dereference.
    unsafe {
        initialize_output(out_path);
        ffi_call(out_error, || {
            require_output(out_path, "out_path")?;
            let path = local_file_path(required_utf8(uri, "uri")?)?;
            let path = path
                .to_str()
                .ok_or_else(|| invalid_argument("locator path cannot be represented as UTF-8"))?;
            out_path.write(exact_cstring(path, "locator path")?.into_raw());
            Ok(())
        })
    }
}

/// Releases a string returned by this library. Null is a no-op.
///
/// # Safety
///
/// `value` must be null or a live string returned through an owned `char **`
/// output of this library that has not already been released.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_string_release(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: The caller contract requires the exact pointer and ownership
        // originating from `CString::into_raw`.
        drop(unsafe { CString::from_raw(value) });
    }
}

/// Creates a new production file.
///
/// # Safety
///
/// `path` must point to a NUL-terminated byte string for the duration of the
/// call. `display_name` may be null or must satisfy the same rule. `out_production`
/// must be a writable pointer. `out_error` may be null or writable. Successful
/// handles must be released exactly once with [`pp_production_release`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_create(
    path: *const c_char,
    display_name: *const c_char,
    out_production: *mut *mut PpProduction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller contract for each pointer is documented above. Helpers
    // validate nullability before dereferencing and borrow inputs only this call.
    unsafe {
        initialize_output(out_production);
        ffi_call(out_error, || {
            if out_production.is_null() {
                return Err(invalid_argument("out_production must not be null"));
            }
            let path = required_utf8(path, "path")?;
            if path.is_empty() {
                return Err(invalid_argument("path must not be empty"));
            }
            let display_name = optional_utf8(display_name, "display_name")?.map(str::to_owned);
            let production = SqliteProduction::create(Path::new(path), display_name)?;
            out_production.write(Box::into_raw(Box::new(production_handle(production))));
            Ok(())
        })
    }
}

/// Opens an existing production file.
///
/// # Safety
///
/// `path` must point to a NUL-terminated byte string for the duration of the
/// call. `out_production` must be writable. `out_error` may be null or writable.
/// Successful handles must be released exactly once with [`pp_production_release`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_open(
    path: *const c_char,
    out_production: *mut *mut PpProduction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller contract for each pointer is documented above. Helpers
    // validate nullability before dereferencing and borrow inputs only this call.
    unsafe {
        initialize_output(out_production);
        ffi_call(out_error, || {
            if out_production.is_null() {
                return Err(invalid_argument("out_production must not be null"));
            }
            let path = required_utf8(path, "path")?;
            if path.is_empty() {
                return Err(invalid_argument("path must not be empty"));
            }
            let production = SqliteProduction::open(Path::new(path))?;
            out_production.write(Box::into_raw(Box::new(production_handle(production))));
            Ok(())
        })
    }
}

/// Copies the stable production identity into caller-owned storage.
///
/// # Safety
///
/// `production` must be a live handle returned by this library. `out_id` must be
/// writable. `out_error` may be null or writable. The production must not be used
/// concurrently by another thread during the call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_id(
    production: *const PpProduction,
    out_id: *mut PpProductionId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference; non-null pointer
    // validity and synchronization are guaranteed by the caller contract.
    unsafe {
        initialize_value(out_id, PpProductionId { bytes: [0; 16] });
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            if out_id.is_null() {
                return Err(invalid_argument("out_id must not be null"));
            }
            let inner = lock_production(&production.state);
            out_id.write(uuid(inner.production().id()));
            Ok(())
        })
    }
}

/// Reports whether a stable asset identity exists in a production.
///
/// # Safety
///
/// `production` must be a live handle returned by this library and
/// `out_exists` writable. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_asset_exists(
    production: *const PpProduction,
    asset_id: PpAssetId,
    out_exists: *mut u8,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference; non-null pointer
    // validity and synchronization are guaranteed by the caller contract.
    unsafe {
        if !out_exists.is_null() {
            out_exists.write(0);
        }
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            if out_exists.is_null() {
                return Err(invalid_argument("out_exists must not be null"));
            }
            let inner = lock_production(&production.state);
            let expected = AssetId::from_bytes(asset_id.bytes);
            let exists = match inner.asset(expected) {
                Ok(_) => true,
                Err(error) if error.kind() == ErrorKind::NotFound => false,
                Err(error) => return Err(error),
            };
            out_exists.write(u8::from(exists));
            Ok(())
        })
    }
}

/// Returns every asset in deterministic storage order.
///
/// # Safety
///
/// `production` must be live, `out_assets` writable, and `out_error` null or
/// writable. The returned set is caller-owned.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_assets(
    production: *const PpProduction,
    out_assets: *mut *mut PpAssetSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and pointers checked before use.
    unsafe {
        initialize_output(out_assets);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            if out_assets.is_null() {
                return Err(invalid_argument("out_assets must not be null"));
            }
            let assets = lock_production(&production.state).assets()?;
            let assets = assets
                .iter()
                .map(AbiAsset::try_from)
                .collect::<Result<Vec<_>, Error>>()?;
            out_assets.write(Box::into_raw(Box::new(PpAssetSet {
                assets,
                next_cursor: None,
            })));
            Ok(())
        })
    }
}

/// Reads one asset by identity as a one-element asset set.
///
/// # Safety
///
/// `production` must be live, `out_assets` must be writable, and
/// `out_error` may be null or writable. The returned set is caller-owned.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_asset(
    production: *const PpProduction,
    asset_id: PpAssetId,
    out_assets: *mut *mut PpAssetSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and pointers checked before use.
    unsafe {
        initialize_output(out_assets);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_assets, "out_assets")?;
            let asset =
                lock_production(&production.state).asset(AssetId::from_bytes(asset_id.bytes))?;
            out_assets.write(Box::into_raw(Box::new(PpAssetSet {
                assets: vec![AbiAsset::try_from(&asset)?],
                next_cursor: None,
            })));
            Ok(())
        })
    }
}

/// Queries one bounded page of assets in creation and identity order.
///
/// # Safety
///
/// `production` must be live; `cursor` must be null or NUL-terminated UTF-8;
/// `out_assets` must be writable; and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_assets_page(
    production: *const PpProduction,
    limit: u32,
    cursor: *const c_char,
    out_assets: *mut *mut PpAssetSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_assets);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_assets, "out_assets")?;
            let page_request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state).assets_page(&page_request)?;
            let assets = page
                .items()
                .iter()
                .map(AbiAsset::try_from)
                .collect::<Result<Vec<_>, Error>>()?;
            out_assets.write(Box::into_raw(Box::new(PpAssetSet {
                assets,
                next_cursor: query_cursor_to_cstring(page.next_cursor())?,
            })));
            Ok(())
        })
    }
}

/// Returns the borrowed next-page cursor, or null for the last page.
///
/// # Safety
///
/// `assets` must be null or a live asset set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_asset_set_next_cursor(assets: *const PpAssetSet) -> *const c_char {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        assets.as_ref().map_or(ptr::null(), |set| {
            set.next_cursor
                .as_ref()
                .map_or(ptr::null(), |cursor| cursor.as_ptr())
        })
    }))
    .unwrap_or(ptr::null())
}

/// Returns the number of assets in a result set. Null returns zero.
///
/// # Safety
///
/// A non-null pointer must be a live asset set returned by this library.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_asset_set_count(assets: *const PpAssetSet) -> u64 {
    // SAFETY: A non-null pointer is guaranteed live by the caller.
    unsafe {
        assets
            .as_ref()
            .and_then(|assets| length_as_u64(assets.assets.len()).ok())
            .unwrap_or(0)
    }
}

/// Copies one asset summary and borrows its optional strings from the set.
///
/// # Safety
///
/// The set and every output pointer must be live. String outputs remain valid
/// until the set is released; `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn pp_asset_set_get(
    assets: *const PpAssetSet,
    index: u64,
    out_id: *mut PpAssetId,
    out_created_at_unix_micros: *mut i64,
    out_display_name: *mut *const c_char,
    out_import_source: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and validated before writes.
    unsafe {
        initialize_value(out_id, PpAssetId { bytes: [0; 16] });
        initialize_value(out_created_at_unix_micros, 0);
        initialize_const_output(out_display_name);
        initialize_const_output(out_import_source);
        ffi_call(out_error, || {
            let assets = assets
                .as_ref()
                .ok_or_else(|| invalid_argument("assets must not be null"))?;
            let asset = item_at(&assets.assets, index, "asset")?;
            write_copy(
                out_id,
                PpAssetId {
                    bytes: asset.id.into_bytes(),
                },
                "out_id",
            )?;
            write_copy(
                out_created_at_unix_micros,
                asset.created_at_unix_micros,
                "out_created_at_unix_micros",
            )?;
            write_copy(
                out_display_name,
                asset
                    .display_name
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
                "out_display_name",
            )?;
            write_copy(
                out_import_source,
                asset
                    .import_source
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
                "out_import_source",
            )
        })
    }
}

/// Releases an asset result set. Null is accepted.
///
/// # Safety
///
/// A non-null pointer must be an owned asset set returned by this library and
/// must not be used after this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_asset_set_release(assets: *mut PpAssetSet) {
    if !assets.is_null() {
        // SAFETY: Non-null pointers must originate from `pp_production_assets`.
        unsafe { drop(Box::from_raw(assets)) };
    }
}

/// Returns current configured roots in resolver order, capped at 1000.
///
/// # Safety
///
/// `production` must be live, `out_roots` writable, and `out_error` null or
/// writable. The returned set is caller-owned.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_media_roots(
    production: *const PpProduction,
    out_roots: *mut *mut PpMediaRootSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and pointers checked before use.
    unsafe {
        initialize_output(out_roots);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_roots, "out_roots")?;
            let inner = lock_production(&production.state);
            let roots = inner
                .media_roots()?
                .iter()
                .map(AbiMediaRoot::try_from)
                .collect::<Result<Vec<_>, Error>>()?;
            out_roots.write(Box::into_raw(Box::new(PpMediaRootSet {
                roots,
                next_cursor: None,
            })));
            Ok(())
        })
    }
}

/// Returns one bounded page of current roots in priority/identity order.
///
/// # Safety
///
/// Production must be live, cursor null or borrowed UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_media_roots_page(
    production: *const PpProduction,
    limit: u32,
    cursor: *const c_char,
    out_roots: *mut *mut PpMediaRootSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are cleared before validating borrowed caller inputs.
    unsafe {
        initialize_output(out_roots);
        ffi_call(out_error, || {
            require_output(out_roots, "out_roots")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state).media_roots_page(&request)?;
            let roots = page
                .items()
                .iter()
                .map(AbiMediaRoot::try_from)
                .collect::<Result<Vec<_>, Error>>()?;
            out_roots.write(Box::into_raw(Box::new(PpMediaRootSet {
                roots,
                next_cursor: query_cursor_to_cstring(page.next_cursor())?,
            })));
            Ok(())
        })
    }
}

/// Borrows the root page's continuation, or returns null at its end.
///
/// # Safety
///
/// A non-null set must be live; the returned string borrows it.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_media_root_set_next_cursor(
    roots: *const PpMediaRootSet,
) -> *const c_char {
    // SAFETY: A non-null result set is live by the caller contract.
    unsafe {
        roots
            .as_ref()
            .and_then(|roots| roots.next_cursor.as_ref())
            .map_or(ptr::null(), |cursor| cursor.as_ptr())
    }
}

/// Returns the number of media roots in a result set. Null returns zero.
///
/// # Safety
///
/// A non-null pointer must be a live media-root set returned by this library.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_media_root_set_count(roots: *const PpMediaRootSet) -> u64 {
    // SAFETY: A non-null pointer is guaranteed live by the caller.
    unsafe {
        roots
            .as_ref()
            .and_then(|roots| length_as_u64(roots.roots.len()).ok())
            .unwrap_or(0)
    }
}

/// Copies one media-root summary and borrows its strings from the set.
///
/// # Safety
///
/// The set and every output pointer must be live. String outputs remain valid
/// until the set is released; `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn pp_media_root_set_get(
    roots: *const PpMediaRootSet,
    index: u64,
    out_id: *mut PpMediaRootId,
    out_name: *mut *const c_char,
    out_label: *mut *const c_char,
    out_legacy_uri: *mut *const c_char,
    out_priority: *mut i32,
    out_enabled: *mut u8,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and validated before writes.
    unsafe {
        initialize_value(out_id, PpMediaRootId { bytes: [0; 16] });
        initialize_const_output(out_name);
        initialize_const_output(out_label);
        initialize_const_output(out_legacy_uri);
        initialize_value(out_priority, 0);
        initialize_value(out_enabled, 0);
        ffi_call(out_error, || {
            let roots = roots
                .as_ref()
                .ok_or_else(|| invalid_argument("roots must not be null"))?;
            let root = item_at(&roots.roots, index, "media root")?;
            write_copy(
                out_id,
                PpMediaRootId {
                    bytes: root.id.into_bytes(),
                },
                "out_id",
            )?;
            write_copy(out_name, root.name.as_ptr(), "out_name")?;
            write_copy(
                out_label,
                root.label
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
                "out_label",
            )?;
            write_copy(
                out_legacy_uri,
                root.legacy_uri
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
                "out_legacy_uri",
            )?;
            write_copy(out_priority, root.priority, "out_priority")?;
            write_copy(out_enabled, u8::from(root.enabled), "out_enabled")
        })
    }
}

/// Releases a media-root result set. Null is accepted.
///
/// # Safety
///
/// A non-null pointer must be an owned media-root set returned by this library.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_media_root_set_release(roots: *mut PpMediaRootSet) {
    if !roots.is_null() {
        // SAFETY: Non-null pointers must originate from `pp_production_media_roots`.
        unsafe { drop(Box::from_raw(roots)) };
    }
}

/// Loads external identifiers attached to one typed object reference.
///
/// Strings returned by result accessors are borrowed until the result set is
/// released.
///
/// # Safety
///
/// `production` and `target` must be readable live values. `out_identifiers` must
/// be writable. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_external_identifiers(
    production: *const PpProduction,
    target: *const PpObjectRef,
    out_identifiers: *mut *mut PpExternalIdentifierSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointers are validated before use and outputs are initialized.
    unsafe {
        initialize_output(out_identifiers);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let target = target
                .as_ref()
                .ok_or_else(|| invalid_argument("target must not be null"))?;
            if out_identifiers.is_null() {
                return Err(invalid_argument("out_identifiers must not be null"));
            }
            let target = object_ref_from_abi(*target)?;
            let inner = lock_production(&production.state);
            let identifiers = inner
                .external_identifiers(target)?
                .into_iter()
                .map(AbiExternalIdentifier::try_from)
                .collect::<Result<Vec<_>, _>>()?;
            out_identifiers.write(Box::into_raw(Box::new(PpExternalIdentifierSet {
                identifiers,
            })));
            Ok(())
        })
    }
}

/// Finds objects carrying an exact external identifier scheme and value,
/// restricted to an exact qualifier unless `qualifier` is null.
///
/// # Safety
///
/// `production` must be live; `scheme` and `value` must be borrowed NUL-terminated
/// UTF-8 strings; `qualifier` must be null or such a string; `out_objects` must be
/// writable; and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_find_by_external_identifier(
    production: *const PpProduction,
    scheme: *const c_char,
    value: *const c_char,
    qualifier: *const c_char,
    out_objects: *mut *mut PpObjectRefSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointers are validated before use and outputs are initialized.
    unsafe {
        initialize_output(out_objects);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            if out_objects.is_null() {
                return Err(invalid_argument("out_objects must not be null"));
            }
            let scheme = IdentifierScheme::new(required_utf8(scheme, "scheme")?)?;
            let value = required_utf8(value, "value")?;
            let qualifier = optional_utf8(qualifier, "qualifier")?;
            let inner = lock_production(&production.state);
            let objects = inner
                .find_by_external_identifier(&scheme, value, qualifier)?
                .into_iter()
                .map(object_ref_to_abi)
                .collect::<Result<Vec<_>, _>>()?;
            out_objects.write(Box::into_raw(Box::new(PpObjectRefSet { objects })));
            Ok(())
        })
    }
}

/// Returns the number of values in an external-identifier result set.
/// Null input returns zero.
///
/// # Safety
///
/// `identifiers` must be null or a live handle returned by this library.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_external_identifier_set_count(
    identifiers: *const PpExternalIdentifierSet,
) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { identifiers.as_ref() }.map_or(0, |set| {
            u64::try_from(set.identifiers.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Reads one external identifier. Returned strings are borrowed from the set.
///
/// # Safety
///
/// `identifiers` must be live; outputs must be writable; and `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_external_identifier_set_get(
    identifiers: *const PpExternalIdentifierSet,
    index: u64,
    out_scheme: *mut *const c_char,
    out_value: *mut *const c_char,
    out_qualifier: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_const_output(out_scheme);
        initialize_const_output(out_value);
        initialize_const_output(out_qualifier);
        ffi_call(out_error, || {
            require_output(out_scheme, "out_scheme")?;
            require_output(out_value, "out_value")?;
            require_output(out_qualifier, "out_qualifier")?;
            let identifiers = identifiers
                .as_ref()
                .ok_or_else(|| invalid_argument("identifiers must not be null"))?;
            let identifier = item_at(&identifiers.identifiers, index, "identifier")?;
            out_scheme.write(identifier.scheme.as_ptr());
            out_value.write(identifier.value.as_ptr());
            out_qualifier.write(
                identifier
                    .qualifier
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
            );
            Ok(())
        })
    }
}

/// Releases an external-identifier result set. Null is a no-op.
///
/// # Safety
///
/// A non-null handle must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_external_identifier_set_release(
    identifiers: *mut PpExternalIdentifierSet,
) {
    if identifiers.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(identifiers) });
    }));
}

/// Returns the number of values in an object-reference result set.
/// Null input returns zero.
///
/// # Safety
///
/// `objects` must be null or a live handle returned by this library.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_set_count(objects: *const PpObjectRefSet) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { objects.as_ref() }.map_or(0, |set| {
            u64::try_from(set.objects.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Reads one typed object reference.
///
/// # Safety
///
/// `objects` must be live; `out_object` must be writable; and `out_error` may
/// be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_set_get(
    objects: *const PpObjectRefSet,
    index: u64,
    out_object: *mut PpObjectRef,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_object_ref(out_object);
        ffi_call(out_error, || {
            require_output(out_object, "out_object")?;
            let objects = objects
                .as_ref()
                .ok_or_else(|| invalid_argument("objects must not be null"))?;
            out_object.write(*item_at(&objects.objects, index, "object")?);
            Ok(())
        })
    }
}

/// Releases an object-reference result set. Null is a no-op.
///
/// # Safety
///
/// A non-null handle must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_ref_set_release(objects: *mut PpObjectRefSet) {
    if objects.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(objects) });
    }));
}

/// Returns the number of values in a paginated object-query result.
///
/// # Safety
///
/// `objects` must be null or a live object-query set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_query_set_count(objects: *const PpObjectQuerySet) -> u64 {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        objects.as_ref().map_or(0, |set| {
            u64::try_from(set.objects.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Reads one typed object and its shortest traversal depth.
///
/// # Safety
///
/// `objects` must be live, outputs writable, and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_query_set_get(
    objects: *const PpObjectQuerySet,
    index: u64,
    out_object: *mut PpObjectRef,
    out_depth: *mut u32,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_object_ref(out_object);
        initialize_value(out_depth, 0);
        ffi_call(out_error, || {
            require_output(out_object, "out_object")?;
            require_output(out_depth, "out_depth")?;
            let objects = objects
                .as_ref()
                .ok_or_else(|| invalid_argument("objects must not be null"))?;
            let (object, depth) = item_at(&objects.objects, index, "query object")?;
            out_object.write(*object);
            out_depth.write(*depth);
            Ok(())
        })
    }
}

/// Returns the borrowed continuation cursor, or null for the last page.
///
/// # Safety
///
/// `objects` must be null or a live object-query set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_query_set_next_cursor(
    objects: *const PpObjectQuerySet,
) -> *const c_char {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        objects.as_ref().map_or(ptr::null(), |set| {
            set.next_cursor
                .as_ref()
                .map_or(ptr::null(), |cursor| cursor.as_ptr())
        })
    }))
    .unwrap_or(ptr::null())
}

/// Returns one when an explicit traversal bound truncated the result.
///
/// # Safety
///
/// `objects` must be null or a live object-query set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_query_set_traversal_truncated(
    objects: *const PpObjectQuerySet,
) -> u8 {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        objects
            .as_ref()
            .map_or(0, |set| u8::from(set.traversal_truncated))
    }))
    .unwrap_or(0)
}

/// Releases a paginated object-query result. Null is accepted.
///
/// # Safety
///
/// A non-null pointer must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_object_query_set_release(objects: *mut PpObjectQuerySet) {
    if !objects.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
            drop(Box::from_raw(objects));
        }));
    }
}

/// Returns the number of locators in a query page.
///
/// # Safety
///
/// `locators` must be null or a live locator-query set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_locator_query_set_count(locators: *const PpLocatorQuerySet) -> u64 {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        locators.as_ref().map_or(0, |set| {
            u64::try_from(set.locators.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Reads one locator query result. Strings borrow the result set.
///
/// # Safety
///
/// `locators` must be live, outputs writable, and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn pp_locator_query_set_get(
    locators: *const PpLocatorQuerySet,
    index: u64,
    out_id: *mut PpLocatorId,
    out_resource_id: *mut PpResourceId,
    out_uri: *mut *const c_char,
    out_availability: *mut u32,
    out_has_last_seen: *mut u8,
    out_last_seen_unix_micros: *mut i64,
    out_media_root: *mut *const c_char,
    out_has_sequence_naming: *mut u8,
    out_sequence_naming: *mut PpSequenceNaming,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_value(out_id, PpLocatorId { bytes: [0; 16] });
        initialize_value(out_resource_id, PpResourceId { bytes: [0; 16] });
        initialize_const_output(out_uri);
        initialize_value(out_availability, 0);
        initialize_value(out_has_last_seen, 0);
        initialize_value(out_last_seen_unix_micros, 0);
        initialize_const_output(out_media_root);
        initialize_naming_output(out_has_sequence_naming, out_sequence_naming);
        ffi_call(out_error, || {
            require_output(out_has_sequence_naming, "out_has_sequence_naming")?;
            require_output(out_sequence_naming, "out_sequence_naming")?;
            require_output(out_id, "out_id")?;
            require_output(out_resource_id, "out_resource_id")?;
            require_output(out_uri, "out_uri")?;
            require_output(out_availability, "out_availability")?;
            require_output(out_has_last_seen, "out_has_last_seen")?;
            require_output(out_last_seen_unix_micros, "out_last_seen_unix_micros")?;
            require_output(out_media_root, "out_media_root")?;
            let locators = locators
                .as_ref()
                .ok_or_else(|| invalid_argument("locators must not be null"))?;
            let locator = item_at(&locators.locators, index, "locator")?;
            out_id.write(PpLocatorId {
                bytes: locator.id.into_bytes(),
            });
            out_resource_id.write(PpResourceId {
                bytes: locator.resource_id.into_bytes(),
            });
            out_uri.write(locator.uri.as_ptr());
            out_availability.write(locator.availability);
            if let Some(last_seen) = locator.last_seen {
                out_has_last_seen.write(1);
                out_last_seen_unix_micros.write(last_seen);
            }
            out_media_root.write(
                locator
                    .media_root
                    .as_ref()
                    .map_or(ptr::null(), |root| root.as_ptr()),
            );
            write_naming_output(
                locator.sequence_naming.as_ref(),
                out_has_sequence_naming,
                out_sequence_naming,
            );
            Ok(())
        })
    }
}

/// Returns the borrowed locator-page continuation cursor.
///
/// # Safety
///
/// `locators` must be null or a live locator-query set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_locator_query_set_next_cursor(
    locators: *const PpLocatorQuerySet,
) -> *const c_char {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        locators.as_ref().map_or(ptr::null(), |set| {
            set.next_cursor
                .as_ref()
                .map_or(ptr::null(), |cursor| cursor.as_ptr())
        })
    }))
    .unwrap_or(ptr::null())
}

/// Releases a locator query page. Null is accepted.
///
/// # Safety
///
/// A non-null pointer must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_locator_query_set_release(locators: *mut PpLocatorQuerySet) {
    if !locators.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
            drop(Box::from_raw(locators));
        }));
    }
}

/// Queries one bounded page of resources in representation structure order.
///
/// # Safety
///
/// `production` and `representation_id` must be live, `cursor` null or UTF-8,
/// `out_objects` writable, and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_resources_page(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_objects);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_objects, "out_objects")?;
            let request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state).resources_page(
                RepresentationId::from_bytes(representation_id.bytes),
                &request,
            )?;
            out_objects.write(Box::into_raw(Box::new(object_query_set(
                page.items()
                    .iter()
                    .map(|resource| (ObjectRef::Resource(resource.id()), 0)),
                page.next_cursor(),
                false,
            )?)));
            Ok(())
        })
    }
}

/// Queries one bounded page of locators belonging to a resource.
///
/// # Safety
///
/// Pointer rules match [`pp_production_resources_page`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_locators_page(
    production: *const PpProduction,
    resource_id: PpResourceId,
    limit: u32,
    cursor: *const c_char,
    out_locators: *mut *mut PpLocatorQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_locators);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_locators, "out_locators")?;
            let request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state)
                .locators_page(ResourceId::from_bytes(resource_id.bytes), &request)?;
            out_locators.write(Box::into_raw(Box::new(locator_query_set(
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

/// Queries representations with required resources lacking locator knowledge.
///
/// # Safety
///
/// `production` must be live, `cursor` null or UTF-8, `out_objects` writable,
/// and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_unresolved_media(
    production: *const PpProduction,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_objects);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_objects, "out_objects")?;
            let request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state).unresolved_media(&request)?;
            out_objects.write(Box::into_raw(Box::new(object_query_set(
                page.items()
                    .iter()
                    .map(|id| (ObjectRef::Representation(*id), 0)),
                page.next_cursor(),
                false,
            )?)));
            Ok(())
        })
    }
}

/// Queries output representations produced by an exact activity kind.
///
/// # Safety
///
/// `kind` is required UTF-8; other pointer rules match
/// [`pp_production_unresolved_media`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_outputs_by_activity_kind(
    production: *const PpProduction,
    kind: *const c_char,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_objects);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_objects, "out_objects")?;
            let query = ActivityOutputQuery::Kind(ActivityKind::new(required_utf8(kind, "kind")?)?);
            let request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state).activity_outputs(&query, &request)?;
            out_objects.write(Box::into_raw(Box::new(object_query_set(
                page.items()
                    .iter()
                    .map(|id| (ObjectRef::Representation(*id), 0)),
                page.next_cursor(),
                false,
            )?)));
            Ok(())
        })
    }
}

/// Queries output representations produced by one exact tool identity.
///
/// # Safety
///
/// `name` is required UTF-8; `version`, `uri`, and `cursor` may be null or
/// UTF-8; outputs follow [`pp_production_unresolved_media`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_outputs_by_tool(
    production: *const PpProduction,
    name: *const c_char,
    version: *const c_char,
    uri: *const c_char,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_objects);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_objects, "out_objects")?;
            let query = ActivityOutputQuery::Tool(ToolIdentity::new(
                required_utf8(name, "name")?,
                optional_utf8(version, "version")?.map(str::to_owned),
                optional_utf8(uri, "uri")?.map(str::to_owned),
            )?);
            let request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state).activity_outputs(&query, &request)?;
            out_objects.write(Box::into_raw(Box::new(object_query_set(
                page.items()
                    .iter()
                    .map(|id| (ObjectRef::Representation(*id), 0)),
                page.next_cursor(),
                false,
            )?)));
            Ok(())
        })
    }
}

/// Queries one bounded page of activities producing a representation.
///
/// # Safety
///
/// Pointer rules match [`pp_production_resources_page`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_activities_producing_page(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    limit: u32,
    cursor: *const c_char,
    out_activities: *mut *mut PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        production_activities_page(
            production,
            representation_id,
            limit,
            cursor,
            true,
            out_activities,
            out_error,
        )
    }
}

/// Queries one bounded page of activities consuming a representation.
///
/// # Safety
///
/// Pointer rules match [`pp_production_activities_producing_page`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_activities_consuming_page(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    limit: u32,
    cursor: *const c_char,
    out_activities: *mut *mut PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        production_activities_page(
            production,
            representation_id,
            limit,
            cursor,
            false,
            out_activities,
            out_error,
        )
    }
}

/// Queries bounded shortest-depth provenance ancestors.
///
/// # Safety
///
/// Pointer rules match [`pp_production_resources_page`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_provenance_ancestors_page(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        production_provenance_page(
            production,
            representation_id,
            max_depth,
            max_representations,
            limit,
            cursor,
            true,
            out_objects,
            out_error,
        )
    }
}

/// Queries bounded shortest-depth provenance descendants.
///
/// # Safety
///
/// Pointer rules match [`pp_production_provenance_ancestors_page`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_provenance_descendants_page(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        production_provenance_page(
            production,
            representation_id,
            max_depth,
            max_representations,
            limit,
            cursor,
            false,
            out_objects,
            out_error,
        )
    }
}

/// Queries produced representations currently evaluated as stale.
///
/// A null `source_representation_id` selects all produced representations.
///
/// # Safety
///
/// `production` must be live; optional pointers must be null or readable;
/// result pointers must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_stale_artifacts(
    production: *const PpProduction,
    source_representation_id: *const PpRepresentationId,
    evaluation_max_depth: u32,
    evaluation_max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_objects);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_objects, "out_objects")?;
            let source = source_representation_id
                .as_ref()
                .map(|id| RepresentationId::from_bytes(id.bytes));
            let query = StaleArtifactQuery::new(
                source,
                ArtifactEvaluationLimits::new(
                    evaluation_max_depth,
                    evaluation_max_representations,
                )?,
            );
            let request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state).stale_artifacts(query, &request)?;
            out_objects.write(Box::into_raw(Box::new(object_query_set(
                page.items()
                    .iter()
                    .map(|id| (ObjectRef::Representation(*id), 0)),
                page.next_cursor(),
                page.traversal_truncated(),
            )?)));
            Ok(())
        })
    }
}

/// Queries distinct semantic objects touched after a revision sequence.
///
/// # Safety
///
/// Pointer rules match [`pp_production_unresolved_media`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_objects_changed_since(
    production: *const PpProduction,
    sequence: u64,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_objects);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_objects, "out_objects")?;
            let request = query_page_request(limit, cursor)?;
            let page =
                lock_production(&production.state).objects_changed_since(sequence, &request)?;
            out_objects.write(Box::into_raw(Box::new(object_query_set(
                page.items().iter().map(|object| (*object, 0)),
                page.next_cursor(),
                false,
            )?)));
            Ok(())
        })
    }
}

/// Loads typed metadata assertions attached to one object.
///
/// Returned strings and value pointers are borrowed until the result set is
/// released.
///
/// # Safety
///
/// `production` and `target` must be readable live values. `out_metadata` must be
/// writable and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_metadata(
    production: *const PpProduction,
    target: *const PpObjectRef,
    out_metadata: *mut *mut PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and output ownership is explicit.
    unsafe {
        initialize_output(out_metadata);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let target = target
                .as_ref()
                .ok_or_else(|| invalid_argument("target must not be null"))?;
            require_output(out_metadata, "out_metadata")?;
            let target = object_ref_from_abi(*target)?;
            let inner = lock_production(&production.state);
            let metadata = PpMetadataSet::from_assertions(target, &inner.metadata(target)?)?;
            out_metadata.write(Box::into_raw(Box::new(metadata)));
            Ok(())
        })
    }
}

/// Finds every assertion using one exact vocabulary and property.
///
/// # Safety
///
/// `production` must be live, strings must be borrowed NUL-terminated UTF-8,
/// `out_metadata` must be writable, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_find_metadata(
    production: *const PpProduction,
    vocabulary: *const c_char,
    property: *const c_char,
    out_metadata: *mut *mut PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and output ownership is explicit.
    unsafe {
        initialize_output(out_metadata);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_metadata, "out_metadata")?;
            let property = metadata_property_from_abi(vocabulary, property)?;
            let inner = lock_production(&production.state);
            let metadata =
                PpMetadataSet::from_matches(&inner.query_by_metadata_property(&property)?)?;
            out_metadata.write(Box::into_raw(Box::new(metadata)));
            Ok(())
        })
    }
}

/// Queries one bounded page of assertions by property and optional exact value.
///
/// A null `exact_value` selects every value for the property. A non-null value
/// must be scalar; list and struct predicates are rejected.
///
/// # Safety
///
/// `production` must be live; strings must be NUL-terminated UTF-8;
/// `exact_value` must be null or a live metadata input; `cursor` must be null or
/// UTF-8; `out_metadata` must be writable; and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(
    clippy::too_many_arguments,
    reason = "the C ABI exposes filter and pagination inputs explicitly"
)]
pub unsafe extern "C" fn pp_production_query_metadata(
    production: *const PpProduction,
    vocabulary: *const c_char,
    property: *const c_char,
    exact_value: *const PpMetadataInput,
    limit: u32,
    cursor: *const c_char,
    out_metadata: *mut *mut PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are copied after validation and output ownership is explicit.
    unsafe {
        initialize_output(out_metadata);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_metadata, "out_metadata")?;
            let property = metadata_property_from_abi(vocabulary, property)?;
            let exact_value = exact_value.as_ref().map(|input| input.value.clone());
            let query = MetadataQuery::new(property, exact_value)?;
            let request = query_page_request(limit, cursor)?;
            let page = lock_production(&production.state).metadata_query(&query, &request)?;
            out_metadata.write(Box::into_raw(Box::new(PpMetadataSet::from_page(
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

/// Returns the number of assertions in a metadata result set. Null returns zero.
///
/// # Safety
///
/// `metadata` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_set_count(metadata: *const PpMetadataSet) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { metadata.as_ref() }.map_or(0, |set| {
            u64::try_from(set.assertions.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Returns the borrowed metadata-page continuation cursor, or null.
///
/// # Safety
///
/// `metadata` must be null or a live metadata set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_set_next_cursor(
    metadata: *const PpMetadataSet,
) -> *const c_char {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        metadata.as_ref().map_or(ptr::null(), |set| {
            set.next_cursor
                .as_ref()
                .map_or(ptr::null(), |cursor| cursor.as_ptr())
        })
    }))
    .unwrap_or(ptr::null())
}

/// Reads one assertion and a borrowed pointer to its recursive value.
///
/// # Safety
///
/// `metadata` must be live. Every output must be writable and `out_error` may
/// be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_set_get(
    metadata: *const PpMetadataSet,
    index: u64,
    out_target: *mut PpObjectRef,
    out_vocabulary: *mut *const c_char,
    out_property: *mut *const c_char,
    out_value: *mut *const PpMetadataValue,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_object_ref(out_target);
        initialize_const_output(out_vocabulary);
        initialize_const_output(out_property);
        initialize_const_output(out_value);
        ffi_call(out_error, || {
            require_output(out_target, "out_target")?;
            require_output(out_vocabulary, "out_vocabulary")?;
            require_output(out_property, "out_property")?;
            require_output(out_value, "out_value")?;
            let metadata = metadata
                .as_ref()
                .ok_or_else(|| invalid_argument("metadata must not be null"))?;
            let assertion = item_at(&metadata.assertions, index, "metadata assertion")?;
            out_target.write(assertion.target);
            out_vocabulary.write(assertion.vocabulary.as_ptr());
            out_property.write(assertion.property.as_ptr());
            out_value.write(ptr::from_ref(&assertion.value));
            Ok(())
        })
    }
}

/// Releases a metadata result set. Null is a no-op.
///
/// # Safety
///
/// A non-null pointer must be live and released exactly once. No borrowed value
/// or string from the set may be used after this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_set_release(metadata: *mut PpMetadataSet) {
    if metadata.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(metadata) });
    }));
}

/// Loads one complete dependency observation.
///
/// # Safety
///
/// `production` and `representation_id` must be live readable values,
/// `out_dependencies` must be writable, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_dependency_set(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    out_dependencies: *mut *mut PpDependencySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and result ownership is explicit.
    unsafe {
        initialize_output(out_dependencies);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_dependencies, "out_dependencies")?;
            let representation_id = RepresentationId::from_bytes(representation_id.bytes);
            let inner = lock_production(&production.state);
            let dependencies =
                PpDependencySet::new(representation_id, inner.dependency_set(representation_id)?)?;
            out_dependencies.write(Box::into_raw(Box::new(dependencies)));
            Ok(())
        })
    }
}

/// Reads dependency-observation summary fields.
///
/// # Safety
///
/// `dependencies` must be live, all outputs must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(
    clippy::too_many_arguments,
    reason = "the C ABI exposes each summary field as an explicit output"
)]
pub unsafe extern "C" fn pp_dependency_set_get(
    dependencies: *const PpDependencySet,
    out_present: *mut u8,
    out_source_representation_id: *mut PpRepresentationId,
    out_recorded_at_revision: *mut u64,
    out_status: *mut u32,
    out_dependency_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_present, 0);
        initialize_value(
            out_source_representation_id,
            PpRepresentationId { bytes: [0; 16] },
        );
        initialize_value(out_recorded_at_revision, 0);
        initialize_value(out_status, 0);
        initialize_value(out_dependency_count, 0);
        ffi_call(out_error, || {
            let dependencies = dependencies
                .as_ref()
                .ok_or_else(|| invalid_argument("dependencies must not be null"))?;
            require_output(out_present, "out_present")?;
            require_output(out_source_representation_id, "out_source_representation_id")?;
            require_output(out_recorded_at_revision, "out_recorded_at_revision")?;
            require_output(out_status, "out_status")?;
            require_output(out_dependency_count, "out_dependency_count")?;
            out_present.write(u8::from(dependencies.present));
            out_source_representation_id.write(PpRepresentationId {
                bytes: dependencies.source_representation_id.into_bytes(),
            });
            out_recorded_at_revision.write(dependencies.recorded_at_revision);
            out_status.write(dependencies.status);
            out_dependency_count.write(length_as_u64(dependencies.len())?);
            Ok(())
        })
    }
}

/// Reads one borrowed dependency edge.
///
/// # Safety
///
/// `dependencies` must be live, `out_dependency` writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_dependency_set_get_dependency(
    dependencies: *const PpDependencySet,
    index: u64,
    out_dependency: *mut PpDependency,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_dependency, zero_dependency());
        ffi_call(out_error, || {
            require_output(out_dependency, "out_dependency")?;
            let dependencies = dependencies
                .as_ref()
                .ok_or_else(|| invalid_argument("dependencies must not be null"))?;
            let index = usize::try_from(index)
                .map_err(|_| invalid_argument("dependency index is out of range"))?;
            let dependency = dependencies.get(index).ok_or_else(|| {
                invalid_argument(format!("dependency index {index} is out of range"))
            })?;
            out_dependency.write(dependency);
            Ok(())
        })
    }
}

/// Releases a dependency observation. Null is a no-op.
///
/// # Safety
///
/// A non-null pointer must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_dependency_set_release(dependencies: *mut PpDependencySet) {
    if dependencies.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(dependencies) });
    }));
}

/// Queries representations that directly or transitively depend on a target.
///
/// # Safety
///
/// `production` and `target` must be live readable values, `cursor` must be
/// null or NUL-terminated UTF-8 for this call, `out_matches`
/// must be writable, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_dependents(
    production: *const PpProduction,
    target: *const PpObjectRef,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut PpDependencyQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and result ownership is explicit.
    unsafe {
        initialize_output(out_matches);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let target = target
                .as_ref()
                .ok_or_else(|| invalid_argument("target must not be null"))?;
            require_output(out_matches, "out_matches")?;
            let target = match object_ref_from_abi(*target)? {
                ObjectRef::Asset(id) => DependencyTarget::Asset(id),
                ObjectRef::Representation(id) => DependencyTarget::Representation(id),
                _ => {
                    return Err(invalid_argument(
                        "dependency target must be an asset or representation",
                    ));
                }
            };
            let page_request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let page = inner.dependents(
                target,
                DependencyQueryLimits::new(max_depth, max_representations)?,
                &page_request,
            )?;
            out_matches.write(Box::into_raw(Box::new(PpDependencyQuerySet::new(
                page.items(),
                page.next_cursor(),
                page.traversal_truncated(),
            )?)));
            Ok(())
        })
    }
}

/// Queries direct or transitive dependencies of a representation.
///
/// # Safety
///
/// `production` and `representation_id` must be live readable values, `cursor`
/// must be null or NUL-terminated UTF-8 for this call, `out_matches` must be
/// writable, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_dependencies(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut PpDependencyQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and result ownership is explicit.
    unsafe {
        initialize_output(out_matches);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_matches, "out_matches")?;
            let page_request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let page = inner.dependencies(
                RepresentationId::from_bytes(representation_id.bytes),
                DependencyQueryLimits::new(max_depth, max_representations)?,
                &page_request,
            )?;
            out_matches.write(Box::into_raw(Box::new(PpDependencyQuerySet::new(
                page.items(),
                page.next_cursor(),
                page.traversal_truncated(),
            )?)));
            Ok(())
        })
    }
}

/// Returns the number of dependency matches in a page. Null returns zero.
///
/// # Safety
///
/// `matches` must be null or a live query-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_dependency_query_set_count(
    matches: *const PpDependencyQuerySet,
) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { matches.as_ref() }.map_or(0, |set| u64::try_from(set.len()).unwrap_or(u64::MAX))
    }))
    .unwrap_or(0)
}

/// Reads one dependency-query match.
///
/// # Safety
///
/// `matches` must be live, `out_match` writable, and `out_error` may be null
/// or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_dependency_query_set_get(
    matches: *const PpDependencyQuerySet,
    index: u64,
    out_match: *mut PpDependencyMatch,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(
            out_match,
            PpDependencyMatch {
                target: PpObjectRef {
                    kind: 0,
                    id: PpUuid { bytes: [0; 16] },
                },
                depth: 0,
            },
        );
        ffi_call(out_error, || {
            require_output(out_match, "out_match")?;
            let matches = matches
                .as_ref()
                .ok_or_else(|| invalid_argument("matches must not be null"))?;
            let index = usize::try_from(index)
                .map_err(|_| invalid_argument("dependency match index is too large"))?;
            out_match.write(
                matches
                    .get(index)
                    .ok_or_else(|| invalid_argument("dependency match index is out of range"))?,
            );
            Ok(())
        })
    }
}

/// Returns a borrowed next-page cursor, or null when this is the last page.
///
/// # Safety
///
/// `matches` must be null or a live query-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_dependency_query_set_next_cursor(
    matches: *const PpDependencyQuerySet,
) -> *const c_char {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { matches.as_ref() }.map_or(ptr::null(), PpDependencyQuerySet::next_cursor)
    }))
    .unwrap_or(ptr::null())
}

/// Reports whether a traversal bound made the page's query incomplete.
///
/// # Safety
///
/// `matches` must be null or a live query-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_dependency_query_set_traversal_truncated(
    matches: *const PpDependencyQuerySet,
) -> u8 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { matches.as_ref() }.map_or(0, |set| u8::from(set.traversal_truncated))
    }))
    .unwrap_or(0)
}

/// Releases a dependency query page. Null is a no-op.
///
/// # Safety
///
/// A non-null handle must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_dependency_query_set_release(matches: *mut PpDependencyQuerySet) {
    if matches.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(matches) });
    }));
}

/// Evaluates artifact knowledge without accessing media files.
///
/// # Safety
///
/// `production` and `representation_id` must be readable live values,
/// `out_evaluation` must be writable, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_evaluate_artifact(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    max_depth: u32,
    max_representations: u32,
    out_evaluation: *mut *mut PpArtifactEvaluation,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and output ownership is explicit.
    unsafe {
        initialize_output(out_evaluation);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_evaluation, "out_evaluation")?;
            let limits = ArtifactEvaluationLimits::new(max_depth, max_representations)?;
            let inner = lock_production(&production.state);
            let evaluation = inner.evaluate_artifact(
                RepresentationId::from_bytes(representation_id.bytes),
                limits,
            )?;
            let evaluation = PpArtifactEvaluation::new(&evaluation)?;
            out_evaluation.write(Box::into_raw(Box::new(evaluation)));
            Ok(())
        })
    }
}

/// Reads the artifact-evaluation summary.
///
/// # Safety
///
/// `evaluation` must be live. Every output must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_artifact_evaluation_get(
    evaluation: *const PpArtifactEvaluation,
    out_representation_id: *mut PpRepresentationId,
    out_state: *mut u32,
    out_visited_representations: *mut u32,
    out_truncated: *mut u8,
    out_reason_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_representation_id, PpRepresentationId { bytes: [0; 16] });
        initialize_value(out_state, 0);
        initialize_value(out_visited_representations, 0);
        initialize_value(out_truncated, 0);
        initialize_value(out_reason_count, 0);
        ffi_call(out_error, || {
            let evaluation = evaluation
                .as_ref()
                .ok_or_else(|| invalid_argument("evaluation must not be null"))?;
            require_output(out_representation_id, "out_representation_id")?;
            require_output(out_state, "out_state")?;
            require_output(out_visited_representations, "out_visited_representations")?;
            require_output(out_truncated, "out_truncated")?;
            require_output(out_reason_count, "out_reason_count")?;
            out_representation_id.write(PpRepresentationId {
                bytes: evaluation.representation_id.into_bytes(),
            });
            out_state.write(evaluation.state);
            out_visited_representations.write(evaluation.visited_representations);
            out_truncated.write(u8::from(evaluation.truncated));
            out_reason_count.write(length_as_u64(evaluation.reasons.len())?);
            Ok(())
        })
    }
}

/// Reads one borrowed artifact-evaluation reason.
///
/// # Safety
///
/// `evaluation` must be live, `out_reason` must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_artifact_evaluation_get_reason(
    evaluation: *const PpArtifactEvaluation,
    index: u64,
    out_reason: *mut PpArtifactReason,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_reason, zero_artifact_reason());
        ffi_call(out_error, || {
            require_output(out_reason, "out_reason")?;
            let evaluation = evaluation
                .as_ref()
                .ok_or_else(|| invalid_argument("evaluation must not be null"))?;
            let reason = item_at(&evaluation.reasons, index, "artifact reason")?;
            out_reason.write(reason.as_abi());
            Ok(())
        })
    }
}

/// Releases an artifact evaluation. Null is a no-op.
///
/// # Safety
///
/// A non-null pointer must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_artifact_evaluation_release(evaluation: *mut PpArtifactEvaluation) {
    if evaluation.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(evaluation) });
    }));
}

/// Reports whether production knowledge can reproduce an artifact.
///
/// # Safety
///
/// Pointer rules match [`pp_production_evaluate_artifact`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_artifact_reproducibility(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    out_report: *mut *mut PpArtifactReproducibility,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and output ownership is explicit.
    unsafe {
        initialize_output(out_report);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_report, "out_report")?;
            let inner = lock_production(&production.state);
            let report = inner
                .artifact_reproducibility(RepresentationId::from_bytes(representation_id.bytes))?;
            let report = PpArtifactReproducibility::new(&report)?;
            out_report.write(Box::into_raw(Box::new(report)));
            Ok(())
        })
    }
}

/// Reads an artifact-reproducibility summary.
///
/// # Safety
///
/// `report` must be live. Every output must be writable and `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(clippy::too_many_arguments, reason = "flat C outputs are ABI-safe")]
pub unsafe extern "C" fn pp_artifact_reproducibility_get(
    report: *const PpArtifactReproducibility,
    out_representation_id: *mut PpRepresentationId,
    out_reproducible: *mut u8,
    out_has_producing_activity: *mut u8,
    out_producing_activity_id: *mut PpActivityId,
    out_activity_kind: *mut *const c_char,
    out_issue_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_representation_id, PpRepresentationId { bytes: [0; 16] });
        initialize_value(out_reproducible, 0);
        initialize_value(out_has_producing_activity, 0);
        initialize_value(out_producing_activity_id, PpActivityId { bytes: [0; 16] });
        initialize_const_output(out_activity_kind);
        initialize_value(out_issue_count, 0);
        ffi_call(out_error, || {
            let report = report
                .as_ref()
                .ok_or_else(|| invalid_argument("report must not be null"))?;
            require_output(out_representation_id, "out_representation_id")?;
            require_output(out_reproducible, "out_reproducible")?;
            require_output(out_has_producing_activity, "out_has_producing_activity")?;
            require_output(out_producing_activity_id, "out_producing_activity_id")?;
            require_output(out_activity_kind, "out_activity_kind")?;
            require_output(out_issue_count, "out_issue_count")?;
            out_representation_id.write(PpRepresentationId {
                bytes: report.representation_id.into_bytes(),
            });
            out_reproducible.write(u8::from(report.issues.is_empty()));
            if let Some(activity_id) = report.producing_activity_id {
                out_has_producing_activity.write(1);
                out_producing_activity_id.write(PpActivityId {
                    bytes: activity_id.into_bytes(),
                });
            }
            if let Some(kind) = report.activity_kind.as_ref() {
                out_activity_kind.write(kind.as_ptr());
            }
            out_issue_count.write(length_as_u64(report.issues.len())?);
            Ok(())
        })
    }
}

/// Reads one reproducibility issue.
///
/// # Safety
///
/// `report` must be live, `out_issue` must be writable, and `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_artifact_reproducibility_get_issue(
    report: *const PpArtifactReproducibility,
    index: u64,
    out_issue: *mut PpArtifactReproducibilityIssue,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_issue, zero_reproducibility_issue());
        ffi_call(out_error, || {
            require_output(out_issue, "out_issue")?;
            let report = report
                .as_ref()
                .ok_or_else(|| invalid_argument("report must not be null"))?;
            out_issue.write(*item_at(
                &report.issues,
                index,
                "artifact reproducibility issue",
            )?);
            Ok(())
        })
    }
}

/// Releases an artifact-reproducibility report. Null is a no-op.
///
/// # Safety
///
/// A non-null pointer must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_artifact_reproducibility_release(
    report: *mut PpArtifactReproducibility,
) {
    if report.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(report) });
    }));
}

/// Loads every production activity in deterministic identity order.
///
/// # Safety
///
/// `production` must be live, `out_activities` must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_activities(
    production: *const PpProduction,
    out_activities: *mut *mut PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and output ownership is explicit.
    unsafe {
        initialize_output(out_activities);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_activities, "out_activities")?;
            let inner = lock_production(&production.state);
            let activities = PpActivitySet::new(&inner.activities()?)?;
            out_activities.write(Box::into_raw(Box::new(activities)));
            Ok(())
        })
    }
}

/// Loads activities that produce one representation.
///
/// # Safety
///
/// All pointers must follow the same rules as [`pp_production_activities`], and
/// `representation_id` must be readable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_activities_producing(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    out_activities: *mut *mut PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper validates every pointer before use.
    unsafe {
        production_activities_for_representation(
            production,
            representation_id,
            ActivityRelation::Producing,
            out_activities,
            out_error,
        )
    }
}

/// Loads activities that consume one representation.
///
/// # Safety
///
/// Pointer rules match [`pp_production_activities_producing`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_activities_consuming(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    out_activities: *mut *mut PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper validates every pointer before use.
    unsafe {
        production_activities_for_representation(
            production,
            representation_id,
            ActivityRelation::Consuming,
            out_activities,
            out_error,
        )
    }
}

/// Loads every transitive provenance ancestor as representation references.
///
/// # Safety
///
/// `production` and `representation_id` must be readable live values,
/// `out_representations` must be writable, and `out_error` may be null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_provenance_ancestors(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    out_representations: *mut *mut PpObjectRefSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper validates every pointer before use.
    unsafe {
        production_provenance_relatives(
            production,
            representation_id,
            ProvenanceDirection::Ancestors,
            out_representations,
            out_error,
        )
    }
}

/// Loads every transitive provenance descendant as representation references.
///
/// # Safety
///
/// Pointer rules match [`pp_production_provenance_ancestors`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_provenance_descendants(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    out_representations: *mut *mut PpObjectRefSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper validates every pointer before use.
    unsafe {
        production_provenance_relatives(
            production,
            representation_id,
            ProvenanceDirection::Descendants,
            out_representations,
            out_error,
        )
    }
}

/// Returns the number of activities in a result set. Null returns zero.
///
/// # Safety
///
/// `activities` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_count(activities: *const PpActivitySet) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { activities.as_ref() }.map_or(0, |set| {
            u64::try_from(set.activities.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Returns the borrowed activity-page continuation cursor, or null.
///
/// # Safety
///
/// `activities` must be null or a live activity set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_next_cursor(
    activities: *const PpActivitySet,
) -> *const c_char {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        activities.as_ref().map_or(ptr::null(), |set| {
            set.next_cursor
                .as_ref()
                .map_or(ptr::null(), |cursor| cursor.as_ptr())
        })
    }))
    .unwrap_or(ptr::null())
}

/// Reads one activity's identity, kind, timing, and edge counts.
///
/// Strings are borrowed until the result set is released. Optional timestamps
/// have explicit presence flags and zero values when absent.
///
/// # Safety
///
/// `activities` must be live. Every output must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(
    clippy::too_many_arguments,
    reason = "flat C output parameters are ABI-safe"
)]
pub unsafe extern "C" fn pp_activity_set_get(
    activities: *const PpActivitySet,
    index: u64,
    out_id: *mut PpActivityId,
    out_kind: *mut *const c_char,
    out_has_started_at: *mut u8,
    out_started_at_unix_micros: *mut i64,
    out_has_finished_at: *mut u8,
    out_finished_at_unix_micros: *mut i64,
    out_input_count: *mut u64,
    out_output_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_id, PpActivityId { bytes: [0; 16] });
        initialize_const_output(out_kind);
        initialize_value(out_has_started_at, 0);
        initialize_value(out_started_at_unix_micros, 0);
        initialize_value(out_has_finished_at, 0);
        initialize_value(out_finished_at_unix_micros, 0);
        initialize_value(out_input_count, 0);
        initialize_value(out_output_count, 0);
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            require_output(out_kind, "out_kind")?;
            require_output(out_has_started_at, "out_has_started_at")?;
            require_output(out_started_at_unix_micros, "out_started_at_unix_micros")?;
            require_output(out_has_finished_at, "out_has_finished_at")?;
            require_output(out_finished_at_unix_micros, "out_finished_at_unix_micros")?;
            require_output(out_input_count, "out_input_count")?;
            require_output(out_output_count, "out_output_count")?;
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, index, "activity")?;
            out_id.write(PpActivityId {
                bytes: activity.id.into_bytes(),
            });
            out_kind.write(activity.kind.as_ptr());
            if let Some(value) = activity.started_at_unix_micros {
                out_has_started_at.write(1);
                out_started_at_unix_micros.write(value);
            }
            if let Some(value) = activity.finished_at_unix_micros {
                out_has_finished_at.write(1);
                out_finished_at_unix_micros.write(value);
            }
            out_input_count.write(length_as_u64(activity.inputs.len())?);
            out_output_count.write(length_as_u64(activity.outputs.len())?);
            Ok(())
        })
    }
}

/// Reads one input edge and its optional borrowed role.
///
/// # Safety
///
/// `activities` must be live. Every output must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_get_input(
    activities: *const PpActivitySet,
    activity_index: u64,
    input_index: u64,
    out_representation_id: *mut PpRepresentationId,
    out_role: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper initializes and validates every output.
    unsafe {
        ffi_call(out_error, || {
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, activity_index, "activity")?;
            write_activity_edge(
                &activity.inputs,
                input_index,
                out_representation_id,
                out_role,
            )
        })
    }
}

/// Reads one output edge and its optional borrowed role.
///
/// # Safety
///
/// `activities` must be live. Every output must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_get_output(
    activities: *const PpActivitySet,
    activity_index: u64,
    output_index: u64,
    out_representation_id: *mut PpRepresentationId,
    out_role: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper initializes and validates every output.
    unsafe {
        ffi_call(out_error, || {
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, activity_index, "activity")?;
            write_activity_edge(
                &activity.outputs,
                output_index,
                out_representation_id,
                out_role,
            )
        })
    }
}

/// Reads the storage-captured snapshot summary for one input edge.
///
/// An absent snapshot is reported by a zero presence flag, revision, and count.
///
/// # Safety
///
/// `activities` must be live. Every output must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_get_input_snapshot(
    activities: *const PpActivitySet,
    activity_index: u64,
    input_index: u64,
    out_has_snapshot: *mut u8,
    out_revision_sequence: *mut u64,
    out_fingerprint_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper initializes and validates every output.
    unsafe {
        ffi_call(out_error, || {
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, activity_index, "activity")?;
            write_activity_edge_snapshot(
                &activity.inputs,
                input_index,
                out_has_snapshot,
                out_revision_sequence,
                out_fingerprint_count,
            )
        })
    }
}

/// Reads the storage-captured snapshot summary for one output edge.
///
/// Pointer and absence rules match [`pp_activity_set_get_input_snapshot`].
///
/// # Safety
///
/// All pointers follow the rules documented above.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_get_output_snapshot(
    activities: *const PpActivitySet,
    activity_index: u64,
    output_index: u64,
    out_has_snapshot: *mut u8,
    out_revision_sequence: *mut u64,
    out_fingerprint_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper initializes and validates every output.
    unsafe {
        ffi_call(out_error, || {
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, activity_index, "activity")?;
            write_activity_edge_snapshot(
                &activity.outputs,
                output_index,
                out_has_snapshot,
                out_revision_sequence,
                out_fingerprint_count,
            )
        })
    }
}

/// Reads one fingerprint from an input-edge snapshot.
///
/// Returned algorithm and value pointers borrow the activity set.
///
/// # Safety
///
/// `activities` must be live. Every output must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(clippy::too_many_arguments, reason = "flat C outputs are ABI-safe")]
pub unsafe extern "C" fn pp_activity_set_get_input_snapshot_fingerprint(
    activities: *const PpActivitySet,
    activity_index: u64,
    input_index: u64,
    fingerprint_index: u64,
    out_algorithm: *mut *const c_char,
    out_version: *mut u16,
    out_value: *mut *const u8,
    out_value_length: *mut u64,
    out_has_observed_revision: *mut u8,
    out_observed_revision_sequence: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper initializes and validates every output.
    unsafe {
        ffi_call(out_error, || {
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, activity_index, "activity")?;
            let edge = item_at(&activity.inputs, input_index, "activity input")?;
            write_snapshot_fingerprint(
                edge.snapshot.as_ref(),
                fingerprint_index,
                out_algorithm,
                out_version,
                out_value,
                out_value_length,
                out_has_observed_revision,
                out_observed_revision_sequence,
            )
        })
    }
}

/// Reads one fingerprint from an output-edge snapshot.
///
/// Pointer and ownership rules match
/// [`pp_activity_set_get_input_snapshot_fingerprint`].
///
/// # Safety
///
/// All pointers follow the rules documented above.
#[postproject_ffi_macros::ffi_export]
#[allow(clippy::too_many_arguments, reason = "flat C outputs are ABI-safe")]
pub unsafe extern "C" fn pp_activity_set_get_output_snapshot_fingerprint(
    activities: *const PpActivitySet,
    activity_index: u64,
    output_index: u64,
    fingerprint_index: u64,
    out_algorithm: *mut *const c_char,
    out_version: *mut u16,
    out_value: *mut *const u8,
    out_value_length: *mut u64,
    out_has_observed_revision: *mut u8,
    out_observed_revision_sequence: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The shared helper initializes and validates every output.
    unsafe {
        ffi_call(out_error, || {
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, activity_index, "activity")?;
            let edge = item_at(&activity.outputs, output_index, "activity output")?;
            write_snapshot_fingerprint(
                edge.snapshot.as_ref(),
                fingerprint_index,
                out_algorithm,
                out_version,
                out_value,
                out_value_length,
                out_has_observed_revision,
                out_observed_revision_sequence,
            )
        })
    }
}

/// Reads the optional tool identity for one activity.
///
/// Every returned string is borrowed. All outputs are null when no tool was
/// recorded; version and URI may independently be null for a present tool.
///
/// # Safety
///
/// `activities` must be live. Every output must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_get_tool(
    activities: *const PpActivitySet,
    index: u64,
    out_name: *mut *const c_char,
    out_version: *mut *const c_char,
    out_uri: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_const_output(out_name);
        initialize_const_output(out_version);
        initialize_const_output(out_uri);
        ffi_call(out_error, || {
            require_output(out_name, "out_name")?;
            require_output(out_version, "out_version")?;
            require_output(out_uri, "out_uri")?;
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, index, "activity")?;
            if let Some(tool) = &activity.tool {
                out_name.write(tool.name.as_ptr());
                out_version.write(
                    tool.version
                        .as_ref()
                        .map_or(ptr::null(), |value| value.as_ptr()),
                );
                out_uri.write(
                    tool.uri
                        .as_ref()
                        .map_or(ptr::null(), |value| value.as_ptr()),
                );
            }
            Ok(())
        })
    }
}

/// Reads the optional agent identity for one activity.
///
/// Every returned string is borrowed and nullable. A present agent has a name,
/// an external identifier, or both.
///
/// # Safety
///
/// `activities` must be live. Every output must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_get_agent(
    activities: *const PpActivitySet,
    index: u64,
    out_name: *mut *const c_char,
    out_identifier_scheme: *mut *const c_char,
    out_identifier_value: *mut *const c_char,
    out_identifier_qualifier: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_const_output(out_name);
        initialize_const_output(out_identifier_scheme);
        initialize_const_output(out_identifier_value);
        initialize_const_output(out_identifier_qualifier);
        ffi_call(out_error, || {
            require_output(out_name, "out_name")?;
            require_output(out_identifier_scheme, "out_identifier_scheme")?;
            require_output(out_identifier_value, "out_identifier_value")?;
            require_output(out_identifier_qualifier, "out_identifier_qualifier")?;
            let activities = activities
                .as_ref()
                .ok_or_else(|| invalid_argument("activities must not be null"))?;
            let activity = item_at(&activities.activities, index, "activity")?;
            if let Some(agent) = &activity.agent {
                out_name.write(
                    agent
                        .name
                        .as_ref()
                        .map_or(ptr::null(), |value| value.as_ptr()),
                );
                out_identifier_scheme.write(
                    agent
                        .identifier_scheme
                        .as_ref()
                        .map_or(ptr::null(), |value| value.as_ptr()),
                );
                out_identifier_value.write(
                    agent
                        .identifier_value
                        .as_ref()
                        .map_or(ptr::null(), |value| value.as_ptr()),
                );
                out_identifier_qualifier.write(
                    agent
                        .identifier_qualifier
                        .as_ref()
                        .map_or(ptr::null(), |value| value.as_ptr()),
                );
            }
            Ok(())
        })
    }
}

/// Releases an activity result set. Null is a no-op.
///
/// # Safety
///
/// A non-null handle must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_activity_set_release(activities: *mut PpActivitySet) {
    if activities.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(activities) });
    }));
}

/// Reads one durable job by identity as a one-element job set.
///
/// # Safety
///
/// `production` must be live, `out_jobs` must be writable, and
/// `out_error` may be null or writable. The returned set is caller-owned.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_job(
    production: *const PpProduction,
    job_id: PpJobId,
    out_jobs: *mut *mut PpJobSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked before use and output ownership is explicit.
    unsafe {
        initialize_output(out_jobs);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_jobs, "out_jobs")?;
            let job = lock_production(&production.state).job(JobId::from_bytes(job_id.bytes))?;
            out_jobs.write(Box::into_raw(Box::new(PpJobSet::new_page(&[job], None)?)));
            Ok(())
        })
    }
}

/// Queries one page of durable jobs in stable identity order.
///
/// # Safety
///
/// `production` must be live; `kind` and `cursor` must each be null or
/// NUL-terminated UTF-8 for this call; `out_jobs` must be writable; and
/// `out_error` may be null or writable. State zero means any state.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_jobs(
    production: *const PpProduction,
    state: u32,
    kind: *const c_char,
    limit: u32,
    cursor: *const c_char,
    out_jobs: *mut *mut PpJobSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked before use and output ownership is explicit.
    unsafe {
        initialize_output(out_jobs);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_jobs, "out_jobs")?;
            let query = JobQuery::new(
                job_state_kind_from_abi(state)?,
                optional_utf8(kind, "job kind")?
                    .map(JobKind::new)
                    .transpose()?,
            );
            let page_request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let page = inner.jobs(&query, &page_request)?;
            out_jobs.write(Box::into_raw(Box::new(PpJobSet::new_page(
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

/// Returns a borrowed next-page cursor, or null when this is the last page.
///
/// # Safety
///
/// `jobs` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_set_next_cursor(jobs: *const PpJobSet) -> *const c_char {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { jobs.as_ref() }.map_or(ptr::null(), PpJobSet::next_cursor)
    }))
    .unwrap_or(ptr::null())
}

/// Returns the number of jobs in a result set. Null returns zero.
///
/// # Safety
///
/// `jobs` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_set_count(jobs: *const PpJobSet) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { jobs.as_ref() }.map_or(0, |set| u64::try_from(set.len()).unwrap_or(u64::MAX))
    }))
    .unwrap_or(0)
}

/// Reads one borrowed job view.
///
/// # Safety
///
/// `jobs` must be live, `out_job` must be writable, and `out_error` may be null
/// or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_set_get(
    jobs: *const PpJobSet,
    index: u64,
    out_job: *mut PpJob,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_job, PpJob::empty());
        ffi_call(out_error, || {
            let jobs = jobs
                .as_ref()
                .ok_or_else(|| invalid_argument("jobs must not be null"))?;
            require_output(out_job, "out_job")?;
            let index =
                usize::try_from(index).map_err(|_| invalid_argument("job index is too large"))?;
            let job = jobs
                .get(index)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "job index is out of range"))?;
            out_job.write(job);
            Ok(())
        })
    }
}

/// Reads one input representation from a job.
///
/// # Safety
///
/// `jobs` must be live, `out_representation_id` must be writable, and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_set_get_input(
    jobs: *const PpJobSet,
    job_index: u64,
    input_index: u64,
    out_representation_id: *mut PpRepresentationId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_representation_id, PpRepresentationId { bytes: [0; 16] });
        ffi_call(out_error, || {
            let jobs = jobs
                .as_ref()
                .ok_or_else(|| invalid_argument("jobs must not be null"))?;
            require_output(out_representation_id, "out_representation_id")?;
            let job_index = usize::try_from(job_index)
                .map_err(|_| invalid_argument("job index is too large"))?;
            let input_index = usize::try_from(input_index)
                .map_err(|_| invalid_argument("job input index is too large"))?;
            let input = jobs.input(job_index, input_index).ok_or_else(|| {
                Error::new(ErrorKind::NotFound, "job or input index is out of range")
            })?;
            out_representation_id.write(input);
            Ok(())
        })
    }
}

/// Releases a job result set. Null is a no-op.
///
/// # Safety
///
/// A non-null handle must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_job_set_release(jobs: *mut PpJobSet) {
    if jobs.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(jobs) });
    }));
}

/// Plans explicit regeneration requests without persisting or running work.
///
/// # Safety
///
/// `production` must be live; the artifact array must contain `artifact_count`
/// readable UUIDs (or be null for zero); `out_plans` must be writable; and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_plan_regeneration(
    production: *const PpProduction,
    artifact_representation_ids: *const PpRepresentationId,
    artifact_count: u64,
    out_plans: *mut *mut PpRegenerationPlanSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked before use and output ownership is explicit.
    unsafe {
        initialize_output(out_plans);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_plans, "out_plans")?;
            let artifact_count = usize::try_from(artifact_count)
                .map_err(|_| invalid_argument("artifact count is too large"))?;
            if artifact_count > postproject_core::MAX_REGENERATION_PLANS {
                return Err(invalid_argument(
                    "too many artifacts for regeneration planning",
                ));
            }
            let artifacts = if artifact_count == 0 {
                Vec::new()
            } else {
                if artifact_representation_ids.is_null() {
                    return Err(invalid_argument(
                        "artifact_representation_ids must not be null when count is nonzero",
                    ));
                }
                // SAFETY: The caller guarantees `artifact_count` readable UUIDs.
                std::slice::from_raw_parts(artifact_representation_ids, artifact_count)
                    .iter()
                    .map(|id| RepresentationId::from_bytes(id.bytes))
                    .collect()
            };
            let inner = lock_production(&production.state);
            let plans = inner.plan_regeneration(&artifacts)?;
            out_plans.write(Box::into_raw(Box::new(PpRegenerationPlanSet::new(plans))));
            Ok(())
        })
    }
}

/// Returns the number of regeneration plans. Null returns zero.
///
/// # Safety
///
/// `plans` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_regeneration_plan_set_count(
    plans: *const PpRegenerationPlanSet,
) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { plans.as_ref() }.map_or(0, |set| u64::try_from(set.len()).unwrap_or(u64::MAX))
    }))
    .unwrap_or(0)
}

/// Copies one artifact ID and transfers owned one-job and parameter sets.
///
/// The returned metadata assertions are targeted at the planned job ID so they
/// can be copied directly when the caller explicitly enqueues that job.
///
/// # Safety
///
/// `plans` must be live; every output must be writable; and `out_error` may be
/// null or writable. Each returned set must be released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_regeneration_plan_set_get(
    plans: *const PpRegenerationPlanSet,
    index: u64,
    out_artifact_representation_id: *mut PpRepresentationId,
    out_job: *mut *mut PpJobSet,
    out_parameters: *mut *mut PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(
            out_artifact_representation_id,
            PpRepresentationId { bytes: [0; 16] },
        );
        initialize_output(out_job);
        initialize_output(out_parameters);
        ffi_call(out_error, || {
            require_output(
                out_artifact_representation_id,
                "out_artifact_representation_id",
            )?;
            require_output(out_job, "out_job")?;
            require_output(out_parameters, "out_parameters")?;
            let plans = plans
                .as_ref()
                .ok_or_else(|| invalid_argument("plans must not be null"))?;
            let index = usize::try_from(index)
                .map_err(|_| invalid_argument("regeneration plan index is too large"))?;
            let plan = plans.get(index).ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    "regeneration plan index is out of range",
                )
            })?;
            let job = Box::new(PpJobSet::new(std::slice::from_ref(plan.job()))?);
            let parameters = Box::new(PpMetadataSet::from_assertions(
                ObjectRef::Job(plan.job().id()),
                plan.parameters(),
            )?);
            out_artifact_representation_id.write(PpRepresentationId {
                bytes: plan.artifact_representation_id().into_bytes(),
            });
            out_job.write(Box::into_raw(job));
            out_parameters.write(Box::into_raw(parameters));
            Ok(())
        })
    }
}

/// Releases a regeneration-plan result set. Null is a no-op.
///
/// # Safety
///
/// A non-null handle must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_regeneration_plan_set_release(plans: *mut PpRegenerationPlanSet) {
    if plans.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(plans) });
    }));
}

/// Loads the newest revision as a zero-or-one-element owned result set.
///
/// # Safety
///
/// `production` must be live, `out_revisions` must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_latest_revision(
    production: *const PpProduction,
    out_revisions: *mut *mut PpRevisionSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and output ownership is explicit.
    unsafe {
        initialize_output(out_revisions);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_revisions, "out_revisions")?;
            let inner = lock_production(&production.state);
            let revisions: Vec<_> = inner.latest_revision()?.into_iter().collect();
            out_revisions.write(Box::into_raw(Box::new(PpRevisionSet::new(&revisions)?)));
            Ok(())
        })
    }
}

/// Loads an ascending, bounded revision page after `sequence`.
///
/// # Safety
///
/// Pointer rules match [`pp_production_latest_revision`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_changes_since(
    production: *const PpProduction,
    sequence: u64,
    limit: u32,
    out_revisions: *mut *mut PpRevisionSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and output ownership is explicit.
    unsafe {
        initialize_output(out_revisions);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_revisions, "out_revisions")?;
            let inner = lock_production(&production.state);
            let revisions = PpRevisionSet::new(&inner.changes_since(sequence, limit)?)?;
            out_revisions.write(Box::into_raw(Box::new(revisions)));
            Ok(())
        })
    }
}

/// Returns the number of revisions in a result set. Null returns zero.
///
/// # Safety
///
/// `revisions` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_set_count(revisions: *const PpRevisionSet) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { revisions.as_ref() }.map_or(0, |set| {
            u64::try_from(set.revisions.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Reads one revision summary. Returned strings borrow the result-set lifetime.
///
/// # Safety
///
/// `revisions` must be live. Every output must be writable and `out_error` may
/// be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(
    clippy::too_many_arguments,
    reason = "flat C output parameters are ABI-safe"
)]
pub unsafe extern "C" fn pp_revision_set_get(
    revisions: *const PpRevisionSet,
    index: u64,
    out_id: *mut PpRevisionId,
    out_sequence: *mut u64,
    out_transaction_id: *mut PpTransactionId,
    out_committed_at_unix_micros: *mut i64,
    out_origin_name: *mut *const c_char,
    out_origin_version: *mut *const c_char,
    out_origin_uri: *mut *const c_char,
    out_message: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_id, PpRevisionId { bytes: [0; 16] });
        initialize_value(out_sequence, 0);
        initialize_value(out_transaction_id, PpTransactionId { bytes: [0; 16] });
        initialize_value(out_committed_at_unix_micros, 0);
        initialize_const_output(out_origin_name);
        initialize_const_output(out_origin_version);
        initialize_const_output(out_origin_uri);
        initialize_const_output(out_message);
        ffi_call(out_error, || {
            require_output(out_id, "out_id")?;
            require_output(out_sequence, "out_sequence")?;
            require_output(out_transaction_id, "out_transaction_id")?;
            require_output(out_committed_at_unix_micros, "out_committed_at_unix_micros")?;
            require_output(out_origin_name, "out_origin_name")?;
            require_output(out_origin_version, "out_origin_version")?;
            require_output(out_origin_uri, "out_origin_uri")?;
            require_output(out_message, "out_message")?;
            let revisions = revisions
                .as_ref()
                .ok_or_else(|| invalid_argument("revisions must not be null"))?;
            let revision = item_at(&revisions.revisions, index, "revision")?;
            out_id.write(PpRevisionId {
                bytes: revision.id.into_bytes(),
            });
            out_sequence.write(revision.sequence);
            out_transaction_id.write(PpTransactionId {
                bytes: revision.transaction_id.into_bytes(),
            });
            out_committed_at_unix_micros.write(revision.committed_at_unix_micros);
            out_origin_name.write(
                revision
                    .origin_name
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
            );
            out_origin_version.write(
                revision
                    .origin_version
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
            );
            out_origin_uri.write(
                revision
                    .origin_uri
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
            );
            out_message.write(
                revision
                    .message
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
            );
            Ok(())
        })
    }
}

/// Releases a revision result set. Null is a no-op.
///
/// # Safety
///
/// A non-null handle must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_set_release(revisions: *mut PpRevisionSet) {
    if revisions.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(revisions) });
    }));
}

/// Loads one revision's ordered semantic events.
///
/// # Safety
///
/// `production` must be live, `out_events` writable, and `out_error` null or
/// writable. Revision identity is a copied stack value.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_revision_events(
    production: *const PpProduction,
    revision_id: PpRevisionId,
    out_events: *mut *mut PpRevisionEventSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before use and output ownership is explicit.
    unsafe {
        initialize_output(out_events);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_events, "out_events")?;
            let inner = lock_production(&production.state);
            let events = inner.events_for_revision(RevisionId::from_bytes(revision_id.bytes))?;
            out_events.write(Box::into_raw(Box::new(PpRevisionEventSet::new(&events)?)));
            Ok(())
        })
    }
}

/// Returns the number of events in a result set. Null returns zero.
///
/// # Safety
///
/// `events` must be null or a live result-set handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_event_set_count(events: *const PpRevisionEventSet) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null handle is live by the caller contract.
        unsafe { events.as_ref() }
            .map_or(0, |set| u64::try_from(set.events.len()).unwrap_or(u64::MAX))
    }))
    .unwrap_or(0)
}

/// Reads one tagged semantic event. Borrowed strings live with the result set.
///
/// # Safety
///
/// `events` must be live, `out_event` must be writable, and `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_event_set_get(
    events: *const PpRevisionEventSet,
    index: u64,
    out_event: *mut PpRevisionEvent,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_event, empty_revision_event());
        ffi_call(out_error, || {
            require_output(out_event, "out_event")?;
            let events = events
                .as_ref()
                .ok_or_else(|| invalid_argument("events must not be null"))?;
            out_event.write(item_at(&events.events, index, "revision event")?.as_abi());
            Ok(())
        })
    }
}

/// Releases a revision-event result set. Null is a no-op.
///
/// # Safety
///
/// A non-null handle must be live and released exactly once.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_event_set_release(events: *mut PpRevisionEventSet) {
    if events.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership is transferred back exactly once by contract.
        drop(unsafe { Box::from_raw(events) });
    }));
}

/// Returns the `PP_METADATA_*` kind of a borrowed value. Null returns zero.
///
/// # Safety
///
/// `value` must be null or borrowed from a live metadata result set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_kind(value: *const PpMetadataValue) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null value is live by the caller contract.
        unsafe { value.as_ref() }.map_or(0, PpMetadataValue::kind)
    }))
    .unwrap_or(0)
}

/// Reads plain or language-tagged text. Language is null for plain text.
///
/// # Safety
///
/// `value` must be borrowed and live. Outputs must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_string(
    value: *const PpMetadataValue,
    out_text: *mut *const c_char,
    out_language: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_const_output(out_text);
        initialize_const_output(out_language);
        ffi_call(out_error, || {
            require_output(out_text, "out_text")?;
            require_output(out_language, "out_language")?;
            match &metadata_value(value)?.inner {
                AbiMetadataValue::String { value, language } => {
                    out_text.write(value.as_ptr());
                    out_language.write(language.as_ref().map_or(ptr::null(), |tag| tag.as_ptr()));
                    Ok(())
                }
                _ => Err(metadata_type_error("string")),
            }
        })
    }
}

/// Reads a signed integer value.
///
/// # Safety
///
/// `value` must be borrowed and live. `out_value` must be writable and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_i64(
    value: *const PpMetadataValue,
    out_value: *mut i64,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_value(out_value, 0);
        ffi_call(out_error, || match &metadata_value(value)?.inner {
            AbiMetadataValue::I64(stored) => write_copy(out_value, *stored, "out_value"),
            _ => Err(metadata_type_error("i64")),
        })
    }
}

/// Reads an unsigned integer value.
///
/// # Safety
///
/// Pointer rules match [`pp_metadata_value_get_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_u64(
    value: *const PpMetadataValue,
    out_value: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_value(out_value, 0);
        ffi_call(out_error, || match &metadata_value(value)?.inner {
            AbiMetadataValue::U64(stored) => write_copy(out_value, *stored, "out_value"),
            _ => Err(metadata_type_error("u64")),
        })
    }
}

/// Reads an exact decimal coefficient string and fractional scale.
///
/// # Safety
///
/// `value` must be borrowed and live. Outputs must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_decimal(
    value: *const PpMetadataValue,
    out_coefficient: *mut *const c_char,
    out_scale: *mut u32,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_const_output(out_coefficient);
        initialize_value(out_scale, 0);
        ffi_call(out_error, || {
            require_output(out_coefficient, "out_coefficient")?;
            require_output(out_scale, "out_scale")?;
            match &metadata_value(value)?.inner {
                AbiMetadataValue::Decimal { coefficient, scale } => {
                    out_coefficient.write(coefficient.as_ptr());
                    out_scale.write(*scale);
                    Ok(())
                }
                _ => Err(metadata_type_error("decimal")),
            }
        })
    }
}

/// Reads a boolean as zero or one.
///
/// # Safety
///
/// `value` must be borrowed and live. `out_value` must be writable and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_bool(
    value: *const PpMetadataValue,
    out_value: *mut u8,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_value(out_value, 0);
        ffi_call(out_error, || match &metadata_value(value)?.inner {
            AbiMetadataValue::Bool(stored) => write_copy(out_value, u8::from(*stored), "out_value"),
            _ => Err(metadata_type_error("bool")),
        })
    }
}

/// Reads a signed Unix-microsecond timestamp.
///
/// # Safety
///
/// Pointer rules match [`pp_metadata_value_get_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_timestamp(
    value: *const PpMetadataValue,
    out_unix_micros: *mut i64,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_value(out_unix_micros, 0);
        ffi_call(out_error, || match &metadata_value(value)?.inner {
            AbiMetadataValue::Timestamp(stored) => {
                write_copy(out_unix_micros, *stored, "out_unix_micros")
            }
            _ => Err(metadata_type_error("timestamp")),
        })
    }
}

/// Reads borrowed URI text.
///
/// # Safety
///
/// `value` must be borrowed and live. `out_uri` must be writable and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_uri(
    value: *const PpMetadataValue,
    out_uri: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_const_output(out_uri);
        ffi_call(out_error, || {
            require_output(out_uri, "out_uri")?;
            match &metadata_value(value)?.inner {
                AbiMetadataValue::Uri(stored) => {
                    out_uri.write(stored.as_ptr());
                    Ok(())
                }
                _ => Err(metadata_type_error("URI")),
            }
        })
    }
}

/// Reads borrowed opaque bytes and their length.
///
/// # Safety
///
/// `value` must be borrowed and live. Outputs must be writable and `out_error`
/// may be null or writable. The byte pointer remains valid with the result set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_bytes(
    value: *const PpMetadataValue,
    out_bytes: *mut *const u8,
    out_length: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_const_output(out_bytes);
        initialize_value(out_length, 0);
        ffi_call(out_error, || {
            require_output(out_bytes, "out_bytes")?;
            require_output(out_length, "out_length")?;
            match &metadata_value(value)?.inner {
                AbiMetadataValue::Bytes(stored) => {
                    out_bytes.write(stored.as_ptr());
                    out_length.write(length_as_u64(stored.len())?);
                    Ok(())
                }
                _ => Err(metadata_type_error("bytes")),
            }
        })
    }
}

/// Reads an exact rational numerator and positive denominator.
///
/// # Safety
///
/// `value` must be borrowed and live. Outputs must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_rational(
    value: *const PpMetadataValue,
    out_numerator: *mut i64,
    out_denominator: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_value(out_numerator, 0);
        initialize_value(out_denominator, 0);
        ffi_call(out_error, || {
            require_output(out_numerator, "out_numerator")?;
            require_output(out_denominator, "out_denominator")?;
            match &metadata_value(value)?.inner {
                AbiMetadataValue::Rational {
                    numerator,
                    denominator,
                } => {
                    out_numerator.write(*numerator);
                    out_denominator.write(*denominator);
                    Ok(())
                }
                _ => Err(metadata_type_error("rational")),
            }
        })
    }
}

/// Returns the number of list items. Null or a non-list value returns zero.
///
/// # Safety
///
/// `value` must be null or borrowed from a live metadata result set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_list_count(value: *const PpMetadataValue) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null value is live by the caller contract.
        unsafe { value.as_ref() }.map_or(0, |value| match &value.inner {
            AbiMetadataValue::List(items) => u64::try_from(items.len()).unwrap_or(u64::MAX),
            _ => 0,
        })
    }))
    .unwrap_or(0)
}

/// Reads one borrowed child from a list value.
///
/// # Safety
///
/// `value` must be borrowed and live. `out_item` must be writable and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_list_get(
    value: *const PpMetadataValue,
    index: u64,
    out_item: *mut *const PpMetadataValue,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_const_output(out_item);
        ffi_call(out_error, || {
            require_output(out_item, "out_item")?;
            match &metadata_value(value)?.inner {
                AbiMetadataValue::List(items) => {
                    out_item.write(ptr::from_ref(item_at(items, index, "metadata list item")?));
                    Ok(())
                }
                _ => Err(metadata_type_error("list")),
            }
        })
    }
}

/// Returns the number of fields. Null or a non-structure value returns zero.
///
/// # Safety
///
/// `value` must be null or borrowed from a live metadata result set.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_struct_count(value: *const PpMetadataValue) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null value is live by the caller contract.
        unsafe { value.as_ref() }.map_or(0, |value| match &value.inner {
            AbiMetadataValue::Struct(fields) => u64::try_from(fields.len()).unwrap_or(u64::MAX),
            _ => 0,
        })
    }))
    .unwrap_or(0)
}

/// Reads one borrowed name/value pair from a structured value.
///
/// # Safety
///
/// `value` must be borrowed and live. Outputs must be writable and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_struct_get(
    value: *const PpMetadataValue,
    index: u64,
    out_name: *mut *const c_char,
    out_field_value: *mut *const PpMetadataValue,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_const_output(out_name);
        initialize_const_output(out_field_value);
        ffi_call(out_error, || {
            require_output(out_name, "out_name")?;
            require_output(out_field_value, "out_field_value")?;
            match &metadata_value(value)?.inner {
                AbiMetadataValue::Struct(fields) => {
                    let field = item_at(fields, index, "metadata structure field")?;
                    out_name.write(field.name.as_ptr());
                    out_field_value.write(ptr::from_ref(&field.value));
                    Ok(())
                }
                _ => Err(metadata_type_error("structure")),
            }
        })
    }
}

/// Reads a typed `PostProject` object reference.
///
/// # Safety
///
/// `value` must be borrowed and live. `out_reference` must be writable and
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_value_get_reference(
    value: *const PpMetadataValue,
    out_reference: *mut PpObjectRef,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_object_ref(out_reference);
        ffi_call(out_error, || match &metadata_value(value)?.inner {
            AbiMetadataValue::Reference(reference) => {
                write_copy(out_reference, *reference, "out_reference")
            }
            _ => Err(metadata_type_error("reference")),
        })
    }
}

/// Returns the number of representation results in a resolution set.
/// Null input returns zero.
///
/// # Safety
///
/// `resolutions` must be null or a live handle returned by this library.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_representation_count(
    resolutions: *const PpResolutionSet,
) -> u64 {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null pointer is live for this call by the caller contract.
        unsafe { resolutions.as_ref() }.map_or(0, |set| {
            u64::try_from(set.representations.len()).unwrap_or(u64::MAX)
        })
    }))
    .unwrap_or(0)
}

/// Reads one representation-level availability result.
///
/// # Safety
///
/// `resolutions` must be live. Every output must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_get_representation(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    out_asset_id: *mut PpAssetId,
    out_representation_id: *mut PpRepresentationId,
    out_availability: *mut u32,
    out_resource_count: *mut u64,
    out_issue_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_asset_id, PpAssetId { bytes: [0; 16] });
        initialize_value(out_representation_id, PpRepresentationId { bytes: [0; 16] });
        initialize_value(out_availability, 0);
        initialize_value(out_resource_count, 0);
        initialize_value(out_issue_count, 0);
        ffi_call(out_error, || {
            require_output(out_asset_id, "out_asset_id")?;
            require_output(out_representation_id, "out_representation_id")?;
            require_output(out_availability, "out_availability")?;
            require_output(out_resource_count, "out_resource_count")?;
            require_output(out_issue_count, "out_issue_count")?;
            let resolution = representation_resolution_at(resolutions, representation_index)?;
            out_asset_id.write(PpAssetId {
                bytes: resolution.asset_id.into_bytes(),
            });
            out_representation_id.write(PpRepresentationId {
                bytes: resolution.representation_id.into_bytes(),
            });
            out_availability.write(representation_availability(resolution.availability));
            out_resource_count.write(length_as_u64(resolution.resources.len())?);
            out_issue_count.write(length_as_u64(resolution.issues.len())?);
            Ok(())
        })
    }
}

/// Reads one resource result nested under a representation result.
///
/// # Safety
///
/// `resolutions` must be live. Every output must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_get_resource(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    resource_index: u64,
    out_resource_id: *mut PpResourceId,
    out_state: *mut u32,
    out_candidate_count: *mut u64,
    out_evidence_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_resource_id, PpResourceId { bytes: [0; 16] });
        initialize_value(out_state, 0);
        initialize_value(out_candidate_count, 0);
        initialize_value(out_evidence_count, 0);
        ffi_call(out_error, || {
            require_output(out_resource_id, "out_resource_id")?;
            require_output(out_state, "out_state")?;
            require_output(out_candidate_count, "out_candidate_count")?;
            require_output(out_evidence_count, "out_evidence_count")?;
            let resolution =
                resource_resolution_at(resolutions, representation_index, resource_index)?;
            out_resource_id.write(PpResourceId {
                bytes: resolution.resource_id.into_bytes(),
            });
            out_state.write(resolution.state);
            out_candidate_count.write(length_as_u64(resolution.candidates.len())?);
            out_evidence_count.write(length_as_u64(resolution.evidence.len())?);
            Ok(())
        })
    }
}

/// Reads one representation availability issue.
///
/// # Safety
///
/// `resolutions` must be live. Every output must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_get_issue(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    issue_index: u64,
    out_resource_id: *mut PpResourceId,
    out_required: *mut u8,
    out_kind: *mut u32,
    out_frame_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_value(out_resource_id, PpResourceId { bytes: [0; 16] });
        initialize_value(out_required, 0);
        initialize_value(out_kind, 0);
        initialize_value(out_frame_count, 0);
        ffi_call(out_error, || {
            require_output(out_resource_id, "out_resource_id")?;
            require_output(out_required, "out_required")?;
            require_output(out_kind, "out_kind")?;
            require_output(out_frame_count, "out_frame_count")?;
            let representation = representation_resolution_at(resolutions, representation_index)?;
            let issue = item_at(&representation.issues, issue_index, "availability issue")?;
            out_resource_id.write(PpResourceId {
                bytes: issue.resource_id().into_bytes(),
            });
            out_required.write(u8::from(issue.is_required()));
            out_kind.write(availability_issue_kind(issue.kind()));
            out_frame_count.write(length_as_u64(issue.frames().len())?);
            Ok(())
        })
    }
}

/// Reads one missing-frame value from an availability issue.
///
/// # Safety
///
/// `resolutions` must be live. `out_frame` must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_get_issue_frame(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    issue_index: u64,
    frame_index: u64,
    out_frame: *mut i64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Output is initialized and checked before writes.
    unsafe {
        initialize_value(out_frame, 0);
        ffi_call(out_error, || {
            require_output(out_frame, "out_frame")?;
            let representation = representation_resolution_at(resolutions, representation_index)?;
            let issue = item_at(&representation.issues, issue_index, "availability issue")?;
            let frame = item_at(issue.frames(), frame_index, "missing frame")?;
            out_frame.write(*frame);
            Ok(())
        })
    }
}

/// Reads one candidate nested under a resource result.
///
/// The URI is borrowed until the resolution set is released.
///
/// # Safety
///
/// `resolutions` must be live. Every output must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_get_candidate(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    resource_index: u64,
    candidate_index: u64,
    out_uri: *mut *const c_char,
    out_confidence_basis_points: *mut u16,
    out_media_root: *mut *const c_char,
    out_has_sequence_naming: *mut u8,
    out_sequence_naming: *mut PpSequenceNaming,
    out_evidence_count: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and checked before writes.
    unsafe {
        initialize_const_output(out_uri);
        initialize_value(out_confidence_basis_points, 0);
        initialize_const_output(out_media_root);
        initialize_naming_output(out_has_sequence_naming, out_sequence_naming);
        initialize_value(out_evidence_count, 0);
        ffi_call(out_error, || {
            require_output(out_has_sequence_naming, "out_has_sequence_naming")?;
            require_output(out_sequence_naming, "out_sequence_naming")?;
            require_output(out_uri, "out_uri")?;
            require_output(out_media_root, "out_media_root")?;
            require_output(out_confidence_basis_points, "out_confidence_basis_points")?;
            require_output(out_evidence_count, "out_evidence_count")?;
            let resource =
                resource_resolution_at(resolutions, representation_index, resource_index)?;
            let candidate = item_at(&resource.candidates, candidate_index, "candidate")?;
            out_uri.write(candidate.uri.as_ptr());
            out_confidence_basis_points.write(candidate.confidence);
            out_media_root.write(
                candidate
                    .media_root
                    .as_ref()
                    .map_or(ptr::null(), |root| root.as_ptr()),
            );
            write_naming_output(
                candidate.sequence_naming.as_ref(),
                out_has_sequence_naming,
                out_sequence_naming,
            );
            out_evidence_count.write(length_as_u64(candidate.evidence.len())?);
            Ok(())
        })
    }
}

/// Reads resource-level evidence.
///
/// # Safety
///
/// `resolutions` must be live. Outputs must be writable, and `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_get_resource_evidence(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    resource_index: u64,
    evidence_index: u64,
    out_kind: *mut u32,
    out_detail: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized before delegating to checked helpers.
    unsafe {
        initialize_value(out_kind, 0);
        initialize_const_output(out_detail);
        ffi_call(out_error, || {
            let resource =
                resource_resolution_at(resolutions, representation_index, resource_index)?;
            write_evidence(&resource.evidence, evidence_index, out_kind, out_detail)
        })
    }
}

/// Reads candidate-level evidence.
///
/// # Safety
///
/// `resolutions` must be live. Outputs must be writable, and `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_get_candidate_evidence(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    resource_index: u64,
    candidate_index: u64,
    evidence_index: u64,
    out_kind: *mut u32,
    out_detail: *mut *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized before delegating to checked helpers.
    unsafe {
        initialize_value(out_kind, 0);
        initialize_const_output(out_detail);
        ffi_call(out_error, || {
            let resource =
                resource_resolution_at(resolutions, representation_index, resource_index)?;
            let candidate = item_at(&resource.candidates, candidate_index, "candidate")?;
            write_evidence(&candidate.evidence, evidence_index, out_kind, out_detail)
        })
    }
}

/// Releases a resolution set. Passing null is a no-op.
///
/// # Safety
///
/// A non-null pointer must have been returned by this library and not previously
/// released. No borrowed strings may be used after this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_release(resolutions: *mut PpResolutionSet) {
    if resolutions.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership of a live allocation is required by this function's
        // contract and is reconstructed exactly once here.
        drop(unsafe { Box::from_raw(resolutions) });
    }));
}

/// Begins an explicit transaction that stages mutations until commit.
///
/// At most one transaction may be open for a production state. The returned handle
/// keeps that state alive even if the original production handle is released.
///
/// # Safety
///
/// `production` must be a live handle returned by this library. `out_transaction`
/// must be writable. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_begin_transaction(
    production: *mut PpProduction,
    out_transaction: *mut *mut PpTransaction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller contract for each pointer is documented above. Outputs
    // are initialized before validation and the production is borrowed only here.
    unsafe {
        initialize_output(out_transaction);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            if out_transaction.is_null() {
                return Err(invalid_argument("out_transaction must not be null"));
            }
            out_transaction.write(begin_transaction_handle(&production.state, None, None)?);
            Ok(())
        })
    }
}

/// Begins a transaction whose decisions were made from `base_revision`.
///
/// # Safety
///
/// `production` must be live, `out_transaction` writable, and `out_error` null
/// or writable. Base revision identity is a copied stack value.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_begin_transaction_at(
    production: *mut PpProduction,
    base_revision: PpRevisionId,
    out_transaction: *mut *mut PpTransaction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and all pointers checked before use.
    unsafe {
        initialize_output(out_transaction);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_transaction, "out_transaction")?;
            let base_revision = RevisionId::from_bytes(base_revision.bytes);
            out_transaction.write(begin_transaction_handle(
                &production.state,
                Some(base_revision),
                None,
            )?);
            Ok(())
        })
    }
}

/// Sets the origin and message attached to this transaction's future revision.
///
/// `origin_name` and `message` may be null. Origin version and URI may be null,
/// but require a non-null origin name when supplied.
///
/// # Safety
///
/// `transaction` must be live. Every non-null string must be NUL-terminated
/// UTF-8 for the duration of the call. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_set_revision_context(
    transaction: *mut PpTransaction,
    origin_name: *const c_char,
    origin_version: *const c_char,
    origin_uri: *const c_char,
    message: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated and copied before this call returns.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let origin_name = optional_utf8(origin_name, "origin_name")?;
            let origin_version = optional_utf8(origin_version, "origin_version")?;
            let origin_uri = optional_utf8(origin_uri, "origin_uri")?;
            let origin = match origin_name {
                Some(name) => Some(OriginIdentity::new(
                    name,
                    origin_version.map(str::to_owned),
                    origin_uri.map(str::to_owned),
                )?),
                None if origin_version.is_none() && origin_uri.is_none() => None,
                None => {
                    return Err(invalid_argument(
                        "origin_version and origin_uri require origin_name",
                    ));
                }
            };
            transaction.revision_context = RevisionContext::new(
                origin,
                optional_utf8(message, "message")?.map(str::to_owned),
            )?;
            Ok(())
        })
    }
}

/// Stages an original-media import and returns its stable asset identity.
///
/// The new asset's original representation has the source's content
/// structure. The source is borrowed only for this call and may be reused.
///
/// # Safety
///
/// `transaction` must be a live transaction handle and `source` a live media
/// source. `display_name` may be null or borrowed NUL-terminated UTF-8.
/// `out_asset_id` must be writable. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_import_media(
    transaction: *mut PpTransaction,
    source: *const PpMediaSource,
    display_name: *const c_char,
    out_asset_id: *mut PpAssetId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference and borrowed
    // inputs are not retained after this call.
    unsafe {
        initialize_value(out_asset_id, PpAssetId { bytes: [0; 16] });
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let source = media_source::borrowed_source(source)?;
            if out_asset_id.is_null() {
                return Err(invalid_argument("out_asset_id must not be null"));
            }
            let display_name = optional_utf8(display_name, "display_name")?.map(str::to_owned);
            let import = prepare_original_media(source.clone(), display_name, None)?;
            out_asset_id.write(PpAssetId {
                bytes: import.asset().id().into_bytes(),
            });
            transaction.mutations.push(StagedMutation::Import(import));
            Ok(())
        })
    }
}

/// Stages a representation of the given kind for an existing asset.
///
/// The representation has the source's content structure. The source is
/// borrowed only for this call and may be reused.
///
/// # Safety
///
/// `transaction` must be a live transaction handle and
/// `source` a live media source. `out_representation_id` must be writable;
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_add_representation(
    transaction: *mut PpTransaction,
    asset_id: PpAssetId,
    kind: u32,
    source: *const PpMediaSource,
    out_representation_id: *mut PpRepresentationId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference and borrowed
    // inputs are not retained after this call.
    unsafe {
        initialize_value(out_representation_id, PpRepresentationId { bytes: [0; 16] });
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let source = media_source::borrowed_source(source)?;
            if out_representation_id.is_null() {
                return Err(invalid_argument("out_representation_id must not be null"));
            }
            let import = prepare_representation(
                AssetId::from_bytes(asset_id.bytes),
                representation_kind_from_abi(kind)?,
                source.clone(),
            )?;
            out_representation_id.write(PpRepresentationId {
                bytes: import.representation().id().into_bytes(),
            });
            transaction
                .mutations
                .push(StagedMutation::Representation(import));
            Ok(())
        })
    }
}

/// Stages a filesystem media root and returns its stable identity.
///
/// # Safety
///
/// `transaction` must be a live transaction handle. `path` must be a borrowed
/// NUL-terminated UTF-8 string; `label` may be null or satisfy the same rule.
/// `out_root_id` must be writable. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_add_media_root(
    transaction: *mut PpTransaction,
    name: *const c_char,
    label: *const c_char,
    priority: i32,
    out_root_id: *mut PpMediaRootId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference and string inputs
    // follow the documented borrowed NUL-terminated contract.
    unsafe {
        initialize_value(out_root_id, PpMediaRootId { bytes: [0; 16] });
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            if out_root_id.is_null() {
                return Err(invalid_argument("out_root_id must not be null"));
            }
            let name = required_utf8(name, "name")?;
            let label = optional_utf8(label, "label")?.map(str::to_owned);
            let root = MediaRoot::new(MediaRootId::new(), name, label, None, priority, true)?;
            out_root_id.write(PpMediaRootId {
                bytes: root.id().into_bytes(),
            });
            transaction.mutations.push(StagedMutation::MediaRoot(root));
            Ok(())
        })
    }
}

/// Stages enabling or disabling one configured media root.
///
/// Requires a decision base; early rejection leaves the transaction open.
/// Reapplying the current state is an idempotent no-op at commit.
///
/// # Safety
///
/// `transaction` must be live, `enabled` exactly zero or
/// one, and `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_set_media_root_enabled(
    transaction: *mut PpTransaction,
    root_id: PpMediaRootId,
    enabled: u8,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before staging copied values.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.require_decision_base()?;
            let enabled = match enabled {
                0 => false,
                1 => true,
                _ => return Err(invalid_argument("enabled must be zero or one")),
            };
            transaction
                .mutations
                .push(StagedMutation::SetMediaRootEnabled(
                    MediaRootId::from_bytes(root_id.bytes),
                    enabled,
                ));
            Ok(())
        })
    }
}

/// Stages removal of one configured media root.
/// Requires a decision base; early rejection leaves the transaction open.
///
/// # Safety
///
/// `transaction` must be live and `out_error` null or
/// writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_remove_media_root(
    transaction: *mut PpTransaction,
    root_id: PpMediaRootId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before staging copied values.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.require_decision_base()?;
            transaction
                .mutations
                .push(StagedMutation::RemoveMediaRoot(MediaRootId::from_bytes(
                    root_id.bytes,
                )));
            Ok(())
        })
    }
}

/// Stages an explicitly confirmed URI for a resource.
///
/// The URI is borrowed UTF-8 without embedded NUL and must be absolute. A
/// non-null `root_name` records the logical media root the URI was found
/// under; it does not require a currently enabled mapping. `sequence_naming`
/// names the files at a locator of an image-sequence resource: it is required
/// for such a resource and rejected at commit for any other. Confirmation is
/// not durable until the transaction commits.
///
/// # Safety
///
/// `transaction` must be a live transaction handle, `resource_id` must be
/// readable, `uri` must be a NUL-terminated string, `root_name` null or
/// NUL-terminated, `sequence_naming` null or readable with NUL-terminated
/// strings, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_confirm_locator(
    transaction: *mut PpTransaction,
    resource_id: PpResourceId,
    uri: *const c_char,
    root_name: *const c_char,
    sequence_naming: *const PpSequenceNaming,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference and borrowed
    // strings follow the documented NUL-terminated contract.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let uri = required_utf8(uri, "uri")?;
            if uri.is_empty() {
                return Err(invalid_argument("uri must not be empty"));
            }
            let root_name = optional_utf8(root_name, "root_name")?;
            let naming = optional_naming(sequence_naming, "sequence_naming")?;
            let locator = prepare_confirmed_locator(
                ResourceId::from_bytes(resource_id.bytes),
                uri.to_owned(),
                root_name,
                naming,
            )?;
            transaction.mutations.push(StagedMutation::Locator(locator));
            Ok(())
        })
    }
}

/// Stages retirement of one superseded resource locator.
///
/// Requires a decision base. Rejection leaves the transaction open.
///
/// # Safety
///
/// `transaction` must be live, `out_error` null or
/// writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_retire_locator(
    transaction: *mut PpTransaction,
    locator_id: PpLocatorId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated before staging copied values.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            transaction.require_decision_base()?;
            transaction.mutations.push(StagedMutation::RetireLocator(
                postproject_core::LocatorId::from_bytes(locator_id.bytes),
            ));
            Ok(())
        })
    }
}

/// Stages a newly observed fingerprint for one resource.
///
/// The value is copied during this call. An identical current observation is a
/// successful no-op when the transaction commits.
/// Requires a decision base, including first and unchanged observations.
///
/// # Safety
///
/// `transaction` must be live, `algorithm` borrowed
/// NUL-terminated UTF-8, `value` readable for `value_length` bytes, and
/// `out_error` null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_record_resource_fingerprint(
    transaction: *mut PpTransaction,
    resource_id: PpResourceId,
    algorithm: *const c_char,
    version: u16,
    value: *const u8,
    value_length: u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated and copied before staging the mutation.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.require_decision_base()?;
            let fingerprint = ResourceFingerprint::new(
                required_utf8(algorithm, "algorithm")?,
                version,
                required_bytes(value, value_length, "value")?.to_vec(),
            )?;
            transaction
                .mutations
                .push(StagedMutation::RecordResourceFingerprint(
                    ResourceId::from_bytes(resource_id.bytes),
                    fingerprint,
                ));
            Ok(())
        })
    }
}

/// Stages a newly observed structure-aware fingerprint for one representation.
///
/// Pointer and no-op rules match
/// [`pp_transaction_record_resource_fingerprint`].
///
/// # Safety
///
/// All pointers follow the rules documented above, with `representation_id`
/// naming the representation to update.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_record_representation_fingerprint(
    transaction: *mut PpTransaction,
    representation_id: PpRepresentationId,
    algorithm: *const c_char,
    version: u16,
    value: *const u8,
    value_length: u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated and copied before staging the mutation.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.require_decision_base()?;
            let fingerprint = RepresentationFingerprint::new(
                required_utf8(algorithm, "algorithm")?,
                version,
                required_bytes(value, value_length, "value")?.to_vec(),
            )?;
            transaction
                .mutations
                .push(StagedMutation::RecordRepresentationFingerprint(
                    RepresentationId::from_bytes(representation_id.bytes),
                    fingerprint,
                ));
            Ok(())
        })
    }
}

/// Stages replacement of one representation's complete dependency observation.
///
/// Requires a decision base. Rejection leaves the transaction open.
///
/// The array and all strings are copied during this call. A null array is valid
/// only when `dependency_count` is zero.
///
/// # Safety
///
/// `transaction` and `representation_id` must be live, every dependency and
/// string must be readable for this call, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_record_dependency_set(
    transaction: *mut PpTransaction,
    representation_id: PpRepresentationId,
    dependencies: *const PpDependency,
    dependency_count: u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are validated and copied before staging the mutation.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            transaction.require_decision_base()?;
            let dependencies = dependencies_from_abi(dependencies, dependency_count)?;
            transaction
                .mutations
                .push(StagedMutation::RecordDependencySet(
                    RepresentationId::from_bytes(representation_id.bytes),
                    dependencies,
                ));
            Ok(())
        })
    }
}

/// Stages an external identifier attachment.
///
/// `qualifier` may be null; other string inputs are required borrowed
/// NUL-terminated UTF-8. The target is validated when the transaction commits.
///
/// # Safety
///
/// `transaction` must be live, `target` readable, string pointers must satisfy
/// the rules above, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_add_external_identifier(
    transaction: *mut PpTransaction,
    target: *const PpObjectRef,
    scheme: *const c_char,
    value: *const c_char,
    qualifier: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked before dereference and borrowed only for this call.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let target = target
                .as_ref()
                .ok_or_else(|| invalid_argument("target must not be null"))?;
            let target = object_ref_from_abi(*target)?;
            let identifier = external_identifier_from_abi(scheme, value, qualifier)?;
            transaction
                .mutations
                .push(StagedMutation::AddExternalIdentifier(target, identifier));
            Ok(())
        })
    }
}

/// Stages removal of one exact external identifier attachment.
///
/// Requires a decision base. Rejection leaves the transaction open.
///
/// # Safety
///
/// The pointer and UTF-8 contracts are identical to
/// [`pp_transaction_add_external_identifier`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_remove_external_identifier(
    transaction: *mut PpTransaction,
    target: *const PpObjectRef,
    scheme: *const c_char,
    value: *const c_char,
    qualifier: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked before dereference and borrowed only for this call.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            transaction.require_decision_base()?;
            let target = target
                .as_ref()
                .ok_or_else(|| invalid_argument("target must not be null"))?;
            let target = object_ref_from_abi(*target)?;
            let identifier = external_identifier_from_abi(scheme, value, qualifier)?;
            transaction
                .mutations
                .push(StagedMutation::RemoveExternalIdentifier(target, identifier));
            Ok(())
        })
    }
}

/// Stages one typed metadata assertion from an owned metadata input.
///
/// The input remains owned by the caller and may be released immediately after
/// this call. The target is validated at commit.
///
/// # Safety
///
/// `transaction`, `target`, and `input` must be live; vocabulary/property must
/// be borrowed NUL-terminated UTF-8; and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_add_metadata_value(
    transaction: *mut PpTransaction,
    target: *const PpObjectRef,
    vocabulary: *const c_char,
    property: *const c_char,
    input: *const PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked before dereference and borrowed only this call.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let target = target
                .as_ref()
                .ok_or_else(|| invalid_argument("target must not be null"))?;
            let input = input
                .as_ref()
                .ok_or_else(|| invalid_argument("input must not be null"))?;
            let target = object_ref_from_abi(*target)?;
            let property = metadata_property_from_abi(vocabulary, property)?;
            transaction.mutations.push(StagedMutation::AddMetadataValue(
                target,
                property,
                input.value.clone(),
            ));
            Ok(())
        })
    }
}

/// Stages removal of every value of one metadata property.
/// Requires a read-bound edit or explicitly based transaction. Rejection before
/// staging leaves the transaction open; actual commit checks the base again.
///
/// # Safety
///
/// `transaction` must be live, `target` readable, vocabulary/property must be
/// borrowed NUL-terminated UTF-8, and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_remove_metadata_property(
    transaction: *mut PpTransaction,
    target: *const PpObjectRef,
    vocabulary: *const c_char,
    property: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked before dereference and borrowed only this call.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.require_decision_base()?;
            let target = target
                .as_ref()
                .ok_or_else(|| invalid_argument("target must not be null"))?;
            let target = object_ref_from_abi(*target)?;
            let property = metadata_property_from_abi(vocabulary, property)?;
            transaction
                .mutations
                .push(StagedMutation::RemoveMetadataProperty(target, property));
            Ok(())
        })
    }
}

/// Stages one durable requested job and returns its new identity.
///
/// The input ID array and strings are borrowed only for this call. A null
/// `target_root` means no preferred logical output root.
///
/// # Safety
///
/// `transaction` must be live and `kind` must be readable,
/// the input array must contain `input_count` readable UUIDs (or be null when
/// the count is zero), `out_job_id` must be writable, and `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(
    clippy::too_many_arguments,
    reason = "the C ABI keeps the requested output fields explicit"
)]
pub unsafe extern "C" fn pp_transaction_request_job(
    transaction: *mut PpTransaction,
    kind: *const c_char,
    input_representation_ids: *const PpRepresentationId,
    input_count: u64,
    output_asset_id: PpAssetId,
    output_representation_kind: u32,
    target_root: *const c_char,
    out_job_id: *mut PpJobId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked and copied before this call returns.
    unsafe {
        initialize_value(out_job_id, PpJobId { bytes: [0; 16] });
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            require_output(out_job_id, "out_job_id")?;
            let input_count = usize::try_from(input_count)
                .map_err(|_| invalid_argument("job input count is too large"))?;
            if input_count > MAX_JOB_INPUTS {
                return Err(invalid_argument(format!(
                    "job input count must not exceed {MAX_JOB_INPUTS}"
                )));
            }
            let inputs = if input_count == 0 {
                Vec::new()
            } else {
                if input_representation_ids.is_null() {
                    return Err(invalid_argument(
                        "input_representation_ids must not be null when count is nonzero",
                    ));
                }
                // SAFETY: The caller guarantees `input_count` readable UUIDs.
                std::slice::from_raw_parts(input_representation_ids, input_count)
                    .iter()
                    .map(|id| RepresentationId::from_bytes(id.bytes))
                    .collect()
            };
            let job = Job::new(
                JobId::new(),
                JobKind::new(required_utf8(kind, "kind")?)?,
                inputs,
                RequestedJobOutput::new(
                    AssetId::from_bytes(output_asset_id.bytes),
                    representation_kind_from_abi(output_representation_kind)?,
                    optional_utf8(target_root, "target_root")?.map(str::to_owned),
                )?,
            )?;
            out_job_id.write(PpJobId {
                bytes: job.id().into_bytes(),
            });
            transaction.mutations.push(StagedMutation::RequestJob(job));
            Ok(())
        })
    }
}

/// Stages an atomic claim and returns its capability token.
///
/// The token becomes usable only after this transaction commits successfully.
/// All strings are borrowed only for this call.
///
/// # Safety
///
/// `transaction` must be live, `tool_name` valid UTF-8,
/// `out_claim_id` writable, and `out_error` null or writable. Other strings
/// may be null or must follow the same UTF-8 contract.
#[postproject_ffi_macros::ffi_export]
#[allow(
    clippy::too_many_arguments,
    reason = "the C ABI keeps worker attribution fields explicit"
)]
pub unsafe extern "C" fn pp_transaction_claim_job(
    transaction: *mut PpTransaction,
    job_id: PpJobId,
    tool_name: *const c_char,
    tool_version: *const c_char,
    tool_uri: *const c_char,
    agent_name: *const c_char,
    agent_identifier_scheme: *const c_char,
    agent_identifier_value: *const c_char,
    agent_identifier_qualifier: *const c_char,
    now_unix_micros: i64,
    expires_at_unix_micros: i64,
    out_claim_id: *mut PpUuid,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked and copied before this call returns.
    unsafe {
        initialize_uuid(out_claim_id);
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            require_output(out_claim_id, "out_claim_id")?;
            let (tool, agent) = worker_identity_from_abi(
                tool_name,
                tool_version,
                tool_uri,
                agent_name,
                agent_identifier_scheme,
                agent_identifier_value,
                agent_identifier_qualifier,
            )?;
            let claim_id = JobClaimId::new();
            out_claim_id.write(PpUuid {
                bytes: claim_id.into_bytes(),
            });
            transaction.mutations.push(StagedMutation::ClaimJob {
                job_id: JobId::from_bytes(job_id.bytes),
                claim_id,
                tool,
                agent,
                now: Timestamp::from_unix_micros(now_unix_micros),
                expires_at: Timestamp::from_unix_micros(expires_at_unix_micros),
            });
            Ok(())
        })
    }
}

/// Stages renewal of an active job claim.
///
/// # Safety
///
/// The transaction must be live; both IDs must be readable; `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_renew_job_claim(
    transaction: *mut PpTransaction,
    job_id: PpJobId,
    claim_id: *const PpUuid,
    now_unix_micros: i64,
    expires_at_unix_micros: i64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: IDs are checked before dereference and copied immediately.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let (job_id, claim_id) = required_job_claim_ids(job_id, claim_id)?;
            transaction.mutations.push(StagedMutation::RenewJobClaim(
                job_id,
                claim_id,
                Timestamp::from_unix_micros(now_unix_micros),
                Timestamp::from_unix_micros(expires_at_unix_micros),
            ));
            Ok(())
        })
    }
}

/// Stages release of an active job claim.
///
/// # Safety
///
/// The transaction must be live; both IDs must be readable; `out_error` may be
/// null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_release_job_claim(
    transaction: *mut PpTransaction,
    job_id: PpJobId,
    claim_id: *const PpUuid,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: IDs are checked before dereference and copied immediately.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let (job_id, claim_id) = required_job_claim_ids(job_id, claim_id)?;
            transaction
                .mutations
                .push(StagedMutation::ReleaseJobClaim(job_id, claim_id));
            Ok(())
        })
    }
}

/// Binds a staged representation and activity into one atomic job completion.
///
/// The representation must have been staged before the activity in this same
/// transaction. Neither fact is persisted separately if completion fails.
///
/// # Safety
///
/// The transaction must be live; claim/output IDs must be readable; `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_complete_job(
    transaction: *mut PpTransaction,
    job_id: PpJobId,
    claim_id: *const PpUuid,
    now_unix_micros: i64,
    output_representation_id: PpRepresentationId,
    activity_id: PpActivityId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: IDs are checked before dereference and copied immediately.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let (job_id, claim_id) = required_job_claim_ids(job_id, claim_id)?;
            let output_id = output_representation_id;
            let output_id = RepresentationId::from_bytes(output_id.bytes);
            let activity_id = ActivityId::from_bytes(activity_id.bytes);
            let output_index = transaction
                .mutations
                .iter()
                .position(|mutation| {
                    matches!(
                        mutation,
                        StagedMutation::Representation(output)
                            if output.representation().id() == output_id
                    )
                })
                .ok_or_else(|| {
                    invalid_argument("output representation is not staged in this transaction")
                })?;
            let activity_index = transaction
                .mutations
                .iter()
                .position(|mutation| {
                    matches!(
                        mutation,
                        StagedMutation::Activity(activity) if activity.id() == activity_id
                    )
                })
                .ok_or_else(|| invalid_argument("activity is not staged in this transaction"))?;
            if output_index >= activity_index {
                return Err(invalid_argument(
                    "job output representation must be staged before its activity",
                ));
            }
            let StagedMutation::Activity(activity) = transaction.mutations.remove(activity_index)
            else {
                unreachable!("activity index was selected by its mutation variant");
            };
            let StagedMutation::Representation(output) = transaction.mutations.remove(output_index)
            else {
                unreachable!("output index was selected by its mutation variant");
            };
            transaction.mutations.insert(
                output_index,
                StagedMutation::CompleteJob {
                    job_id,
                    claim_id,
                    now: Timestamp::from_unix_micros(now_unix_micros),
                    output,
                    activity: Box::new(activity),
                },
            );
            Ok(())
        })
    }
}

/// Stages failure of an active unexpired job claim.
///
/// # Safety
///
/// The transaction must be live; both IDs and `diagnostic` must be readable;
/// `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_fail_job(
    transaction: *mut PpTransaction,
    job_id: PpJobId,
    claim_id: *const PpUuid,
    now_unix_micros: i64,
    diagnostic: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked and copied before this call returns.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let (job_id, claim_id) = required_job_claim_ids(job_id, claim_id)?;
            transaction.mutations.push(StagedMutation::FailJob(
                job_id,
                claim_id,
                Timestamp::from_unix_micros(now_unix_micros),
                JobFailure::new(required_utf8(diagnostic, "diagnostic")?)?,
            ));
            Ok(())
        })
    }
}

/// Stages administrative cancellation of a requested or claimed job.
///
/// # Safety
///
/// The transaction must be live, and `out_error` null or
/// writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_cancel_job(
    transaction: *mut PpTransaction,
    job_id: PpJobId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The job ID is checked before dereference and copied immediately.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            transaction
                .mutations
                .push(StagedMutation::CancelJob(JobId::from_bytes(job_id.bytes)));
            Ok(())
        })
    }
}

/// Stages one complete production activity.
///
/// Input and output arrays are borrowed only for this call. Edge roles, kind,
/// tool fields, and agent fields are NUL-terminated UTF-8. Optional values may
/// be null. Agent identifier scheme and value must be supplied together.
///
/// # Safety
///
/// `transaction` must be live, `kind` and non-empty edge arrays must be
/// readable, `out_activity_id` must be writable, and every non-null string must
/// remain valid for the call. `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
#[allow(
    clippy::too_many_arguments,
    reason = "the C ABI keeps optional provenance fields explicit"
)]
pub unsafe extern "C" fn pp_transaction_create_activity(
    transaction: *mut PpTransaction,
    kind: *const c_char,
    inputs: *const PpActivityEdge,
    input_count: u64,
    outputs: *const PpActivityEdge,
    output_count: u64,
    started_at_unix_micros: *const i64,
    finished_at_unix_micros: *const i64,
    tool_name: *const c_char,
    tool_version: *const c_char,
    tool_uri: *const c_char,
    agent_name: *const c_char,
    agent_identifier_scheme: *const c_char,
    agent_identifier_value: *const c_char,
    agent_identifier_qualifier: *const c_char,
    out_activity_id: *mut PpActivityId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Inputs are checked and converted to owned domain values before return.
    unsafe {
        initialize_value(out_activity_id, PpActivityId { bytes: [0; 16] });
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            require_output(out_activity_id, "out_activity_id")?;
            let kind = ActivityKind::new(required_utf8(kind, "kind")?)?;
            let inputs = activity_edges_from_abi(inputs, input_count, "input")?
                .into_iter()
                .map(|(representation_id, role)| ActivityInput::new(representation_id, role))
                .collect();
            let outputs = activity_edges_from_abi(outputs, output_count, "output")?
                .into_iter()
                .map(|(representation_id, role)| ActivityOutput::new(representation_id, role))
                .collect();
            let mut activity = Activity::new(ActivityId::new(), kind, inputs, outputs)?
                .with_timing(
                    started_at_unix_micros
                        .as_ref()
                        .copied()
                        .map(Timestamp::from_unix_micros),
                    finished_at_unix_micros
                        .as_ref()
                        .copied()
                        .map(Timestamp::from_unix_micros),
                )?;
            let tool_name = optional_utf8(tool_name, "tool_name")?;
            let tool_version = optional_utf8(tool_version, "tool_version")?;
            let tool_uri = optional_utf8(tool_uri, "tool_uri")?;
            if let Some(name) = tool_name {
                activity = activity.with_tool(ToolIdentity::new(
                    name,
                    tool_version.map(str::to_owned),
                    tool_uri.map(str::to_owned),
                )?);
            } else if tool_version.is_some() || tool_uri.is_some() {
                return Err(invalid_argument("tool version and URI require a tool name"));
            }
            let agent_name = optional_utf8(agent_name, "agent_name")?;
            let agent_scheme = optional_utf8(agent_identifier_scheme, "agent_identifier_scheme")?;
            let agent_value = optional_utf8(agent_identifier_value, "agent_identifier_value")?;
            let agent_qualifier =
                optional_utf8(agent_identifier_qualifier, "agent_identifier_qualifier")?;
            let agent_identifier = match (agent_scheme, agent_value) {
                (Some(scheme), Some(value)) => Some(ExternalIdentifier::new(
                    IdentifierScheme::new(scheme)?,
                    value,
                    agent_qualifier.map(str::to_owned),
                )?),
                (None, None) if agent_qualifier.is_none() => None,
                _ => {
                    return Err(invalid_argument(
                        "agent identifier scheme and value must be supplied together",
                    ));
                }
            };
            if agent_name.is_some() || agent_identifier.is_some() {
                activity = activity.with_agent(AgentIdentity::new(
                    agent_name.map(str::to_owned),
                    agent_identifier,
                )?);
            }
            out_activity_id.write(PpActivityId {
                bytes: activity.id().into_bytes(),
            });
            transaction
                .mutations
                .push(StagedMutation::Activity(activity));
            Ok(())
        })
    }
}

/// Atomically commits all staged transaction mutations.
///
/// # Safety
///
/// `transaction` must be a live transaction handle and `out_error` may be null
/// or writable. A closed transaction remains valid only for release.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_commit(
    transaction: *mut PpTransaction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The non-null transaction is required to be live and exclusively
    // accessed by the caller for this operation.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.commit().map(|_| ())
        })
    }
}

/// Commits and returns the production and revision from the atomic write path.
///
/// # Safety
///
/// `transaction` must be live and exclusively accessed, `out_receipt` writable,
/// and `out_error` null or writable. Missing outputs reject before commit.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_commit_with_receipt(
    transaction: *mut PpTransaction,
    out_receipt: *mut PpCommitReceipt,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointer validity and exclusive access are caller requirements.
    unsafe {
        initialize_value(
            out_receipt,
            PpCommitReceipt {
                production_id: PpProductionId { bytes: [0; 16] },
                outcome: 0,
                revision_id: PpRevisionId { bytes: [0; 16] },
                revision_sequence: 0,
            },
        );
        ffi_call(out_error, || {
            require_output(out_receipt, "out_receipt")?;
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            let receipt = transaction.commit()?;
            out_receipt.write(PpCommitReceipt::from(&receipt));
            Ok(())
        })
    }
}

/// Discards all staged transaction mutations.
///
/// # Safety
///
/// `transaction` must be a live transaction handle and `out_error` may be null
/// or writable. A closed transaction remains valid only for release.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_rollback(
    transaction: *mut PpTransaction,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The non-null transaction is required to be live and exclusively
    // accessed by the caller for this operation.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.rollback()
        })
    }
}

/// Releases a transaction, implicitly discarding staged work when still open.
/// Passing null is a no-op.
///
/// # Safety
///
/// A non-null pointer must have been returned by this library and not previously
/// released. No other thread may use it during or after this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_release(transaction: *mut PpTransaction) {
    if transaction.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership of a live allocation is required by this function's
        // contract and is reconstructed exactly once here.
        drop(unsafe { Box::from_raw(transaction) });
    }));
}

/// Releases a production handle. Passing null is a no-op.
///
/// # Safety
///
/// A non-null pointer must have been returned by this library and not previously
/// released. No other thread may use it during or after this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_release(production: *mut PpProduction) {
    if production.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership of a live allocation is required by this function's
        // contract and is reconstructed exactly once here.
        drop(unsafe { Box::from_raw(production) });
    }));
}

/// Returns an error object's stable numeric code.
///
/// # Safety
///
/// `error` must be null or a live error returned by this library.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_error_code(error: *const PpError) -> u32 {
    // SAFETY: A non-null pointer is required to reference a live error by the
    // caller contract and is only borrowed for this call.
    unsafe { error.as_ref().map_or(PP_OK, |error| error.code) }
}

/// Returns a borrowed NUL-terminated UTF-8 error message.
///
/// The pointer remains valid until the error is released. Null input returns
/// null.
///
/// # Safety
///
/// `error` must be null or a live error returned by this library.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_error_message(error: *const PpError) -> *const c_char {
    // SAFETY: A non-null pointer is required to reference a live error by the
    // caller contract and is only borrowed for this call.
    unsafe {
        error
            .as_ref()
            .map_or(ptr::null(), |error| error.message.as_ptr())
    }
}

/// Copies borrowed structured transaction-conflict detail from an error.
///
/// Returns one when detail is present and zero otherwise. String pointers in
/// the output remain valid until `error` is released.
///
/// # Safety
///
/// `error` must be null or live. `out_conflict` must be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_error_transaction_conflict(
    error: *const PpError,
    out_conflict: *mut PpTransactionConflict,
) -> u8 {
    if out_conflict.is_null() {
        return 0;
    }
    // SAFETY: The caller contract requires a non-null output to be writable
    // and a non-null error to reference a live error object.
    unsafe {
        out_conflict.write(empty_transaction_conflict());
        let Some(conflict) = error
            .as_ref()
            .and_then(|error| error.transaction_conflict.as_ref())
        else {
            return 0;
        };
        out_conflict.write(conflict.borrowed());
    }
    1
}

/// Releases an error object. Passing null is a no-op.
///
/// # Safety
///
/// A non-null pointer must have been returned by this library and not previously
/// released.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_error_release(error: *mut PpError) {
    if error.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership of a live allocation is required by this function's
        // contract and is reconstructed exactly once here.
        drop(unsafe { Box::from_raw(error) });
    }));
}

unsafe fn ffi_call(
    out_error: *mut *mut PpError,
    operation: impl FnOnce() -> Result<(), Error>,
) -> u32 {
    // SAFETY: The caller of this helper passes through the exported function's
    // documented optional writable error pointer.
    unsafe { initialize_output(out_error) };
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => PP_OK,
        Ok(Err(error)) => {
            let code = error_code(error.kind());
            let message = error.to_string();
            let transaction_conflict = error
                .transaction_conflict_detail()
                .and_then(AbiTransactionConflict::from_domain);
            // SAFETY: Same output-pointer contract as above.
            unsafe { write_error(out_error, code, &message, transaction_conflict) };
            code
        }
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            // SAFETY: Same output-pointer contract as above.
            unsafe { write_error(out_error, PP_ERROR_INTERNAL, &message, None) };
            PP_ERROR_INTERNAL
        }
    }
}

#[derive(Clone, Copy)]
enum ActivityRelation {
    Producing,
    Consuming,
}

unsafe fn production_activities_for_representation(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    relation: ActivityRelation,
    out_activities: *mut *mut PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and all pointers checked before use.
    unsafe {
        initialize_output(out_activities);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_activities, "out_activities")?;
            let inner = lock_production(&production.state);
            let representation_id = RepresentationId::from_bytes(representation_id.bytes);
            let activities = match relation {
                ActivityRelation::Producing => inner.activities_producing(representation_id),
                ActivityRelation::Consuming => inner.activities_consuming(representation_id),
            }?;
            out_activities.write(Box::into_raw(Box::new(PpActivitySet::new(&activities)?)));
            Ok(())
        })
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the C ABI carries explicit page inputs and result outputs"
)]
unsafe fn production_activities_page(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    limit: u32,
    cursor: *const c_char,
    producing: bool,
    out_activities: *mut *mut PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and all pointers checked before use.
    unsafe {
        initialize_output(out_activities);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_activities, "out_activities")?;
            let request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let representation_id = RepresentationId::from_bytes(representation_id.bytes);
            let page = if producing {
                inner.activities_producing_page(representation_id, &request)
            } else {
                inner.activities_consuming_page(representation_id, &request)
            }?;
            out_activities.write(Box::into_raw(Box::new(PpActivitySet::new_page(
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

#[derive(Clone, Copy)]
enum ProvenanceDirection {
    Ancestors,
    Descendants,
}

unsafe fn production_provenance_relatives(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    direction: ProvenanceDirection,
    out_representations: *mut *mut PpObjectRefSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and all pointers checked before use.
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let inner = lock_production(&production.state);
            let representation_id = RepresentationId::from_bytes(representation_id.bytes);
            let related = match direction {
                ProvenanceDirection::Ancestors => inner.ancestors(representation_id),
                ProvenanceDirection::Descendants => inner.descendants(representation_id),
            }?;
            let objects = related
                .into_iter()
                .map(|id| PpObjectRef {
                    kind: PP_OBJECT_REPRESENTATION,
                    id: PpUuid {
                        bytes: id.into_bytes(),
                    },
                })
                .collect();
            out_representations.write(Box::into_raw(Box::new(PpObjectRefSet { objects })));
            Ok(())
        })
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the C ABI carries explicit traversal bounds and page outputs"
)]
unsafe fn production_provenance_page(
    production: *const PpProduction,
    representation_id: PpRepresentationId,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    ancestors: bool,
    out_objects: *mut *mut PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and all pointers checked before use.
    unsafe {
        initialize_output(out_objects);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_objects, "out_objects")?;
            let limits = ProvenanceQueryLimits::new(max_depth, max_representations)?;
            let request = query_page_request(limit, cursor)?;
            let inner = lock_production(&production.state);
            let representation_id = RepresentationId::from_bytes(representation_id.bytes);
            let page = if ancestors {
                inner.ancestors_page(representation_id, limits, &request)
            } else {
                inner.descendants_page(representation_id, limits, &request)
            }?;
            out_objects.write(Box::into_raw(Box::new(object_query_set(
                page.items().iter().map(|matched: &ProvenanceQueryMatch| {
                    (
                        ObjectRef::Representation(matched.representation_id()),
                        matched.depth(),
                    )
                }),
                page.next_cursor(),
                page.traversal_truncated(),
            )?)));
            Ok(())
        })
    }
}

unsafe fn initialize_output<T>(output: *mut *mut T) {
    if !output.is_null() {
        // SAFETY: Non-null output pointers are required to be writable by every
        // exported caller contract using this helper.
        unsafe { output.write(ptr::null_mut()) };
    }
}

unsafe fn initialize_uuid(output: *mut PpUuid) {
    if !output.is_null() {
        // SAFETY: Non-null UUID output pointers are required to be writable by
        // every exported caller contract using this helper.
        unsafe { output.write(PpUuid { bytes: [0; 16] }) };
    }
}

unsafe fn initialize_object_ref(output: *mut PpObjectRef) {
    if !output.is_null() {
        // SAFETY: Non-null outputs are writable by the exported caller contract.
        unsafe {
            output.write(PpObjectRef {
                kind: 0,
                id: PpUuid { bytes: [0; 16] },
            });
        }
    }
}

unsafe fn initialize_value<T: Copy>(output: *mut T, value: T) {
    if !output.is_null() {
        // SAFETY: Non-null output pointers are required to be writable by every
        // exported caller contract using this helper.
        unsafe { output.write(value) };
    }
}

unsafe fn initialize_const_output<T>(output: *mut *const T) {
    if !output.is_null() {
        // SAFETY: Non-null output pointers are required to be writable by every
        // exported caller contract using this helper.
        unsafe { output.write(ptr::null()) };
    }
}

const fn empty_revision_event() -> PpRevisionEvent {
    PpRevisionEvent {
        kind: 0,
        position: 0,
        asset_id: PpAssetId { bytes: [0; 16] },
        representation_id: PpRepresentationId { bytes: [0; 16] },
        resource_id: PpResourceId { bytes: [0; 16] },
        locator_id: PpLocatorId { bytes: [0; 16] },
        media_root_id: PpMediaRootId { bytes: [0; 16] },
        activity_id: PpActivityId { bytes: [0; 16] },
        job_id: PpJobId { bytes: [0; 16] },
        target: PpObjectRef {
            kind: 0,
            id: PpUuid { bytes: [0; 16] },
        },
        structural_position: 0,
        enabled: 0,
        identifier_scheme: ptr::null(),
        identifier_value: ptr::null(),
        identifier_qualifier: ptr::null(),
        vocabulary: ptr::null(),
        property: ptr::null(),
        activity_kind: ptr::null(),
        role: ptr::null(),
        fingerprint_algorithm: ptr::null(),
        fingerprint_version: 0,
    }
}

const fn zero_artifact_reason() -> PpArtifactReason {
    PpArtifactReason {
        kind: 0,
        activity_id: PpActivityId { bytes: [0; 16] },
        representation_id: PpRepresentationId { bytes: [0; 16] },
        input_representation_id: PpRepresentationId { bytes: [0; 16] },
        edge_kind: 0,
        upstream_state: 0,
        traversal_limit: 0,
        activity_count: 0,
        dependency_issue: 0,
        dependency_path: ptr::null(),
        dependency_path_length: 0,
        fingerprint_algorithm: ptr::null(),
        fingerprint_version: 0,
        has_snapshot_value: 0,
        snapshot_value: ptr::null(),
        snapshot_value_length: 0,
        has_current_value: 0,
        current_value: ptr::null(),
        current_value_length: 0,
    }
}

const fn zero_dependency() -> PpDependency {
    PpDependency {
        has_source_resource: 0,
        source_resource_id: PpResourceId { bytes: [0; 16] },
        kind: ptr::null(),
        target: PpObjectRef {
            kind: 0,
            id: PpUuid { bytes: [0; 16] },
        },
        has_resolved_representation: 0,
        resolved_representation_id: PpRepresentationId { bytes: [0; 16] },
        required: 0,
        authored_reference: ptr::null(),
    }
}

const fn zero_reproducibility_issue() -> PpArtifactReproducibilityIssue {
    PpArtifactReproducibilityIssue {
        kind: 0,
        activity_id: PpActivityId { bytes: [0; 16] },
        representation_id: PpRepresentationId { bytes: [0; 16] },
        activity_count: 0,
    }
}

fn require_output<T>(output: *mut T, label: &str) -> Result<(), Error> {
    if output.is_null() {
        Err(invalid_argument(format!("{label} must not be null")))
    } else {
        Ok(())
    }
}

unsafe fn write_evidence(
    evidence: &[AbiEvidence],
    evidence_index: u64,
    out_kind: *mut u32,
    out_detail: *mut *const c_char,
) -> Result<(), Error> {
    // SAFETY: Output validity is checked before either pointer is written.
    unsafe {
        initialize_value(out_kind, 0);
        initialize_const_output(out_detail);
        require_output(out_kind, "out_kind")?;
        require_output(out_detail, "out_detail")?;
        let evidence = item_at(evidence, evidence_index, "evidence")?;
        out_kind.write(evidence.kind);
        out_detail.write(
            evidence
                .detail
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
        );
        Ok(())
    }
}

unsafe fn write_activity_edge(
    edges: &[AbiActivityEdge],
    index: u64,
    out_representation_id: *mut PpRepresentationId,
    out_role: *mut *const c_char,
) -> Result<(), Error> {
    // SAFETY: Output validity is checked before either pointer is written.
    unsafe {
        initialize_value(out_representation_id, PpRepresentationId { bytes: [0; 16] });
        initialize_const_output(out_role);
        require_output(out_representation_id, "out_representation_id")?;
        require_output(out_role, "out_role")?;
        let edge = item_at(edges, index, "activity edge")?;
        out_representation_id.write(PpRepresentationId {
            bytes: edge.representation_id.into_bytes(),
        });
        out_role.write(
            edge.role
                .as_ref()
                .map_or(ptr::null(), |value| value.as_ptr()),
        );
        Ok(())
    }
}

unsafe fn write_activity_edge_snapshot(
    edges: &[AbiActivityEdge],
    index: u64,
    out_has_snapshot: *mut u8,
    out_revision_sequence: *mut u64,
    out_fingerprint_count: *mut u64,
) -> Result<(), Error> {
    // SAFETY: Output validity is checked before any pointer is written.
    unsafe {
        initialize_value(out_has_snapshot, 0);
        initialize_value(out_revision_sequence, 0);
        initialize_value(out_fingerprint_count, 0);
        require_output(out_has_snapshot, "out_has_snapshot")?;
        require_output(out_revision_sequence, "out_revision_sequence")?;
        require_output(out_fingerprint_count, "out_fingerprint_count")?;
        let edge = item_at(edges, index, "activity edge")?;
        if let Some(snapshot) = &edge.snapshot {
            out_has_snapshot.write(1);
            out_revision_sequence.write(snapshot.revision_sequence);
            out_fingerprint_count.write(length_as_u64(snapshot.fingerprints.len())?);
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments, reason = "flat C outputs are ABI-safe")]
unsafe fn write_snapshot_fingerprint(
    snapshot: Option<&AbiActivityEdgeSnapshot>,
    index: u64,
    out_algorithm: *mut *const c_char,
    out_version: *mut u16,
    out_value: *mut *const u8,
    out_value_length: *mut u64,
    out_has_observed_revision: *mut u8,
    out_observed_revision_sequence: *mut u64,
) -> Result<(), Error> {
    // SAFETY: Output validity is checked before any pointer is written.
    unsafe {
        initialize_const_output(out_algorithm);
        initialize_value(out_version, 0);
        initialize_const_output(out_value);
        initialize_value(out_value_length, 0);
        initialize_value(out_has_observed_revision, 0);
        initialize_value(out_observed_revision_sequence, 0);
        require_output(out_algorithm, "out_algorithm")?;
        require_output(out_version, "out_version")?;
        require_output(out_value, "out_value")?;
        require_output(out_value_length, "out_value_length")?;
        require_output(out_has_observed_revision, "out_has_observed_revision")?;
        require_output(
            out_observed_revision_sequence,
            "out_observed_revision_sequence",
        )?;
        let snapshot = snapshot
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "activity edge snapshot is absent"))?;
        let fingerprint: &AbiFingerprintSnapshot =
            item_at(&snapshot.fingerprints, index, "snapshot fingerprint")?;
        out_algorithm.write(fingerprint.algorithm.as_ptr());
        out_version.write(fingerprint.version);
        out_value.write(fingerprint.value.as_ptr());
        out_value_length.write(length_as_u64(fingerprint.value.len())?);
        if let Some(sequence) = fingerprint.observed_revision_sequence {
            out_has_observed_revision.write(1);
            out_observed_revision_sequence.write(sequence);
        }
        Ok(())
    }
}

unsafe fn write_error(
    output: *mut *mut PpError,
    code: u32,
    message: &str,
    transaction_conflict: Option<AbiTransactionConflict>,
) {
    if output.is_null() {
        return;
    }
    let message = CString::new(message.replace('\0', "�")).unwrap_or_default();
    let error = Box::new(PpError {
        code,
        message,
        transaction_conflict,
    });
    // SAFETY: Non-null output pointers are required to be writable by every
    // exported caller contract using this helper.
    unsafe { output.write(Box::into_raw(error)) };
}

unsafe fn required_utf8<'a>(value: *const c_char, label: &str) -> Result<&'a str, Error> {
    if value.is_null() {
        return Err(invalid_argument(format!("{label} must not be null")));
    }
    // SAFETY: The exported caller contract requires a NUL-terminated input that
    // remains valid throughout the call.
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map_err(|error| invalid_argument(format!("{label} must contain valid UTF-8: {error}")))
}

unsafe fn optional_utf8<'a>(value: *const c_char, label: &str) -> Result<Option<&'a str>, Error> {
    if value.is_null() {
        Ok(None)
    } else {
        // SAFETY: Non-null input has the same contract as `required_utf8`.
        unsafe { required_utf8(value, label) }.map(Some)
    }
}

unsafe fn query_page_request(limit: u32, cursor: *const c_char) -> Result<QueryPageRequest, Error> {
    // SAFETY: The caller of this helper carries the exported optional-string contract.
    let cursor = unsafe { optional_utf8(cursor, "query cursor") }?
        .map(QueryCursor::new)
        .transpose()?;
    QueryPageRequest::new(limit, cursor)
}

fn query_cursor_to_cstring(cursor: Option<&QueryCursor>) -> Result<Option<CString>, Error> {
    cursor
        .map(|cursor| exact_cstring(cursor.as_str(), "query cursor"))
        .transpose()
}

fn object_query_set(
    objects: impl IntoIterator<Item = (ObjectRef, u32)>,
    next_cursor: Option<&QueryCursor>,
    traversal_truncated: bool,
) -> Result<PpObjectQuerySet, Error> {
    Ok(PpObjectQuerySet {
        objects: objects
            .into_iter()
            .map(|(object, depth)| Ok((object_ref_to_abi(object)?, depth)))
            .collect::<Result<Vec<_>, Error>>()?,
        next_cursor: query_cursor_to_cstring(next_cursor)?,
        traversal_truncated,
    })
}

fn locator_query_set(
    locators: &[Locator],
    next_cursor: Option<&QueryCursor>,
) -> Result<PpLocatorQuerySet, Error> {
    Ok(PpLocatorQuerySet {
        locators: locators
            .iter()
            .map(|locator| {
                Ok(AbiQueryLocator {
                    id: locator.id(),
                    resource_id: locator.resource_id(),
                    uri: exact_cstring(locator.uri(), "locator URI")?,
                    availability: match locator.availability() {
                        LocatorAvailability::Unknown => 1,
                        LocatorAvailability::Online => 2,
                        LocatorAvailability::Offline => 3,
                        _ => 0,
                    },
                    last_seen: locator.last_seen().map(Timestamp::as_unix_micros),
                    media_root: locator
                        .media_root()
                        .map(|name| exact_cstring(name, "locator media root"))
                        .transpose()?,
                    sequence_naming: AbiSequenceNaming::optional(locator.sequence_naming())?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?,
        next_cursor: query_cursor_to_cstring(next_cursor)?,
    })
}

unsafe fn required_bytes<'a>(
    value: *const u8,
    length: u64,
    label: &str,
) -> Result<&'a [u8], Error> {
    let length = usize::try_from(length)
        .map_err(|_| invalid_argument(format!("{label} length is too large")))?;
    if value.is_null() {
        return Err(invalid_argument(format!("{label} must not be null")));
    }
    if length > isize::MAX as usize {
        return Err(invalid_argument(format!("{label} length is too large")));
    }
    // SAFETY: The caller guarantees `length` readable contiguous bytes.
    Ok(unsafe { std::slice::from_raw_parts(value, length) })
}

unsafe fn activity_edges_from_abi(
    edges: *const PpActivityEdge,
    count: u64,
    label: &str,
) -> Result<Vec<(RepresentationId, Option<ActivityRole>)>, Error> {
    let count = usize::try_from(count)
        .map_err(|_| invalid_argument(format!("activity {label} count is too large")))?;
    if count > MAX_ACTIVITY_EDGES {
        return Err(invalid_argument(format!(
            "activity {label} count must not exceed {MAX_ACTIVITY_EDGES}"
        )));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    if edges.is_null() {
        return Err(invalid_argument(format!(
            "activity {label} array must not be null when count is nonzero"
        )));
    }
    // SAFETY: The caller guarantees `count` readable contiguous edge values.
    unsafe { std::slice::from_raw_parts(edges, count) }
        .iter()
        .map(|edge| {
            // SAFETY: Each optional role follows the exported string contract.
            let role = unsafe { optional_utf8(edge.role, "activity edge role") }?
                .map(ActivityRole::new)
                .transpose()?;
            Ok((
                RepresentationId::from_bytes(edge.representation_id.bytes),
                role,
            ))
        })
        .collect()
}

unsafe fn dependencies_from_abi(
    dependencies: *const PpDependency,
    count: u64,
) -> Result<Vec<Dependency>, Error> {
    let count =
        usize::try_from(count).map_err(|_| invalid_argument("dependency count is too large"))?;
    if count > MAX_DEPENDENCIES_PER_SET {
        return Err(invalid_argument(format!(
            "dependency count must not exceed {MAX_DEPENDENCIES_PER_SET}"
        )));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    if dependencies.is_null() {
        return Err(invalid_argument(
            "dependencies must not be null when count is nonzero",
        ));
    }
    // SAFETY: The caller guarantees `count` readable contiguous dependency values.
    unsafe { std::slice::from_raw_parts(dependencies, count) }
        .iter()
        .map(|dependency| {
            // SAFETY: Each string follows the exported borrowed-string contract.
            let kind = unsafe { required_utf8(dependency.kind, "dependency kind") }?;
            // SAFETY: The authored reference follows the same contract.
            let authored_reference = unsafe {
                required_utf8(
                    dependency.authored_reference,
                    "dependency authored_reference",
                )
            }?;
            let source_resource_id = optional_uuid_flagged(
                dependency.has_source_resource,
                dependency.source_resource_id.bytes,
                "has_source_resource",
            )?
            .map(ResourceId::from_bytes);
            let target = match object_ref_from_abi(dependency.target)? {
                ObjectRef::Asset(id) => DependencyTarget::Asset(id),
                ObjectRef::Representation(id) => DependencyTarget::Representation(id),
                _ => {
                    return Err(invalid_argument(
                        "dependency target must be an asset or representation",
                    ));
                }
            };
            let resolved_representation_id = optional_uuid_flagged(
                dependency.has_resolved_representation,
                dependency.resolved_representation_id.bytes,
                "has_resolved_representation",
            )?
            .map(RepresentationId::from_bytes);
            let required = match dependency.required {
                0 => false,
                1 => true,
                _ => return Err(invalid_argument("dependency required must be zero or one")),
            };
            Dependency::new(
                source_resource_id,
                DependencyKind::new(kind)?,
                target,
                resolved_representation_id,
                required,
                authored_reference,
            )
        })
        .collect()
}

fn optional_uuid_flagged(
    flag: u8,
    value: [u8; 16],
    label: &str,
) -> Result<Option<[u8; 16]>, Error> {
    match flag {
        0 => Ok(None),
        1 => Ok(Some(value)),
        _ => Err(invalid_argument(format!("{label} must be zero or one"))),
    }
}

unsafe fn required_job_claim_ids(
    job_id: PpJobId,
    claim_id: *const PpUuid,
) -> Result<(JobId, JobClaimId), Error> {
    // SAFETY: Callers of this helper carry the exported readable-pointer contract.
    // SAFETY: The caller guarantees a readable claim identity.
    let claim_id = unsafe { claim_id.as_ref() }
        .ok_or_else(|| invalid_argument("claim_id must not be null"))?;
    Ok((
        JobId::from_bytes(job_id.bytes),
        JobClaimId::from_bytes(claim_id.bytes),
    ))
}

#[allow(
    clippy::too_many_arguments,
    reason = "the helper mirrors explicit C worker-attribution fields"
)]
unsafe fn worker_identity_from_abi(
    tool_name: *const c_char,
    tool_version: *const c_char,
    tool_uri: *const c_char,
    agent_name: *const c_char,
    agent_identifier_scheme: *const c_char,
    agent_identifier_value: *const c_char,
    agent_identifier_qualifier: *const c_char,
) -> Result<(ToolIdentity, Option<AgentIdentity>), Error> {
    // SAFETY: Exported callers guarantee valid borrowed strings.
    let tool_name = unsafe { required_utf8(tool_name, "tool_name") }?;
    // SAFETY: Optional fields follow the same string contract.
    let tool_version = unsafe { optional_utf8(tool_version, "tool_version") }?;
    // SAFETY: Optional fields follow the same string contract.
    let tool_uri = unsafe { optional_utf8(tool_uri, "tool_uri") }?;
    let tool = ToolIdentity::new(
        tool_name,
        tool_version.map(str::to_owned),
        tool_uri.map(str::to_owned),
    )?;
    // SAFETY: Optional fields follow the same string contract.
    let agent_name = unsafe { optional_utf8(agent_name, "agent_name") }?;
    // SAFETY: Optional fields follow the same string contract.
    let agent_scheme =
        unsafe { optional_utf8(agent_identifier_scheme, "agent_identifier_scheme") }?;
    // SAFETY: Optional fields follow the same string contract.
    let agent_value = unsafe { optional_utf8(agent_identifier_value, "agent_identifier_value") }?;
    // SAFETY: Optional fields follow the same string contract.
    let agent_qualifier =
        unsafe { optional_utf8(agent_identifier_qualifier, "agent_identifier_qualifier") }?;
    let identifier = match (agent_scheme, agent_value) {
        (Some(scheme), Some(value)) => Some(ExternalIdentifier::new(
            IdentifierScheme::new(scheme)?,
            value,
            agent_qualifier.map(str::to_owned),
        )?),
        (None, None) if agent_qualifier.is_none() => None,
        _ => {
            return Err(invalid_argument(
                "agent identifier scheme and value must be supplied together",
            ));
        }
    };
    let agent = if agent_name.is_some() || identifier.is_some() {
        Some(AgentIdentity::new(
            agent_name.map(str::to_owned),
            identifier,
        )?)
    } else {
        None
    };
    Ok((tool, agent))
}

fn invalid_argument(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidArgument, message)
}

unsafe fn external_identifier_from_abi(
    scheme: *const c_char,
    value: *const c_char,
    qualifier: *const c_char,
) -> Result<ExternalIdentifier, Error> {
    // SAFETY: The exported caller guarantees live NUL-terminated strings.
    let scheme = IdentifierScheme::new(unsafe { required_utf8(scheme, "scheme") }?)?;
    // SAFETY: Same contract as above.
    let value = unsafe { required_utf8(value, "value") }?;
    // SAFETY: Null is accepted for the optional qualifier.
    let qualifier = unsafe { optional_utf8(qualifier, "qualifier") }?.map(str::to_owned);
    ExternalIdentifier::new(scheme, value, qualifier)
}

unsafe fn metadata_property_from_abi(
    vocabulary: *const c_char,
    property: *const c_char,
) -> Result<MetadataProperty, Error> {
    // SAFETY: Exported callers guarantee live NUL-terminated strings.
    let vocabulary = VocabularyId::new(unsafe { required_utf8(vocabulary, "vocabulary") }?)?;
    // SAFETY: Same contract as above.
    let property = PropertyId::new(unsafe { required_utf8(property, "property") }?)?;
    Ok(MetadataProperty::new(vocabulary, property))
}

fn object_ref_from_abi(value: PpObjectRef) -> Result<ObjectRef, Error> {
    match value.kind {
        PP_OBJECT_PRODUCTION => Ok(ObjectRef::Production(ProductionId::from_bytes(
            value.id.bytes,
        ))),
        PP_OBJECT_ASSET => Ok(ObjectRef::Asset(AssetId::from_bytes(value.id.bytes))),
        PP_OBJECT_REPRESENTATION => Ok(ObjectRef::Representation(RepresentationId::from_bytes(
            value.id.bytes,
        ))),
        PP_OBJECT_RESOURCE => Ok(ObjectRef::Resource(ResourceId::from_bytes(value.id.bytes))),
        PP_OBJECT_ACTIVITY => Ok(ObjectRef::Activity(
            postproject_core::ActivityId::from_bytes(value.id.bytes),
        )),
        PP_OBJECT_JOB => Ok(ObjectRef::Job(postproject_core::JobId::from_bytes(
            value.id.bytes,
        ))),
        kind => Err(invalid_argument(format!(
            "object kind {kind} is not recognized"
        ))),
    }
}

fn representation_kind_from_abi(value: u32) -> Result<RepresentationKind, Error> {
    match value {
        PP_REPRESENTATION_ORIGINAL => Ok(RepresentationKind::Original),
        PP_REPRESENTATION_PROXY => Ok(RepresentationKind::Proxy),
        PP_REPRESENTATION_OPTIMIZED => Ok(RepresentationKind::Optimized),
        PP_REPRESENTATION_DERIVED => Ok(RepresentationKind::Derived),
        _ => Err(invalid_argument("unknown representation kind")),
    }
}

fn job_state_kind_from_abi(value: u32) -> Result<Option<JobStateKind>, Error> {
    match value {
        0 => Ok(None),
        1 => Ok(Some(JobStateKind::Requested)),
        2 => Ok(Some(JobStateKind::Claimed)),
        3 => Ok(Some(JobStateKind::Succeeded)),
        4 => Ok(Some(JobStateKind::Failed)),
        5 => Ok(Some(JobStateKind::Cancelled)),
        _ => Err(invalid_argument("job state filter is invalid")),
    }
}

pub(crate) fn object_ref_to_abi(value: ObjectRef) -> Result<PpObjectRef, Error> {
    let (kind, bytes) = match value {
        ObjectRef::Production(id) => (PP_OBJECT_PRODUCTION, id.into_bytes()),
        ObjectRef::Asset(id) => (PP_OBJECT_ASSET, id.into_bytes()),
        ObjectRef::Representation(id) => (PP_OBJECT_REPRESENTATION, id.into_bytes()),
        ObjectRef::Resource(id) => (PP_OBJECT_RESOURCE, id.into_bytes()),
        ObjectRef::Activity(id) => (PP_OBJECT_ACTIVITY, id.into_bytes()),
        ObjectRef::Job(id) => (PP_OBJECT_JOB, id.into_bytes()),
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "object kind is not supported by this ABI",
            ));
        }
    };
    Ok(PpObjectRef {
        kind,
        id: PpUuid { bytes },
    })
}

const fn empty_object_ref() -> PpObjectRef {
    PpObjectRef {
        kind: 0,
        id: PpUuid { bytes: [0; 16] },
    }
}

const fn empty_transaction_conflict() -> PpTransactionConflict {
    PpTransactionConflict {
        kind: 0,
        target: empty_object_ref(),
        media_root_id: PpMediaRootId { bytes: [0; 16] },
        namespace_name: ptr::null(),
        local_name: ptr::null(),
        qualifier: ptr::null(),
        version: 0,
        has_base_revision: 0,
        base_revision_id: PpRevisionId { bytes: [0; 16] },
        base_revision_sequence: 0,
        superseding_revision_id: PpRevisionId { bytes: [0; 16] },
        superseding_revision_sequence: 0,
    }
}

const fn resource_target(resource_id: ResourceId) -> PpObjectRef {
    PpObjectRef {
        kind: PP_OBJECT_RESOURCE,
        id: PpUuid {
            bytes: resource_id.into_bytes(),
        },
    }
}

const fn representation_target(representation_id: RepresentationId) -> PpObjectRef {
    PpObjectRef {
        kind: PP_OBJECT_REPRESENTATION,
        id: PpUuid {
            bytes: representation_id.into_bytes(),
        },
    }
}

fn lossy_cstring(value: &str) -> CString {
    CString::new(value.replace('\0', "�")).unwrap_or_default()
}

const fn error_code(kind: ErrorKind) -> u32 {
    match kind {
        ErrorKind::InvalidArgument => PP_ERROR_INVALID_ARGUMENT,
        ErrorKind::NotFound => PP_ERROR_NOT_FOUND,
        ErrorKind::AlreadyExists => PP_ERROR_ALREADY_EXISTS,
        ErrorKind::Io => PP_ERROR_IO,
        ErrorKind::Storage => PP_ERROR_STORAGE,
        ErrorKind::Migration => PP_ERROR_MIGRATION,
        ErrorKind::Conflict => PP_ERROR_CONFLICT,
        ErrorKind::AmbiguousResolution => PP_ERROR_AMBIGUOUS_RESOLUTION,
        ErrorKind::Fingerprint => PP_ERROR_FINGERPRINT,
        ErrorKind::Unsupported => PP_ERROR_UNSUPPORTED,
        ErrorKind::Cancelled => PP_ERROR_CANCELLED,
        _ => PP_ERROR_INTERNAL,
    }
}

fn uuid(id: ProductionId) -> PpProductionId {
    PpProductionId {
        bytes: id.into_bytes(),
    }
}

fn production_handle(production: SqliteProduction) -> PpProduction {
    PpProduction {
        state: Arc::new(ProductionState {
            inner: Mutex::new(production),
            transaction_open: AtomicBool::new(false),
        }),
    }
}

fn lock_production(state: &ProductionState) -> MutexGuard<'_, SqliteProduction> {
    state
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

unsafe fn representation_resolution_at<'a>(
    resolutions: *const PpResolutionSet,
    index: u64,
) -> Result<&'a AbiRepresentationResolution, Error> {
    // SAFETY: Exported callers guarantee a non-null handle remains live for the
    // complete call. The reference never escapes an exported operation.
    let resolutions = unsafe { resolutions.as_ref() }
        .ok_or_else(|| invalid_argument("resolutions must not be null"))?;
    item_at(
        &resolutions.representations,
        index,
        "representation resolution",
    )
}

unsafe fn resource_resolution_at<'a>(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    resource_index: u64,
) -> Result<&'a AbiResolution, Error> {
    // SAFETY: The caller upholds the live-handle contract for this complete call.
    let representation =
        unsafe { representation_resolution_at(resolutions, representation_index) }?;
    item_at(
        &representation.resources,
        resource_index,
        "resource resolution",
    )
}

fn item_at<'a, T>(items: &'a [T], index: u64, label: &str) -> Result<&'a T, Error> {
    let index = usize::try_from(index)
        .map_err(|_| invalid_argument(format!("{label} index is out of range")))?;
    items
        .get(index)
        .ok_or_else(|| invalid_argument(format!("{label} index {index} is out of range")))
}

fn length_as_u64(length: usize) -> Result<u64, Error> {
    u64::try_from(length).map_err(|_| Error::new(ErrorKind::Internal, "result is too large"))
}

unsafe fn metadata_value<'a>(value: *const PpMetadataValue) -> Result<&'a PpMetadataValue, Error> {
    // SAFETY: The exported caller guarantees the pointer is borrowed from a
    // live result set for the duration of the call.
    unsafe { value.as_ref() }.ok_or_else(|| invalid_argument("value must not be null"))
}

fn metadata_type_error(expected: &str) -> Error {
    invalid_argument(format!("metadata value is not {expected}"))
}

unsafe fn write_copy<T: Copy>(output: *mut T, value: T, label: &str) -> Result<(), Error> {
    require_output(output, label)?;
    // SAFETY: The caller contract requires this checked non-null output to be writable.
    unsafe { output.write(value) };
    Ok(())
}

impl PpResolutionSet {
    fn new(resolutions: Vec<(AssetId, RepresentationResolution)>) -> Self {
        Self {
            representations: resolutions
                .into_iter()
                .map(|(asset_id, resolution)| AbiRepresentationResolution {
                    asset_id,
                    representation_id: resolution.representation_id(),
                    availability: resolution.availability(),
                    resources: resolution
                        .resources()
                        .iter()
                        .map(|resource| AbiResolution {
                            resource_id: resource.resource_id(),
                            state: resolution_state(resource.state()),
                            candidates: resource
                                .candidates()
                                .iter()
                                .map(|candidate| AbiCandidate {
                                    uri: sanitized_cstring(candidate.uri()),
                                    confidence: candidate.confidence().basis_points(),
                                    media_root: candidate.media_root().map(sanitized_cstring),
                                    sequence_naming: candidate
                                        .sequence_naming()
                                        .map(AbiSequenceNaming::sanitized),
                                    evidence: candidate
                                        .evidence()
                                        .iter()
                                        .map(AbiEvidence::from)
                                        .collect(),
                                })
                                .collect(),
                            evidence: resource.evidence().iter().map(AbiEvidence::from).collect(),
                        })
                        .collect(),
                    issues: resolution.issues().to_vec(),
                })
                .collect(),
        }
    }
}

impl From<&ResolutionEvidence> for AbiEvidence {
    fn from(evidence: &ResolutionEvidence) -> Self {
        Self {
            kind: evidence_kind(evidence.kind()),
            detail: evidence.detail().map(sanitized_cstring),
        }
    }
}

fn sanitized_cstring(value: &str) -> CString {
    CString::new(value.replace('\0', "�")).unwrap_or_default()
}

pub(crate) fn exact_cstring(value: &str, label: &str) -> Result<CString, Error> {
    CString::new(value).map_err(|_| {
        Error::new(
            ErrorKind::Internal,
            format!("validated {label} unexpectedly contains NUL"),
        )
    })
}

impl TryFrom<ExternalIdentifier> for AbiExternalIdentifier {
    type Error = Error;

    fn try_from(identifier: ExternalIdentifier) -> Result<Self, Self::Error> {
        Ok(Self {
            scheme: exact_cstring(identifier.scheme().as_str(), "identifier scheme")?,
            value: exact_cstring(identifier.value(), "identifier value")?,
            qualifier: identifier
                .qualifier()
                .map(|value| exact_cstring(value, "identifier qualifier"))
                .transpose()?,
        })
    }
}

const fn resolution_state(state: ResourceResolutionState) -> u32 {
    match state {
        ResourceResolutionState::OnlineAtKnownLocator => PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR,
        ResourceResolutionState::ResolvedExact => PP_RESOURCE_RESOLVED_EXACT,
        ResourceResolutionState::ResolvedProbable => PP_RESOURCE_RESOLVED_PROBABLE,
        ResourceResolutionState::Offline => PP_RESOURCE_OFFLINE,
        ResourceResolutionState::Ambiguous => PP_RESOURCE_AMBIGUOUS,
        ResourceResolutionState::Error => PP_RESOURCE_RESOLUTION_ERROR,
        _ => 0,
    }
}

const fn representation_availability(availability: RepresentationAvailability) -> u32 {
    match availability {
        RepresentationAvailability::Online => PP_AVAILABILITY_ONLINE,
        RepresentationAvailability::Partial => PP_AVAILABILITY_PARTIAL,
        RepresentationAvailability::Offline => PP_AVAILABILITY_OFFLINE,
        RepresentationAvailability::Ambiguous => PP_AVAILABILITY_AMBIGUOUS,
        RepresentationAvailability::Error => PP_AVAILABILITY_ERROR,
        _ => 0,
    }
}

const fn availability_issue_kind(kind: AvailabilityIssueKind) -> u32 {
    match kind {
        AvailabilityIssueKind::OfflineResource => PP_AVAILABILITY_ISSUE_OFFLINE_RESOURCE,
        AvailabilityIssueKind::AmbiguousResource => PP_AVAILABILITY_ISSUE_AMBIGUOUS_RESOURCE,
        AvailabilityIssueKind::ResourceError => PP_AVAILABILITY_ISSUE_RESOURCE_ERROR,
        AvailabilityIssueKind::MissingFrames => PP_AVAILABILITY_ISSUE_MISSING_FRAMES,
        _ => 0,
    }
}

const fn evidence_kind(kind: EvidenceKind) -> u32 {
    match kind {
        EvidenceKind::KnownLocatorAvailable => PP_EVIDENCE_KNOWN_LOCATOR_AVAILABLE,
        EvidenceKind::ExactFingerprintMatch => PP_EVIDENCE_EXACT_FINGERPRINT_MATCH,
        EvidenceKind::FullHashMatch => PP_EVIDENCE_FULL_HASH_MATCH,
        EvidenceKind::PartialFingerprintMatch => PP_EVIDENCE_PARTIAL_FINGERPRINT_MATCH,
        EvidenceKind::FileSizeMatch => PP_EVIDENCE_FILE_SIZE_MATCH,
        EvidenceKind::FileNameMatch => PP_EVIDENCE_FILE_NAME_MATCH,
        EvidenceKind::RelativePathSimilarity => PP_EVIDENCE_RELATIVE_PATH_SIMILARITY,
        EvidenceKind::MediaRootRelation => PP_EVIDENCE_MEDIA_ROOT_RELATION,
        EvidenceKind::ConflictingCandidate => PP_EVIDENCE_CONFLICTING_CANDIDATE,
        EvidenceKind::DiscoveryError => PP_EVIDENCE_DISCOVERY_ERROR,
        EvidenceKind::MediaRootUnmapped => PP_EVIDENCE_MEDIA_ROOT_UNMAPPED,
        EvidenceKind::MediaRootUnavailable => PP_EVIDENCE_MEDIA_ROOT_UNAVAILABLE,
        EvidenceKind::FingerprintMismatch => PP_EVIDENCE_FINGERPRINT_MISMATCH,
        EvidenceKind::FingerprintNotVerified => PP_EVIDENCE_FINGERPRINT_NOT_VERIFIED,
        EvidenceKind::SearchTruncated => PP_EVIDENCE_SEARCH_TRUNCATED,
        _ => 0,
    }
}

fn apply_staged_mutation(
    transaction: &mut postproject_storage_sqlite::SqliteTransaction<'_>,
    mutation: &StagedMutation,
) -> Result<(), Error> {
    match mutation {
        StagedMutation::Import(import) => transaction.import_original(import)?,
        StagedMutation::Representation(import) => {
            transaction.add_representation(import)?;
        }
        StagedMutation::MediaRoot(root) => transaction.add_media_root(root.clone())?,
        StagedMutation::SetMediaRootEnabled(root_id, enabled) => {
            transaction.set_media_root_enabled(*root_id, *enabled)?;
        }
        StagedMutation::RemoveMediaRoot(root_id) => {
            transaction.remove_media_root(*root_id)?;
        }
        StagedMutation::Locator(locator) => transaction.add_locator(locator)?,
        StagedMutation::RetireLocator(locator_id) => {
            transaction.retire_locator(*locator_id)?;
        }
        StagedMutation::RecordResourceFingerprint(resource_id, fingerprint) => {
            transaction.record_resource_fingerprint(*resource_id, fingerprint)?;
        }
        StagedMutation::RecordResourceFileFacts(resource_id, facts) => {
            transaction.record_resource_file_facts(*resource_id, *facts)?;
        }
        StagedMutation::RecordRepresentationFingerprint(representation_id, fingerprint) => {
            transaction.record_representation_fingerprint(*representation_id, fingerprint)?;
        }
        StagedMutation::RecordDependencySet(representation_id, dependencies) => {
            transaction.record_dependency_set(*representation_id, dependencies)?;
        }
        StagedMutation::AddExternalIdentifier(target, identifier) => {
            transaction.add_external_identifier(*target, identifier)?;
        }
        StagedMutation::RemoveExternalIdentifier(target, identifier) => {
            transaction.remove_external_identifier(*target, identifier)?;
        }
        StagedMutation::AddMetadataValue(target, property, value) => {
            transaction.add_metadata_value(*target, property, value)?;
        }
        StagedMutation::RemoveMetadataProperty(target, property) => {
            transaction.remove_metadata_property(*target, property)?;
        }
        StagedMutation::Activity(activity) => {
            transaction.create_activity(activity)?;
        }
        StagedMutation::RequestJob(job) => transaction.request_job(job)?,
        StagedMutation::ClaimJob {
            job_id,
            claim_id,
            tool,
            agent,
            now,
            expires_at,
        } => {
            transaction.claim_job_with_id(
                *job_id,
                *claim_id,
                tool,
                agent.as_ref(),
                *now,
                *expires_at,
            )?;
        }
        StagedMutation::RenewJobClaim(job_id, claim_id, now, expires_at) => {
            transaction.renew_job_claim(*job_id, *claim_id, *now, *expires_at)?;
        }
        StagedMutation::ReleaseJobClaim(job_id, claim_id) => {
            transaction.release_job_claim(*job_id, *claim_id)?;
        }
        StagedMutation::CompleteJob {
            job_id,
            claim_id,
            now,
            output,
            activity,
        } => {
            transaction.complete_job(*job_id, *claim_id, *now, output, activity)?;
        }
        StagedMutation::FailJob(job_id, claim_id, now, failure) => {
            transaction.fail_job(*job_id, *claim_id, *now, failure)?;
        }
        StagedMutation::CancelJob(job_id) => transaction.cancel_job(*job_id)?,
    }
    Ok(())
}

fn begin_transaction_handle(
    state: &Arc<ProductionState>,
    base_revision: Option<RevisionId>,
    decision_base: Option<DecisionBase>,
) -> Result<*mut PpTransaction, Error> {
    if lock_production(state).is_read_only() {
        return Err(invalid_argument("a pinned read view cannot begin a write"));
    }
    if let Some(base_revision) = base_revision {
        lock_production(state).events_for_revision(base_revision)?;
    }
    if let Some(base) = decision_base {
        // Validate scope and revision pairing now and again at actual commit.
        lock_production(state).begin_edit(base)?;
    }
    if state
        .transaction_open
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(Error::new(
            ErrorKind::Conflict,
            "production already has an open transaction",
        ));
    }
    Ok(Box::into_raw(Box::new(PpTransaction {
        state: Arc::clone(state),
        lifecycle: TransactionLifecycle::new(),
        revision_context: RevisionContext::default(),
        base_revision,
        decision_base,
        decision_reader: None,
        mutations: Vec::new(),
    })))
}

impl PpTransaction {
    fn require_decision_base(&self) -> Result<(), Error> {
        self.lifecycle.ensure_open()?;
        if self.decision_base.is_none() && self.base_revision.is_none() {
            return Err(invalid_argument(
                "non-additive mutation requires a decision base",
            ));
        }
        Ok(())
    }

    fn commit(&mut self) -> Result<CommitReceipt, Error> {
        self.lifecycle.ensure_open()?;
        let result = (|| {
            let mut production = lock_production(&self.state);
            let mut transaction = match (self.decision_base, self.base_revision) {
                (Some(base), _) => production.begin_edit(base)?,
                (None, Some(base_revision)) => production.begin_transaction_at(base_revision)?,
                (None, None) => production.begin_transaction()?,
            };
            transaction.set_revision_context(self.revision_context.clone())?;
            for mutation in &self.mutations {
                apply_staged_mutation(&mut transaction, mutation)?;
            }
            transaction.commit_with_receipt()
        })();

        self.state.transaction_open.store(false, Ordering::Release);
        if result.is_ok() {
            self.lifecycle.mark_committed()?;
            self.mutations.clear();
        } else {
            let state_result = self.lifecycle.mark_rolled_back();
            debug_assert!(state_result.is_ok());
        }
        result
    }

    fn rollback(&mut self) -> Result<(), Error> {
        self.lifecycle.mark_rolled_back()?;
        self.mutations.clear();
        self.state.transaction_open.store(false, Ordering::Release);
        Ok(())
    }
}

impl Drop for PpTransaction {
    fn drop(&mut self) {
        // Commit and rollback already relinquished this transaction's guard.
        // A closed handle can outlive a newer transaction on the production.
        if self.lifecycle.ensure_open().is_ok() {
            self.state.transaction_open.store(false, Ordering::Release);
        }
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload.downcast_ref::<&str>().map_or_else(
        || {
            payload.downcast_ref::<String>().map_or_else(
                || "panic contained at the C ABI boundary".to_owned(),
                |message| format!("panic contained at the C ABI boundary: {message}"),
            )
        },
        |message| format!("panic contained at the C ABI boundary: {message}"),
    )
}

#[cfg(test)]
mod tests {
    use std::{ffi::CStr, sync::mpsc, thread, time::Duration};

    use super::*;
    use postproject_core::{
        Confidence, ContentStructure, EvidenceKind, FrameRange, ImageSequenceDescriptor,
        MetadataAssertion, MetadataField, MetadataProperty, PropertyId, RationalRate,
        ResolutionCandidate, ResourceResolution, SequenceNaming, VocabularyId,
    };

    #[test]
    fn creates_reads_and_releases_production_handle() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = CString::new(
            directory
                .path()
                .join("production.pproj")
                .to_string_lossy()
                .as_bytes(),
        )
        .expect("path has no NUL");
        let mut production = ptr::null_mut();
        let mut error = ptr::null_mut();

        // SAFETY: Test inputs and outputs follow the documented ABI contract.
        let status = unsafe {
            pp_production_create(
                path.as_ptr(),
                ptr::null(),
                &raw mut production,
                &raw mut error,
            )
        };
        assert_eq!(status, PP_OK);
        assert!(!production.is_null());
        assert!(error.is_null());

        let mut id = PpProductionId { bytes: [0; 16] };
        // SAFETY: `production` is live and outputs are writable.
        assert_eq!(
            unsafe { pp_production_id(production, &raw mut id, &raw mut error) },
            PP_OK
        );
        assert_ne!(id.bytes, [0; 16]);
        // SAFETY: The live handle is released exactly once.
        unsafe { pp_production_release(production) };
    }

    #[test]
    fn production_handles_are_send_sync_and_block_on_concurrent_access() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PpProduction>();

        let directory = tempfile::tempdir().expect("create directory");
        let production = SqliteProduction::create(directory.path().join("production.pproj"), None)
            .expect("create production");
        let handle = Arc::new(production_handle(production));
        let guard = lock_production(&handle.state);
        let worker_handle = Arc::clone(&handle);
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut id = PpProductionId { bytes: [0; 16] };
            let mut error = ptr::null_mut();
            // SAFETY: The Arc keeps the handle live, the outputs are local, and
            // no thread releases the handle while this call runs.
            let status = unsafe {
                pp_production_id(Arc::as_ptr(&worker_handle), &raw mut id, &raw mut error)
            };
            sender.send((status, id, error.is_null())).unwrap();
        });

        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        drop(guard);
        let (status, id, error_is_null) = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("concurrent read completes after unlock");
        worker.join().expect("join reader");
        assert_eq!(status, PP_OK);
        assert_ne!(id.bytes, [0; 16]);
        assert!(error_is_null);
    }

    #[test]
    fn poisoned_production_lock_is_recovered() {
        let directory = tempfile::tempdir().expect("create directory");
        let production = SqliteProduction::create(directory.path().join("production.pproj"), None)
            .expect("create production");
        let handle = Arc::new(production_handle(production));
        let state = Arc::clone(&handle.state);
        assert!(
            thread::spawn(move || {
                let _guard = lock_production(&state);
                panic!("poison production lock");
            })
            .join()
            .is_err()
        );

        let mut id = PpProductionId { bytes: [0; 16] };
        let mut error = ptr::null_mut();
        // SAFETY: The Arc keeps the handle live and outputs are writable.
        let status = unsafe { pp_production_id(Arc::as_ptr(&handle), &raw mut id, &raw mut error) };
        assert_eq!(status, PP_OK);
        assert_ne!(id.bytes, [0; 16]);
        assert!(error.is_null());
    }

    #[test]
    fn invalid_arguments_return_owned_error() {
        let mut production = ptr::null_mut();
        let mut error = ptr::null_mut();

        // SAFETY: Null is intentionally supplied where the API validates it.
        let status =
            unsafe { pp_production_open(ptr::null(), &raw mut production, &raw mut error) };
        assert_eq!(status, PP_ERROR_INVALID_ARGUMENT);
        assert!(production.is_null());
        assert!(!error.is_null());
        // SAFETY: `error` is a live library-owned error handle.
        assert_eq!(unsafe { pp_error_code(error) }, status);
        // SAFETY: The message is borrowed while `error` remains live.
        assert!(!unsafe { pp_error_message(error) }.is_null());
        // SAFETY: The live error is released exactly once.
        unsafe { pp_error_release(error) };
    }

    #[test]
    fn transaction_retains_production_state_and_commits_import() {
        let directory = tempfile::tempdir().expect("create directory");
        let production_path = directory.path().join("production.pproj");
        let media_path = directory.path().join("clip.mov");
        std::fs::write(&media_path, b"FFI transaction media").expect("write media");
        let production_path = CString::new(production_path.to_string_lossy().as_bytes())
            .expect("production path has no NUL");
        let media_path =
            CString::new(media_path.to_string_lossy().as_bytes()).expect("media path has no NUL");
        let mut production = ptr::null_mut();
        let mut transaction = ptr::null_mut();
        let mut error = ptr::null_mut();

        // SAFETY: Test inputs and outputs follow the documented ABI contract.
        assert_eq!(
            unsafe {
                pp_production_create(
                    production_path.as_ptr(),
                    ptr::null(),
                    &raw mut production,
                    &raw mut error,
                )
            },
            PP_OK
        );
        // SAFETY: `production` is live and the transaction output is writable.
        assert_eq!(
            unsafe {
                pp_production_begin_transaction(production, &raw mut transaction, &raw mut error)
            },
            PP_OK
        );
        // SAFETY: The transaction retains shared ownership of the state.
        unsafe { pp_production_release(production) };

        let mut source = ptr::null_mut();
        // SAFETY: The path is NUL-terminated and the outputs are writable.
        assert_eq!(
            unsafe {
                media_source::pp_media_source_create_file(
                    media_path.as_ptr(),
                    &raw mut source,
                    &raw mut error,
                )
            },
            PP_OK
        );
        let mut asset_id = PpAssetId { bytes: [0; 16] };
        // SAFETY: `transaction` and `source` are live and outputs are writable.
        assert_eq!(
            unsafe {
                pp_transaction_import_media(
                    transaction,
                    source,
                    ptr::null(),
                    &raw mut asset_id,
                    &raw mut error,
                )
            },
            PP_OK
        );
        // SAFETY: The live source is released exactly once.
        unsafe { media_source::pp_media_source_release(source) };
        // SAFETY: `transaction` remains live and exclusively accessed.
        assert_eq!(
            unsafe { pp_transaction_commit(transaction, &raw mut error) },
            PP_OK
        );
        // SAFETY: The live transaction is released exactly once.
        unsafe { pp_transaction_release(transaction) };

        let reopened = SqliteProduction::open(Path::new(
            production_path.to_str().expect("production path is UTF-8"),
        ))
        .expect("reopen production");
        let assets = reopened.assets().expect("load committed assets");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id().into_bytes(), asset_id.bytes);
        assert!(error.is_null());
    }

    #[test]
    fn resolution_accessors_reject_out_of_range_indices() {
        let resolutions = Box::into_raw(Box::new(PpResolutionSet::new(Vec::new())));
        let mut asset_id = PpAssetId { bytes: [9; 16] };
        let mut representation_id = PpRepresentationId { bytes: [9; 16] };
        let mut availability = 99;
        let mut resource_count = 99;
        let mut issue_count = 99;
        let mut error = ptr::null_mut();

        // SAFETY: The handle is live and every output is writable.
        let status = unsafe {
            pp_resolution_set_get_representation(
                resolutions,
                0,
                &raw mut asset_id,
                &raw mut representation_id,
                &raw mut availability,
                &raw mut resource_count,
                &raw mut issue_count,
                &raw mut error,
            )
        };
        assert_eq!(status, PP_ERROR_INVALID_ARGUMENT);
        assert_eq!(representation_id.bytes, [0; 16]);
        assert_eq!(availability, 0);
        assert_eq!(resource_count, 0);
        assert_eq!(issue_count, 0);
        assert!(!error.is_null());

        // SAFETY: Both handles are live and released exactly once.
        unsafe {
            pp_error_release(error);
            pp_resolution_set_release(resolutions);
        }
        // SAFETY: Null is explicitly accepted by the count accessor.
        assert_eq!(
            unsafe { pp_resolution_set_representation_count(ptr::null()) },
            0
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one result set is read through every nested accessor"
    )]
    fn nested_resolution_accessors_expose_missing_sequence_frames() {
        let representation_id = RepresentationId::new();
        let resource_id = ResourceId::new();
        let descriptor = ImageSequenceDescriptor::new(
            resource_id,
            FrameRange::new(1001, 1003, 1).expect("valid frames"),
            RationalRate::new(24, 1).expect("valid rate"),
            vec![1002],
        )
        .expect("valid sequence");
        let candidate = ResolutionCandidate::new(
            "file:///plates/",
            Confidence::CERTAIN,
            vec![ResolutionEvidence::new(
                EvidenceKind::KnownLocatorAvailable,
                None,
            )],
        )
        .expect("valid candidate")
        .with_sequence_naming(SequenceNaming::new("plate.", ".exr", 4).expect("valid naming"));
        let resource = ResourceResolution::new(
            resource_id,
            ResourceResolutionState::OnlineAtKnownLocator,
            vec![candidate],
            Vec::new(),
        )
        .expect("valid resource result");
        let representation = RepresentationResolution::aggregate(
            representation_id,
            &ContentStructure::image_sequence(descriptor),
            vec![resource],
        )
        .expect("valid aggregate");
        let resolutions = Box::into_raw(Box::new(PpResolutionSet::new(vec![(
            AssetId::new(),
            representation,
        )])));
        let mut asset_id = PpAssetId { bytes: [0; 16] };
        let mut id = PpRepresentationId { bytes: [0; 16] };
        let mut availability = 0;
        let mut resource_count = 0;
        let mut issue_count = 0;
        let mut error = ptr::null_mut();

        // SAFETY: The handle is live and all outputs are writable.
        assert_eq!(
            unsafe {
                pp_resolution_set_get_representation(
                    resolutions,
                    0,
                    &raw mut asset_id,
                    &raw mut id,
                    &raw mut availability,
                    &raw mut resource_count,
                    &raw mut issue_count,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(id.bytes, representation_id.into_bytes());
        assert_eq!(availability, PP_AVAILABILITY_PARTIAL);
        assert_eq!(resource_count, 1);
        assert_eq!(issue_count, 1);

        let mut resource_result = PpResourceId { bytes: [0; 16] };
        let mut required = 0;
        let mut kind = 0;
        let mut frame_count = 0;
        // SAFETY: The handle remains live and all outputs are writable.
        assert_eq!(
            unsafe {
                pp_resolution_set_get_issue(
                    resolutions,
                    0,
                    0,
                    &raw mut resource_result,
                    &raw mut required,
                    &raw mut kind,
                    &raw mut frame_count,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(resource_result.bytes, resource_id.into_bytes());
        assert_eq!(required, 1);
        assert_eq!(kind, PP_AVAILABILITY_ISSUE_MISSING_FRAMES);
        assert_eq!(frame_count, 1);

        let mut frame = 0;
        // SAFETY: The handle remains live and the output is writable.
        assert_eq!(
            unsafe {
                pp_resolution_set_get_issue_frame(
                    resolutions,
                    0,
                    0,
                    0,
                    &raw mut frame,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(frame, 1002);
        assert!(error.is_null());

        let mut uri = ptr::null();
        let mut confidence = 0;
        let mut media_root = ptr::null();
        let mut has_naming = 0;
        let mut naming = PpSequenceNaming {
            prefix: ptr::null(),
            suffix: ptr::null(),
            padding: 0,
        };
        let mut evidence_count = 0;
        // SAFETY: The handle remains live and all outputs are writable.
        assert_eq!(
            unsafe {
                pp_resolution_set_get_candidate(
                    resolutions,
                    0,
                    0,
                    0,
                    &raw mut uri,
                    &raw mut confidence,
                    &raw mut media_root,
                    &raw mut has_naming,
                    &raw mut naming,
                    &raw mut evidence_count,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(has_naming, 1);
        // SAFETY: The naming borrows the still-live result set.
        assert_eq!(
            unsafe { CStr::from_ptr(naming.prefix) }.to_bytes(),
            b"plate."
        );
        // SAFETY: Same borrowed lifetime as the prefix.
        assert_eq!(unsafe { CStr::from_ptr(naming.suffix) }.to_bytes(), b".exr");
        assert_eq!(naming.padding, 4);
        // SAFETY: The live handle is released exactly once.
        unsafe { pp_resolution_set_release(resolutions) };
    }

    #[test]
    fn metadata_accessors_traverse_recursive_values() {
        let asset = ObjectRef::Asset(AssetId::from_bytes([7; 16]));
        let value = MetadataValue::structure(vec![
            MetadataField::new(
                PropertyId::new("labels").unwrap(),
                MetadataValue::list(vec![
                    MetadataValue::language_string("Interview", "en-US").unwrap(),
                ])
                .unwrap(),
            ),
            MetadataField::new(
                PropertyId::new("source").unwrap(),
                MetadataValue::reference(asset),
            ),
        ])
        .unwrap();
        let assertion = MetadataAssertion::new(
            MetadataProperty::new(
                VocabularyId::new("com.example.metadata").unwrap(),
                PropertyId::new("contact").unwrap(),
            ),
            value,
        );
        let metadata = Box::into_raw(Box::new(
            PpMetadataSet::from_assertions(asset, &[assertion]).unwrap(),
        ));
        let mut target = PpObjectRef {
            kind: 0,
            id: PpUuid { bytes: [0; 16] },
        };
        let mut vocabulary = ptr::null();
        let mut property = ptr::null();
        let mut root = ptr::null();
        let mut error = ptr::null_mut();

        // SAFETY: The result set is live and every output is writable.
        assert_eq!(
            unsafe {
                pp_metadata_set_get(
                    metadata,
                    0,
                    &raw mut target,
                    &raw mut vocabulary,
                    &raw mut property,
                    &raw mut root,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(target.kind, PP_OBJECT_ASSET);
        assert_eq!(
            unsafe { pp_metadata_value_kind(root) },
            metadata::PP_METADATA_STRUCT
        );
        assert_eq!(unsafe { pp_metadata_value_struct_count(root) }, 2);

        let mut field_name = ptr::null();
        let mut list = ptr::null();
        assert_eq!(
            unsafe {
                pp_metadata_value_struct_get(
                    root,
                    0,
                    &raw mut field_name,
                    &raw mut list,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(unsafe { CStr::from_ptr(field_name) }.to_bytes(), b"labels");
        assert_eq!(unsafe { pp_metadata_value_list_count(list) }, 1);
        let mut text_value = ptr::null();
        assert_eq!(
            unsafe { pp_metadata_value_list_get(list, 0, &raw mut text_value, &raw mut error) },
            PP_OK
        );
        let mut text = ptr::null();
        let mut language = ptr::null();
        assert_eq!(
            unsafe {
                pp_metadata_value_get_string(
                    text_value,
                    &raw mut text,
                    &raw mut language,
                    &raw mut error,
                )
            },
            PP_OK
        );
        assert_eq!(unsafe { CStr::from_ptr(text) }.to_bytes(), b"Interview");
        assert_eq!(unsafe { CStr::from_ptr(language) }.to_bytes(), b"en-US");

        // SAFETY: The live set is released after all borrowed pointers are done.
        unsafe { pp_metadata_set_release(metadata) };
        assert!(error.is_null());
    }
}
