//! Application-neutral domain types and service contracts for `PostProject`.
//!
//! This crate deliberately has no persistence, FFI, or application-framework
//! dependencies. Backends and adapters depend on this crate, never the reverse.

#![forbid(unsafe_code)]

mod artifact;
mod cancel;
mod conflict;
mod content;
mod dependency;
mod error;
mod id;
mod identifier;
mod identifier_registry;
mod job;
mod metadata;
mod metadata_registry;
mod model;
mod provenance;
mod query;
mod resolution;
mod resource;
mod revision;
mod storage;
mod time;
mod transaction;
mod uri;

pub use artifact::{
    ArtifactDependencyIssue, ArtifactDependencyPathSegment, ArtifactEdgeKind, ArtifactEvaluation,
    ArtifactEvaluationLimits, ArtifactKnowledgeReason, ArtifactKnowledgeState,
    ArtifactReproducibilityIssue, ArtifactReproducibilityReport, ArtifactTraversalLimitKind,
};
pub use cancel::CancellationToken;
pub use conflict::{
    ConcurrencyClassification, ConflictKeyKind, SemanticConflictKey, TransactionConflict,
};
pub use content::{
    ContentStructure, ContentStructureKind, FrameRange, ImageSequenceDescriptor,
    MAX_CONTENT_MEMBERS, MAX_FRAME_PADDING, MAX_RESOURCE_ROLE_BYTES, MAX_SEQUENCE_EXCEPTIONS,
    MAX_SEQUENCE_NAMING_BYTES, ResourceMember, ResourceRole, SequenceNaming,
};
pub use dependency::{
    Dependency, DependencyKind, DependencyQueryMatch, DependencySet, DependencySetStatus,
    DependencyTarget, MAX_AUTHORED_REFERENCE_BYTES, MAX_DEPENDENCIES_PER_SET,
    MAX_DEPENDENCY_KIND_BYTES,
};
pub use error::{Error, ErrorKind, Result};
pub use id::{
    ActivityId, AssetId, HostObjectBinding, JobClaimId, JobId, LocatorId, MediaRootId, ObjectRef,
    ProductionId, RepresentationId, ResourceId, RevisionId, TransactionId,
};
pub use identifier::{
    ExternalIdentifier, IdentifierScheme, MAX_IDENTIFIER_QUALIFIER_BYTES,
    MAX_IDENTIFIER_SCHEME_BYTES, MAX_IDENTIFIER_VALUE_BYTES,
};
pub use identifier_registry::{
    EIDR_SCHEME, IDENTIFIER_SCHEMES, ISAN_SCHEME, IdentifierSchemeDefinition,
    IdentifierValidationKind, POSTPROJECT_APPLICATION_SCHEME, SMPTE_UMID_SCHEME,
    identifier_scheme_definition, validate_known_identifier,
};
pub use job::{
    Job, JobClaim, JobCompletion, JobFailure, JobKind, JobQuery, JobState, JobStateKind,
    MAX_JOB_DIAGNOSTIC_BYTES, MAX_JOB_INPUTS, MAX_JOB_KIND_BYTES, MAX_REGENERATION_PLANS,
    RegenerationJobPlan, RequestedJobOutput,
};
pub use metadata::{
    DecimalValue, MAX_LANGUAGE_TAG_BYTES, MAX_METADATA_BINARY_BYTES, MAX_METADATA_COLLECTION_ITEMS,
    MAX_METADATA_DECIMAL_SCALE, MAX_METADATA_NESTING_DEPTH, MAX_METADATA_TEXT_BYTES,
    MAX_METADATA_TOTAL_BYTES, MAX_METADATA_URI_BYTES, MAX_PROPERTY_ID_BYTES,
    MAX_VOCABULARY_ID_BYTES, MetadataAssertion, MetadataField, MetadataMatch, MetadataProperty,
    MetadataValue, MetadataValueKind, PropertyId, RationalValue, VocabularyId,
};
pub use metadata_registry::{
    DUBLIN_CORE_ELEMENTS_VOCABULARY, EBUCORE_VOCABULARY, IPTC_VMH_JSON_VOCABULARY,
    METADATA_VOCABULARIES, MetadataCardinality, MetadataPropertyAlias, MetadataPropertyDefinition,
    MetadataVocabularyDefinition, POSTPROJECT_METADATA_VOCABULARY, XMP_BASIC_VOCABULARY,
    metadata_property_definition, metadata_vocabulary_definition,
};
pub use model::{
    Asset, MediaRoot, OriginalMediaImport, Production, Representation, RepresentationImport,
    RepresentationKind, Timestamp,
};
pub use provenance::{
    Activity, ActivityEdgeSnapshot, ActivityInput, ActivityKind, ActivityOutput, ActivityRole,
    AgentIdentity, MAX_ACTIVITY_EDGES, MAX_ACTIVITY_KIND_BYTES, MAX_ACTIVITY_ROLE_BYTES,
    MAX_PROVENANCE_NAME_BYTES, MAX_PROVENANCE_URI_BYTES, MAX_TOOL_VERSION_BYTES, ToolIdentity,
};
pub use query::{
    ActivityOutputQuery, DependencyQueryLimits, MAX_DEPENDENCY_QUERY_DEPTH,
    MAX_DEPENDENCY_QUERY_REPRESENTATIONS, MAX_PROVENANCE_QUERY_DEPTH,
    MAX_PROVENANCE_QUERY_REPRESENTATIONS, MAX_QUERY_CURSOR_BYTES, MAX_QUERY_PAGE_SIZE,
    MetadataQuery, ProvenanceQueryLimits, ProvenanceQueryMatch, QueryCursor, QueryPage,
    QueryPageRequest, StaleArtifactQuery,
};
pub use resolution::{
    AvailabilityIssue, AvailabilityIssueKind, Confidence, EvidenceKind, RepresentationAvailability,
    RepresentationResolution, ResolutionCandidate, ResolutionEvidence, ResourceResolution,
    ResourceResolutionState,
};
pub use resource::{
    FileFacts, FingerprintSnapshot, KnownMediaMatch, Locator, LocatorAvailability, LocatorIdentity,
    RepresentationFingerprint, Resource, ResourceFingerprint,
};
pub use revision::{
    FilteredRevisionPage, MAX_REVISION_MESSAGE_BYTES, MAX_REVISION_PAGE_SIZE, MAX_REVISION_WAIT,
    OriginIdentity, Revision, RevisionContext, RevisionEvent, RevisionEventFilter,
    RevisionEventKind, RevisionEventType, RevisionWaitOutcome, RevisionWaiter,
    validate_revision_wait,
};
pub use storage::{ProductionRead, ProductionStore, ProductionStoreTransaction};
pub use time::{RationalRate, RationalTime, TimeRange};
pub use transaction::{TransactionLifecycle, TransactionState};
