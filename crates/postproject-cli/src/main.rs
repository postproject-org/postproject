//! Command-line demonstrator for `PostProject` domain services.

#![forbid(unsafe_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityOutputQuery,
    ActivityRole, AgentIdentity, ArtifactDependencyIssue, ArtifactDependencyPathSegment,
    ArtifactEdgeKind, ArtifactEvaluationLimits, ArtifactKnowledgeReason, ArtifactKnowledgeState,
    ArtifactReproducibilityIssue, ArtifactTraversalLimitKind, Asset, AssetId, AvailabilityIssue,
    AvailabilityIssueKind, CommitReceipt, DecimalValue, DecisionBase, Dependency, DependencyKind,
    DependencyQueryLimits, DependencySet, DependencySetStatus, DependencyTarget, EvidenceKind,
    ExternalIdentifier, FrameRange, IdentifierScheme, Job, JobClaimId, JobFailure, JobId, JobKind,
    JobQuery, JobState, JobStateKind, Locator, LocatorAvailability, LocatorId, LocatorIdentity,
    MAX_JOB_DIAGNOSTIC_BYTES, MediaRoot, MediaRootId, MetadataAssertion, MetadataField,
    MetadataProperty, MetadataQuery, MetadataValue, MetadataValueKind, ObjectRef, OriginIdentity,
    OriginalMediaImport, ProductionId, ProductionStoreTransaction, PropertyId,
    ProvenanceQueryLimits, QueryCursor, QueryPage, QueryPageRequest, RationalRate, RationalValue,
    Representation, RepresentationAvailability, RepresentationId, RepresentationKind,
    RepresentationResolution, RequestedJobOutput, ResolutionEvidence, Resource,
    ResourceFingerprint, ResourceId, ResourceResolution, ResourceResolutionState, ResourceRole,
    Revision, RevisionContext, RevisionEvent, RevisionEventFilter, RevisionEventKind,
    RevisionEventType, RevisionId, RevisionWaitOutcome, SemanticConflictKey, SequenceNaming,
    StaleArtifactQuery, Timestamp, ToolIdentity, TransactionConflict, VocabularyId,
};
use postproject_media::{
    ContentObservationOutcome, ContentVerification, EXECUTOR_PARAMETER_VOCABULARY,
    EXECUTOR_PROFILE_PROPERTY, ExecutionOutcome, ExecutionRequest, Executor, ExecutorCapability,
    FfmpegExecutor, FfprobeInspector, FileResourceSource, ImageSequenceSource, InspectionOutcome,
    InventoryCategory, InventoryReport, InventoryScanner, MediaInspector, MediaRecognizer,
    MediaResolver, MediaRootMapping, MediaSource, RecognizedMedia, ResolutionItem, ResolverOptions,
    SearchScope, TechnicalMetadata, VerificationMode, local_file_path, observe_resource_content,
    prepare_confirmed_locator, prepare_original_media, prepare_recognized_original_media,
    prepare_representation, recorded_sequence_naming, resource_usage, verify_resource_content,
};
use postproject_storage_sqlite::{SqliteProduction, SqliteTransaction};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
enum CliDecisionBase {
    Revision(RevisionId),
    Scoped(DecisionBase),
}

#[derive(Debug, Parser)]
#[command(name = "postproject", version, about)]
struct Cli {
    /// Emit results and structured conflicts as JSON on stdout.
    #[arg(long, global = true)]
    json: bool,

    /// Revision on which this command's writes are based.
    #[arg(long, global = true, value_name = "REVISION_ID")]
    base_revision: Option<RevisionId>,

    /// Production-scoped token returned by inspect; retains no read lock.
    #[arg(
        long,
        global = true,
        conflicts_with = "base_revision",
        value_name = "TOKEN"
    )]
    decision_base: Option<DecisionBase>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Read a coherent bounded view and emit a detached decision token.
    Inspect(ProductionQueryArgs),
    /// Create a production file.
    Init(InitArgs),
    /// Inspect and manage production media.
    Media(MediaArgs),
    /// Add representations to existing assets.
    Representation(RepresentationArgs),
    /// Manage resolver search roots.
    Root(RootArgs),
    /// Manage resource locators.
    Locator(LocatorArgs),
    /// Manage external industry, vendor, and application identifiers.
    Identifier(IdentifierArgs),
    /// Inspect and manage standards-aware metadata assertions.
    Metadata(MetadataArgs),
    /// Inspect production provenance activities.
    Activity(ActivityArgs),
    /// Inspect representation dependencies.
    Dependency(DependencyArgs),
    /// Evaluate managed artifacts from recorded production knowledge.
    Artifact(ArtifactArgs),
    /// Request and inspect durable production work.
    Job(JobArgs),
    /// Inspect the durable semantic change journal.
    Revisions(RevisionsArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Production file to create.
    production: PathBuf,
    /// Optional production display name.
    #[arg(long)]
    name: Option<String>,
}

#[derive(Debug, Args)]
struct MediaArgs {
    #[command(subcommand)]
    command: MediaCommand,
}

#[derive(Debug, Subcommand)]
enum MediaCommand {
    /// Import an original media file.
    Add(MediaAddArgs),
    /// List logical media assets; `--limit` or `--cursor` returns one page.
    List(MediaListArgs),
    /// Show an asset, its representations, resources, and locators.
    Show(MediaAssetArgs),
    /// Resolve an asset under configured media roots.
    Resolve(MediaResolveArgs),
    /// Inventory known and unassociated media without changing the production.
    Inventory(MediaInventoryArgs),
    /// Record a resource's present content as a new fingerprint observation.
    ///
    /// Every representation using the resource is recomputed in the same
    /// revision.
    Fingerprint(MediaFingerprintArgs),
    /// Compare present content with a resource's stored fingerprints.
    ///
    /// Read-only. Prints `matches`, `differs`, or `not_comparable` when no
    /// stored fingerprint lies in a domain the library computes.
    VerifyContent(MediaFingerprintArgs),
    /// Query representations with a required resource that has no durable locator.
    ///
    /// Knowledge-only: no filesystem path is checked. Use `media resolve` or
    /// `media inventory` for current filesystem state.
    Unresolved(ProductionQueryArgs),
    /// Query representations with a locator recorded under a logical media root.
    ///
    /// Knowledge-only: only locators confirmed with root knowledge match.
    UnderRoot(MediaUnderRootArgs),
    /// Find current production media by exact locator identity.
    FindByLocator(MediaFindByLocatorArgs),
    /// Find current production media by exact fingerprint evidence.
    FindByFingerprint(MediaFindByFingerprintArgs),
}

#[derive(Debug, Args)]
struct MediaListArgs {
    production: PathBuf,
    /// Maximum assets returned in one page; opts into paginated output.
    #[arg(long)]
    limit: Option<u32>,
    /// Opaque continuation returned by the preceding page; opts into
    /// paginated output.
    #[arg(long)]
    cursor: Option<String>,
}

#[derive(Debug, Args)]
struct ProductionQueryArgs {
    production: PathBuf,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct MediaUnderRootArgs {
    production: PathBuf,
    /// Logical media root name.
    root: String,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct MediaFindByLocatorArgs {
    production: PathBuf,
    /// Canonical locator URI to match.
    uri: String,
    /// Exact image-sequence naming, such as `shot_%04d.exr`.
    #[arg(long, value_name = "PATTERN")]
    sequence_naming: Option<SequenceNamingArg>,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct MediaFindByFingerprintArgs {
    production: PathBuf,
    /// Fingerprint algorithm/domain.
    algorithm: String,
    /// Fingerprint algorithm version.
    version: u16,
    /// Fingerprint value encoded as hexadecimal.
    value_hex: String,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct MediaAddArgs {
    production: PathBuf,
    path: PathBuf,
    /// Optional asset display name.
    #[arg(long)]
    name: Option<String>,
    /// Exact rate for a recognized image sequence (NUMERATOR/DENOMINATOR).
    #[arg(long)]
    sequence_rate: Option<RationalRate>,
    /// Recognize same-stem metadata sidecars beside a regular file.
    #[arg(long)]
    recognize_companions: bool,
    /// Inspect imported media with ffprobe and record technical metadata.
    #[arg(long)]
    inspect: bool,
    /// ffprobe executable used with --inspect.
    #[arg(long, default_value = "ffprobe", requires = "inspect")]
    ffprobe: PathBuf,
}

#[derive(Debug, Args)]
struct ProductionArgs {
    production: PathBuf,
}

#[derive(Debug, Args)]
struct MediaAssetArgs {
    production: PathBuf,
    asset_id: String,
}

#[derive(Debug, Args)]
struct MediaResolveArgs {
    production: PathBuf,
    asset_id: String,
    /// Confirm one URI returned by this resolution and persist it. An
    /// image-sequence candidate is recorded with the file naming it was found
    /// under.
    #[arg(long, value_name = "URI")]
    confirm: Option<String>,
    /// Choose, by its naming (such as `shot_%04d.png`), which image-sequence
    /// candidate at the confirmed URI to record.
    #[arg(long, value_name = "PATTERN", requires = "confirm")]
    confirm_naming: Option<SequenceNamingArg>,
    /// Map a production root name to this machine's directory (NAME=PATH).
    #[arg(long = "root-map", value_name = "NAME=PATH")]
    root_mappings: Vec<RootMappingArg>,
    /// Also search this unnamed directory, after the mapped roots. Search
    /// directories are never recorded in the production.
    #[arg(long = "search-dir", value_name = "PATH")]
    search_directories: Vec<PathBuf>,
    /// Recompute stored fingerprints for content at known locators.
    #[arg(long)]
    verify: bool,
    /// ffprobe executable used to compare persisted technical metadata.
    #[arg(long, default_value = "ffprobe", requires = "verify")]
    ffprobe: PathBuf,
}

#[derive(Debug, Args)]
struct MediaInventoryArgs {
    production: PathBuf,
    /// Map a production root name to this machine's directory (NAME=PATH).
    #[arg(long = "root-map", value_name = "NAME=PATH")]
    root_mappings: Vec<RootMappingArg>,
    /// Machine-local disposable sidecar cache.
    #[arg(long, value_name = "PATH")]
    cache: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct MediaFingerprintArgs {
    production: PathBuf,
    resource_id: String,
    /// File, or image-sequence directory, whose content realizes the resource.
    path: PathBuf,
    /// Naming of the image-sequence files in the directory, such as
    /// `shot_%04d.png`. Defaults to the naming recorded for the directory.
    #[arg(long, value_name = "PATTERN")]
    sequence_naming: Option<SequenceNamingArg>,
}

/// An image-sequence file naming written as a printf-style pattern: the
/// prefix, `%d` or `%0Nd` for a frame number padded to N digits, and the
/// suffix.
#[derive(Clone, Debug)]
struct SequenceNamingArg(SequenceNaming);

impl FromStr for SequenceNamingArg {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        parse_sequence_naming(value).map(Self)
    }
}

fn parse_sequence_naming(value: &str) -> std::result::Result<SequenceNaming, String> {
    for (start, _) in value.rmatch_indices('%') {
        let rest = &value[start + 1..];
        let Some(end) = rest.find('d') else {
            continue;
        };
        let width = &rest[..end];
        if !width.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let padding = if width.is_empty() {
            0
        } else if width.starts_with('0') {
            width
                .parse::<u8>()
                .map_err(|error| format!("invalid frame padding in {value:?}: {error}"))?
        } else {
            continue;
        };
        return SequenceNaming::new(&value[..start], &rest[end + 1..], padding)
            .map_err(|error| error.to_string());
    }
    Err(format!(
        "sequence naming {value:?} must contain %d or %0Nd for the frame number"
    ))
}

#[derive(Debug, Args)]
struct RepresentationArgs {
    #[command(subcommand)]
    command: RepresentationCommand,
}

#[derive(Debug, Subcommand)]
enum RepresentationCommand {
    /// Add a representation described by a JSON specification.
    Add(RepresentationAddArgs),
    /// Query one page of an asset's representations in identity order.
    List(RepresentationListArgs),
    /// Query one page of a representation's resources in structural order.
    Resources(RepresentationResourcesArgs),
    /// Show one representation by identity.
    Show(RepresentationShowArgs),
    /// Query one page of the representations that use a resource.
    UsingResource(RepresentationUsingResourceArgs),
}

#[derive(Debug, Args)]
struct RepresentationShowArgs {
    production: PathBuf,
    representation_id: String,
}

#[derive(Debug, Args)]
struct RepresentationUsingResourceArgs {
    production: PathBuf,
    resource_id: String,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct RepresentationListArgs {
    production: PathBuf,
    asset_id: String,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct RepresentationResourcesArgs {
    production: PathBuf,
    representation_id: String,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct RepresentationAddArgs {
    production: PathBuf,
    asset_id: String,
    #[arg(value_enum)]
    kind: RepresentationKindArg,
    /// JSON file containing a tagged representation source.
    spec_file: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RepresentationKindArg {
    Original,
    Proxy,
    Optimized,
    Derived,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "structure", rename_all = "snake_case")]
enum RepresentationSourceSpec {
    SingleFile {
        path: PathBuf,
    },
    ImageSequence {
        directory: PathBuf,
        prefix: String,
        suffix: String,
        padding: u8,
        start: i64,
        end: i64,
        step: u32,
        rate_numerator: u32,
        rate_denominator: u32,
        #[serde(default)]
        missing_frames: Vec<i64>,
    },
    OrderedParts {
        members: Vec<FileResourceSpec>,
    },
    Package {
        members: Vec<FileResourceSpec>,
    },
}

#[derive(Debug, Deserialize)]
struct FileResourceSpec {
    path: PathBuf,
    role: String,
    #[serde(default = "default_required")]
    required: bool,
}

const fn default_required() -> bool {
    true
}

#[derive(Debug, Args)]
struct RootArgs {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    /// Add a logical media root to the production.
    Add(RootAddArgs),
    /// List up to 1000 current media roots in resolver order; use page for more.
    List(ProductionArgs),
    /// Page current configured roots in resolver order.
    Page(RootPageArgs),
    /// Include a media root in resolution; requires --decision-base from inspect.
    Enable(RootMutationArgs),
    /// Exclude a media root; requires --decision-base from inspect.
    Disable(RootMutationArgs),
    /// Remove a configured media root; requires --decision-base from inspect.
    Remove(RootMutationArgs),
}

#[derive(Debug, Args)]
struct RootPageArgs {
    production: PathBuf,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct RootAddArgs {
    production: PathBuf,
    /// Portable logical name used by machine-local root mappings.
    name: String,
    /// Optional human-readable description of the root.
    #[arg(long)]
    label: Option<String>,
    /// Lower priorities are searched first.
    #[arg(long, default_value_t = 0)]
    priority: i32,
}

#[derive(Debug, Args)]
struct RootMutationArgs {
    production: PathBuf,
    root_id: String,
}

#[derive(Clone, Debug)]
struct RootMappingArg {
    name: String,
    directory: PathBuf,
}

impl FromStr for RootMappingArg {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let (name, directory) = value
            .split_once('=')
            .ok_or_else(|| "root mapping must use NAME=PATH".to_owned())?;
        if name.is_empty() || directory.is_empty() {
            return Err("root mapping name and path must not be empty".to_owned());
        }
        Ok(Self {
            name: name.to_owned(),
            directory: PathBuf::from(directory),
        })
    }
}

#[derive(Debug, Args)]
struct LocatorArgs {
    #[command(subcommand)]
    command: LocatorCommand,
}

#[derive(Debug, Subcommand)]
enum LocatorCommand {
    /// Retire a locator that no longer identifies a useful access route.
    Retire(LocatorRetireArgs),
    /// Query one page of a resource's known locators in identity order.
    List(LocatorListArgs),
}

#[derive(Debug, Args)]
struct LocatorListArgs {
    production: PathBuf,
    resource_id: String,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct LocatorRetireArgs {
    production: PathBuf,
    locator_id: String,
}

#[derive(Debug, Args)]
struct IdentifierArgs {
    #[command(subcommand)]
    command: IdentifierCommand,
}

#[derive(Debug, Subcommand)]
enum IdentifierCommand {
    /// Attach an external identifier to an asset, representation, or resource.
    Add(IdentifierMutationArgs),
    /// Remove one exact external identifier attachment.
    Remove(IdentifierMutationArgs),
    /// List external identifiers attached to an object.
    List(IdentifierTargetArgs),
    /// Find objects carrying an exact scheme and value, optionally with an
    /// exact qualifier.
    Find(IdentifierFindArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum IdentifierTargetKind {
    Asset,
    Representation,
    Resource,
}

#[derive(Debug, Args)]
struct IdentifierTargetArgs {
    production: PathBuf,
    #[arg(value_enum)]
    target_kind: IdentifierTargetKind,
    target_id: String,
}

#[derive(Debug, Args)]
struct IdentifierMutationArgs {
    #[command(flatten)]
    target: IdentifierTargetArgs,
    scheme: String,
    value: String,
    #[arg(long)]
    qualifier: Option<String>,
}

#[derive(Debug, Args)]
struct IdentifierFindArgs {
    production: PathBuf,
    scheme: String,
    value: String,
    /// Match only identifiers with exactly this qualifier.
    #[arg(long)]
    qualifier: Option<String>,
}

#[derive(Debug, Args)]
struct MetadataArgs {
    #[command(subcommand)]
    command: MetadataCommand,
}

#[derive(Debug, Subcommand)]
enum MetadataCommand {
    /// Append a typed value read from a JSON file.
    Add(MetadataAddArgs),
    /// Append a plain or language-tagged text value.
    AddText(MetadataAddTextArgs),
    /// List all metadata assertions attached to an object.
    List(MetadataTargetArgs),
    /// Remove a property's values using --decision-base from inspect.
    Remove(MetadataPropertyArgs),
    /// Query objects carrying an exact vocabulary and property.
    Find(MetadataFindArgs),
}

#[derive(Clone, Copy, Debug, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum MetadataTargetKind {
    Production,
    Asset,
    Representation,
    Resource,
    Activity,
}

#[derive(Debug, Args)]
struct MetadataTargetArgs {
    production: PathBuf,
    #[arg(value_enum)]
    target_kind: MetadataTargetKind,
    target_id: String,
}

#[derive(Debug, Args)]
struct MetadataPropertyArgs {
    #[command(flatten)]
    target: MetadataTargetArgs,
    vocabulary: String,
    property: String,
}

#[derive(Debug, Args)]
struct MetadataAddTextArgs {
    #[command(flatten)]
    target: MetadataTargetArgs,
    vocabulary: String,
    property: String,
    value: String,
    /// Optional BCP 47-shaped language tag.
    #[arg(long)]
    language: Option<String>,
}

#[derive(Debug, Args)]
struct MetadataAddArgs {
    #[command(flatten)]
    target: MetadataTargetArgs,
    vocabulary: String,
    property: String,
    /// JSON file containing one tagged metadata value.
    value_file: PathBuf,
}

#[derive(Debug, Args)]
struct MetadataFindArgs {
    production: PathBuf,
    vocabulary: String,
    property: String,
    /// JSON file containing one tagged scalar value that must match exactly.
    #[arg(long, value_name = "PATH")]
    value_file: Option<PathBuf>,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct ActivityArgs {
    #[command(subcommand)]
    command: ActivityCommand,
}

#[derive(Debug, Subcommand)]
enum ActivityCommand {
    /// Record a completed production activity.
    Add(Box<ActivityAddArgs>),
    /// List production activities with their inputs and outputs.
    List(ProductionArgs),
    /// Query activities that produced a representation.
    Producing(ActivityRepresentationQueryArgs),
    /// Query activities that consume a representation.
    Consuming(ActivityRepresentationQueryArgs),
    /// Query bounded provenance ancestors with their shortest depth.
    Ancestors(ProvenanceQueryArgs),
    /// Query bounded provenance descendants with their shortest depth.
    Descendants(ProvenanceQueryArgs),
    /// Query representations produced by an activity kind or exact tool.
    Outputs(ActivityOutputsArgs),
}

#[derive(Debug, Args)]
struct ActivityRepresentationQueryArgs {
    production: PathBuf,
    representation_id: String,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct ProvenanceQueryArgs {
    production: PathBuf,
    representation_id: String,
    /// Maximum number of activity steps to traverse.
    #[arg(long, default_value_t = 64)]
    max_depth: u32,
    /// Maximum distinct representations to traverse.
    #[arg(long, default_value_t = 1_000)]
    max_representations: u32,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
#[command(group(
    clap::ArgGroup::new("selector")
        .required(true)
        .args(["kind", "tool_name"])
))]
struct ActivityOutputsArgs {
    production: PathBuf,
    /// Exact namespaced activity kind.
    #[arg(long)]
    kind: Option<String>,
    /// Exact tool name; the complete tool identity must match.
    #[arg(long)]
    tool_name: Option<String>,
    /// Exact tool version; absent matches only tools without a version.
    #[arg(long, requires = "tool_name")]
    tool_version: Option<String>,
    /// Exact tool URI; absent matches only tools without a URI.
    #[arg(long, requires = "tool_name")]
    tool_uri: Option<String>,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct ActivityAddArgs {
    production: PathBuf,
    /// Namespaced activity kind, such as `org.postproject:transcode`.
    kind: String,
    /// Consumed representation, optionally followed by `=ROLE`.
    #[arg(long = "input", value_name = "REPRESENTATION_ID[=ROLE]")]
    inputs: Vec<ActivityEdgeArg>,
    /// Produced representation, optionally followed by `=ROLE`.
    #[arg(
        long = "output",
        value_name = "REPRESENTATION_ID[=ROLE]",
        required = true
    )]
    outputs: Vec<ActivityEdgeArg>,
    /// Optional activity start as Unix microseconds.
    #[arg(long)]
    started_at_unix_micros: Option<i64>,
    /// Optional activity finish as Unix microseconds.
    #[arg(long)]
    finished_at_unix_micros: Option<i64>,
    #[arg(long)]
    tool_name: Option<String>,
    #[arg(long)]
    tool_version: Option<String>,
    #[arg(long)]
    tool_uri: Option<String>,
    #[arg(long)]
    agent_name: Option<String>,
    #[arg(long)]
    agent_identifier_scheme: Option<String>,
    #[arg(long)]
    agent_identifier_value: Option<String>,
    #[arg(long)]
    agent_identifier_qualifier: Option<String>,
}

#[derive(Debug, Args)]
struct ActivityRepresentationArgs {
    production: PathBuf,
    representation_id: String,
}

#[derive(Debug, Args)]
struct DependencyArgs {
    #[command(subcommand)]
    command: DependencyCommand,
}

#[derive(Debug, Subcommand)]
enum DependencyCommand {
    /// Replace one representation's complete dependency observation from JSON.
    Record(DependencyRecordArgs),
    /// Show one representation's complete dependency observation.
    Show(ActivityRepresentationArgs),
    /// Query direct or transitive dependencies of a representation.
    Dependencies(DependencyRepresentationQueryArgs),
    /// Query representations that depend on an asset or representation.
    Dependents(DependencyTargetArgs),
}

/// Page size used when a query command is not given `--limit`.
const DEFAULT_QUERY_PAGE_SIZE: u32 = 100;

#[derive(Debug, Args)]
struct QueryPageArgs {
    /// Maximum items returned in this page.
    #[arg(long, default_value_t = DEFAULT_QUERY_PAGE_SIZE)]
    limit: u32,
    /// Opaque continuation returned by the preceding page.
    #[arg(long)]
    cursor: Option<String>,
}

#[derive(Debug, Args)]
struct DependencyQueryArgs {
    /// Maximum dependency-edge depth to traverse.
    #[arg(long, default_value_t = 1)]
    max_depth: u32,
    /// Maximum distinct representations to traverse.
    #[arg(long, default_value_t = 1_000)]
    max_representations: u32,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct DependencyRepresentationQueryArgs {
    production: PathBuf,
    representation_id: String,
    #[command(flatten)]
    query: DependencyQueryArgs,
}

#[derive(Debug, Args)]
struct DependencyRecordArgs {
    production: PathBuf,
    representation_id: String,
    /// JSON file containing an ordered array of dependency edges.
    spec_file: PathBuf,
}

#[derive(Debug, Deserialize)]
struct DependencySpec {
    source_resource_id: Option<String>,
    kind: String,
    target: DependencyTargetSpec,
    resolved_representation_id: Option<String>,
    #[serde(default = "default_required")]
    required: bool,
    authored_reference: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DependencyTargetSpec {
    Asset { id: String },
    Representation { id: String },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DependencyTargetKind {
    Asset,
    Representation,
}

#[derive(Debug, Args)]
struct DependencyTargetArgs {
    production: PathBuf,
    #[arg(value_enum)]
    target_kind: DependencyTargetKind,
    target_id: String,
    #[command(flatten)]
    query: DependencyQueryArgs,
}

#[derive(Debug, Args)]
struct ArtifactArgs {
    #[command(subcommand)]
    command: ArtifactCommand,
}

#[derive(Debug, Subcommand)]
enum ArtifactCommand {
    /// Evaluate whether an activity-produced representation is current.
    Evaluate(ArtifactEvaluateArgs),
    /// Report whether stored knowledge can reproduce an artifact.
    Reproducibility(ActivityRepresentationArgs),
    /// Query activity-produced representations currently evaluated as stale.
    Stale(ArtifactStaleArgs),
}

#[derive(Debug, Args)]
struct ArtifactStaleArgs {
    production: PathBuf,
    /// Restrict candidates to provenance descendants of this representation.
    #[arg(long, value_name = "REPRESENTATION_ID")]
    source: Option<String>,
    /// Maximum number of upstream activity edges followed per artifact.
    #[arg(long, default_value_t = 64)]
    max_depth: u32,
    /// Maximum number of distinct representations inspected per artifact.
    #[arg(long, default_value_t = 1_000)]
    max_representations: u32,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct ArtifactEvaluateArgs {
    production: PathBuf,
    representation_id: String,
    /// Maximum number of upstream activity edges to follow.
    #[arg(long, default_value_t = 64)]
    max_depth: u32,
    /// Maximum number of distinct representations to inspect.
    #[arg(long, default_value_t = 1_000)]
    max_representations: u32,
}

#[derive(Debug, Args)]
struct JobArgs {
    #[command(subcommand)]
    command: JobCommand,
}

#[derive(Debug, Subcommand)]
enum JobCommand {
    /// Request durable production work.
    Request(JobRequestArgs),
    /// Atomically claim requested work with a caller-supplied lease.
    Claim(JobClaimArgs),
    /// Renew an active job claim.
    Renew(JobLeaseArgs),
    /// Release an active job claim back to requested state.
    Release(JobClaimTokenArgs),
    /// Complete a claimed job with one single-file output and its activity.
    Complete(JobCompleteArgs),
    /// Mark an actively claimed job as failed.
    Fail(JobFailArgs),
    /// Administratively cancel requested or claimed work.
    Cancel(JobIdArgs),
    /// Show one durable job by identity.
    Show(JobIdArgs),
    /// List durable jobs in stable identity order.
    List(JobListArgs),
    /// Derive non-persisted jobs that would regenerate artifacts.
    Plan(JobPlanArgs),
    /// Claim and execute eligible proxy or thumbnail jobs with ffmpeg.
    Run(JobRunArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum JobStateArg {
    Requested,
    Claimed,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Args)]
struct JobListArgs {
    production: PathBuf,
    #[arg(long, value_enum)]
    state: Option<JobStateArg>,
    #[arg(long)]
    kind: Option<String>,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct JobRequestArgs {
    production: PathBuf,
    /// Open-world namespaced job kind.
    kind: String,
    output_asset_id: String,
    #[arg(value_enum)]
    output_kind: RepresentationKindArg,
    /// Input representation ID; repeat for multiple inputs.
    #[arg(long = "input")]
    inputs: Vec<String>,
    /// Optional logical media-root name preferred for the output.
    #[arg(long)]
    target_root: Option<String>,
    /// Named reference-executor profile to persist as typed job metadata.
    #[arg(long)]
    profile: Option<String>,
}

#[derive(Debug, Args)]
struct JobRunArgs {
    production: PathBuf,
    /// Stop after one eligible job reaches a terminal state.
    #[arg(long)]
    once: bool,
    /// Map a production root name to this machine's directory (NAME=PATH).
    #[arg(long = "root-map", value_name = "NAME=PATH")]
    root_mappings: Vec<RootMappingArg>,
    /// ffmpeg executable used by the reference executor.
    #[arg(long, default_value = "ffmpeg")]
    ffmpeg: PathBuf,
    /// Maximum runtime for one ffmpeg process.
    #[arg(long, default_value_t = 7_200)]
    timeout_seconds: u64,
    /// Claim lease duration; the runner renews it at one-third intervals.
    #[arg(long, default_value_t = 60)]
    lease_seconds: u64,
}

#[derive(Debug, Args)]
struct JobClaimArgs {
    production: PathBuf,
    job_id: String,
    #[arg(long)]
    tool_name: String,
    #[arg(long)]
    tool_version: Option<String>,
    #[arg(long)]
    tool_uri: Option<String>,
    #[arg(long)]
    agent_name: Option<String>,
    #[arg(long)]
    agent_identifier_scheme: Option<String>,
    #[arg(long)]
    agent_identifier_value: Option<String>,
    #[arg(long)]
    agent_identifier_qualifier: Option<String>,
    #[arg(long)]
    now_unix_micros: i64,
    #[arg(long)]
    expires_at_unix_micros: i64,
}

#[derive(Debug, Args)]
struct JobLeaseArgs {
    production: PathBuf,
    job_id: String,
    claim_id: String,
    #[arg(long)]
    now_unix_micros: i64,
    #[arg(long)]
    expires_at_unix_micros: i64,
}

#[derive(Debug, Args)]
struct JobClaimTokenArgs {
    production: PathBuf,
    job_id: String,
    claim_id: String,
}

#[derive(Debug, Args)]
struct JobCompleteArgs {
    production: PathBuf,
    job_id: String,
    claim_id: String,
    /// Existing output file to fingerprint and record.
    output: PathBuf,
    #[arg(long)]
    now_unix_micros: i64,
}

#[derive(Debug, Args)]
struct JobFailArgs {
    production: PathBuf,
    job_id: String,
    claim_id: String,
    diagnostic: String,
    #[arg(long)]
    now_unix_micros: i64,
}

#[derive(Debug, Args)]
struct JobIdArgs {
    production: PathBuf,
    job_id: String,
}

#[derive(Debug, Args)]
struct JobPlanArgs {
    production: PathBuf,
    /// Artifact representation ID; repeat to plan multiple artifacts.
    #[arg(long = "artifact", required = true)]
    artifacts: Vec<String>,
}

#[derive(Debug, Args)]
struct RevisionsArgs {
    #[command(subcommand)]
    command: RevisionsCommand,
}

#[derive(Debug, Subcommand)]
enum RevisionsCommand {
    /// Show the newest committed revision.
    Latest(ProductionArgs),
    /// List revisions after a production-local sequence cursor.
    Since(RevisionsSinceArgs),
    /// List the ordered semantic events belonging to one revision.
    Events(RevisionEventsArgs),
    /// Query distinct objects touched after a revision sequence.
    Changed(RevisionsChangedArgs),
    /// List revisions after a cursor that contain one of the given event kinds.
    Filtered(RevisionsFilteredArgs),
    /// Wait for revisions after a cursor, including commits by other processes.
    Wait(RevisionsWaitArgs),
}

#[derive(Debug, Args)]
struct RevisionsFilteredArgs {
    production: PathBuf,
    /// Return matching revisions with a sequence greater than this cursor.
    #[arg(long, default_value_t = 0)]
    after: u64,
    /// Event kind to match, such as `job_succeeded`; repeat for several.
    #[arg(long = "kind", required = true, value_parser = parse_revision_event_type)]
    kinds: Vec<RevisionEventType>,
    /// Maximum number of revisions to return.
    #[arg(long, default_value_t = 100)]
    limit: u32,
}

#[derive(Debug, Args)]
struct RevisionsWaitArgs {
    production: PathBuf,
    /// Wait for revisions after this sequence; defaults to the latest revision.
    #[arg(long)]
    after: Option<u64>,
    /// Maximum number of revisions to return.
    #[arg(long, default_value_t = 100)]
    limit: u32,
    /// Longest wait in milliseconds, at most 60000; zero checks once.
    #[arg(long, default_value_t = 60_000)]
    timeout_ms: u64,
}

#[derive(Debug, Args)]
struct RevisionsChangedArgs {
    production: PathBuf,
    /// Include objects touched by revisions with a sequence greater than this.
    #[arg(long, default_value_t = 0)]
    after: u64,
    #[command(flatten)]
    page: QueryPageArgs,
}

#[derive(Debug, Args)]
struct RevisionsSinceArgs {
    production: PathBuf,
    /// Return revisions with a sequence greater than this cursor.
    #[arg(long, default_value_t = 0)]
    after: u64,
    /// Maximum number of revisions to return.
    #[arg(long, default_value_t = 100)]
    limit: u32,
}

#[derive(Debug, Args)]
struct RevisionEventsArgs {
    production: PathBuf,
    revision_id: String,
}

#[derive(Clone, Debug)]
struct ActivityEdgeArg {
    representation_id: RepresentationId,
    role: Option<ActivityRole>,
}

impl FromStr for ActivityEdgeArg {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let (representation_id, role) = value
            .split_once('=')
            .map_or((value, None), |(id, role)| (id, Some(role)));
        Ok(Self {
            representation_id: RepresentationId::from_str(representation_id)
                .map_err(|error| format!("invalid representation ID: {error}"))?,
            role: role
                .map(ActivityRole::new)
                .transpose()
                .map_err(|error| format!("invalid activity role: {error}"))?,
        })
    }
}

#[derive(Debug, Serialize)]
struct ProductionView {
    id: String,
    path: String,
    schema_version: u32,
    display_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct ImportView {
    asset_id: String,
    representation_id: String,
    resource_id: String,
    locator_id: String,
    uri: String,
    resource_count: usize,
    inspections: Vec<InspectionView>,
}

#[derive(Debug, Serialize)]
struct InspectionView {
    path: String,
    status: &'static str,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct AssetSummary {
    id: String,
    display_name: Option<String>,
    created_at_unix_micros: i64,
    representation_count: usize,
}

#[derive(Debug, Serialize)]
struct AssetView {
    id: String,
    display_name: Option<String>,
    import_source: Option<String>,
    created_at_unix_micros: i64,
    representations: Vec<RepresentationView>,
}

#[derive(Debug, Serialize)]
struct RepresentationView {
    id: String,
    kind: &'static str,
    structure: &'static str,
    resources: Vec<ResourceView>,
}

#[derive(Debug, Serialize)]
struct ResourceView {
    id: String,
    fingerprints: Vec<FingerprintView>,
    file_size_bytes: Option<u64>,
    locators: Vec<LocatorView>,
}

#[derive(Debug, Serialize)]
struct FingerprintView {
    algorithm: String,
    version: u16,
    value_hex: String,
}

#[derive(Debug, Serialize)]
struct FingerprintObservationView {
    resource_id: String,
    outcome: &'static str,
    resource_fingerprint: FingerprintView,
    representation_fingerprints: Vec<RepresentationFingerprintView>,
}

#[derive(Debug, Serialize)]
struct RepresentationFingerprintView {
    representation_id: String,
    fingerprint: FingerprintView,
}

#[derive(Debug, Serialize)]
struct ContentVerificationView {
    resource_id: String,
    verification: &'static str,
}

#[derive(Debug, Serialize)]
struct LocatorView {
    id: String,
    uri: String,
    availability: &'static str,
    last_seen_unix_micros: Option<i64>,
    sequence_naming: Option<SequenceNamingView>,
}

#[derive(Debug, Serialize)]
struct SequenceNamingView {
    prefix: String,
    suffix: String,
    padding: u8,
    pattern: String,
}

impl From<&SequenceNaming> for SequenceNamingView {
    fn from(naming: &SequenceNaming) -> Self {
        Self {
            prefix: naming.prefix().to_owned(),
            suffix: naming.suffix().to_owned(),
            padding: naming.padding(),
            pattern: naming_pattern(naming),
        }
    }
}

/// Formats a naming as the printf-style pattern `--confirm-naming` accepts.
fn naming_pattern(naming: &SequenceNaming) -> String {
    let frame = if naming.padding() == 0 {
        "%d".to_owned()
    } else {
        format!("%0{}d", naming.padding())
    };
    format!("{}{frame}{}", naming.prefix(), naming.suffix())
}

#[derive(Debug, Serialize)]
struct RootView {
    id: String,
    name: String,
    label: Option<String>,
    legacy_uri: Option<String>,
    priority: i32,
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct ResolveView {
    asset_id: String,
    resolutions: Vec<ResolutionView>,
    confirmed_uri: Option<String>,
    confirmed_sequence_naming: Option<SequenceNamingView>,
}

#[derive(Debug, Serialize)]
struct InventoryView {
    items: Vec<InventoryItemView>,
    stats: InventoryStatsView,
}

#[derive(Debug, Serialize)]
struct InventoryItemView {
    category: &'static str,
    representation_id: Option<String>,
    resource_id: Option<String>,
    uri: Option<String>,
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct InventoryStatsView {
    entries_visited: usize,
    fingerprints_computed: usize,
    fingerprint_cache_hits: usize,
    cache_rebuilt: bool,
}

#[derive(Debug, Serialize)]
struct ResolutionView {
    representation_id: String,
    availability: &'static str,
    resources: Vec<ResourceResolutionView>,
    issues: Vec<AvailabilityIssueView>,
}

#[derive(Debug, Serialize)]
struct ResourceResolutionView {
    resource_id: String,
    state: &'static str,
    candidates: Vec<CandidateView>,
    evidence: Vec<EvidenceView>,
}

#[derive(Debug, Serialize)]
struct AvailabilityIssueView {
    resource_id: String,
    required: bool,
    kind: &'static str,
    frames: Vec<i64>,
}

#[derive(Debug, Serialize)]
struct CandidateView {
    uri: String,
    confidence_basis_points: u16,
    media_root: Option<String>,
    sequence_naming: Option<SequenceNamingView>,
    evidence: Vec<EvidenceView>,
}

#[derive(Debug, Serialize)]
struct EvidenceView {
    kind: &'static str,
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorView {
    error: ErrorDetailView,
}

#[derive(Debug, Serialize)]
struct ErrorDetailView {
    message: String,
    transaction_conflict: TransactionConflictView,
}

#[derive(Debug, Serialize)]
struct TransactionConflictView {
    key: ConflictKeyView,
    base_revision_id: Option<String>,
    base_revision_sequence: u64,
    superseding_revision_id: String,
    superseding_revision_sequence: u64,
}

#[derive(Debug, Serialize)]
struct ConflictKeyView {
    kind: &'static str,
    target_kind: &'static str,
    target_id: String,
    namespace_name: Option<String>,
    local_name: Option<String>,
    qualifier: Option<String>,
    version: Option<u16>,
}

#[derive(Debug, Serialize)]
struct ExternalIdentifierView {
    target_kind: &'static str,
    target_id: String,
    scheme: String,
    value: String,
    qualifier: Option<String>,
}

#[derive(Debug, Serialize)]
struct ObjectRefView {
    kind: &'static str,
    id: String,
}

#[derive(Debug, Serialize)]
struct MetadataAssertionView {
    target_kind: &'static str,
    target_id: String,
    vocabulary: String,
    property: String,
    value: MetadataValueView,
}

#[derive(Debug, Serialize)]
struct MetadataPropertyView {
    target_kind: &'static str,
    target_id: String,
    vocabulary: String,
    property: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MetadataValueView {
    String { value: String },
    LangString { value: String, language: String },
    I64 { value: i64 },
    U64 { value: u64 },
    Decimal { coefficient: String, scale: u32 },
    Bool { value: bool },
    Timestamp { unix_micros: i64 },
    Uri { value: String },
    Bytes { hex: String },
    Rational { numerator: i64, denominator: u64 },
    List { values: Vec<MetadataValueView> },
    Struct { fields: Vec<MetadataFieldView> },
    Reference { target: ObjectRefView },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MetadataValueInput {
    String { value: String },
    LangString { value: String, language: String },
    I64 { value: i64 },
    U64 { value: u64 },
    Decimal { coefficient: String, scale: u32 },
    Bool { value: bool },
    Timestamp { unix_micros: i64 },
    Uri { value: String },
    Bytes { hex: String },
    Rational { numerator: i64, denominator: u64 },
    List { values: Vec<MetadataValueInput> },
    Struct { fields: Vec<MetadataFieldInput> },
    Reference { target: MetadataReferenceInput },
}

#[derive(Debug, Deserialize)]
struct MetadataFieldInput {
    name: String,
    value: MetadataValueInput,
}

#[derive(Debug, Deserialize)]
struct MetadataReferenceInput {
    target_kind: MetadataTargetKind,
    target_id: String,
}

#[derive(Debug, Serialize)]
struct MetadataFieldView {
    name: String,
    value: MetadataValueView,
}

#[derive(Debug, Serialize)]
struct ActivityView {
    id: String,
    kind: String,
    started_at_unix_micros: Option<i64>,
    finished_at_unix_micros: Option<i64>,
    tool: Option<ToolView>,
    agent: Option<AgentView>,
    inputs: Vec<ActivityEdgeView>,
    outputs: Vec<ActivityEdgeView>,
}

#[derive(Debug, Serialize)]
struct ToolView {
    name: String,
    version: Option<String>,
    uri: Option<String>,
}

#[derive(Debug, Serialize)]
struct AgentView {
    name: Option<String>,
    identifier: Option<AgentIdentifierView>,
}

#[derive(Debug, Serialize)]
struct AgentIdentifierView {
    scheme: String,
    value: String,
    qualifier: Option<String>,
}

#[derive(Debug, Serialize)]
struct JobView {
    id: String,
    kind: String,
    inputs: Vec<String>,
    output_asset_id: String,
    output_kind: &'static str,
    target_root: Option<String>,
    state: &'static str,
    claim_id: Option<String>,
    claim_expires_at_unix_micros: Option<i64>,
    claim_tool: Option<ToolView>,
    claim_agent: Option<AgentView>,
    completion_activity_id: Option<String>,
    completion_representation_id: Option<String>,
    failure_diagnostic: Option<String>,
}

#[derive(Debug, Serialize)]
struct JobRunView {
    job: JobView,
    output: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegenerationPlanView {
    artifact_representation_id: String,
    job: JobView,
    parameters: Vec<MetadataAssertionView>,
}

#[derive(Debug, Serialize)]
struct ActivityEdgeView {
    representation_id: String,
    role: Option<String>,
    snapshot: Option<ActivityEdgeSnapshotView>,
}

#[derive(Debug, Serialize)]
struct DependencySetView {
    source_representation_id: String,
    recorded_at_revision: u64,
    status: &'static str,
    dependencies: Vec<DependencyView>,
}

#[derive(Debug, Serialize)]
struct DependencyView {
    source_resource_id: Option<String>,
    kind: String,
    target: ObjectRefView,
    resolved_representation_id: Option<String>,
    required: bool,
    authored_reference: String,
}

#[derive(Debug, Serialize)]
struct QueryPageView<T> {
    items: Vec<T>,
    next_cursor: Option<String>,
    traversal_truncated: bool,
}

#[derive(Debug, Serialize)]
struct ProvenanceMatchView {
    representation_id: String,
    depth: u32,
}

#[derive(Debug, Serialize)]
struct RepresentationSummaryView {
    id: String,
    asset_id: String,
    kind: &'static str,
    structure: &'static str,
    fingerprints: Vec<FingerprintView>,
}

#[derive(Debug, Serialize)]
struct KnownMediaView {
    #[serde(rename = "asset_id")]
    asset: String,
    #[serde(rename = "representation_id")]
    representation: String,
    #[serde(rename = "resource_id")]
    resource: String,
}

#[derive(Debug, Serialize)]
struct ResourceSummaryView {
    id: String,
    fingerprints: Vec<FingerprintView>,
    file_size_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
struct LocatorQueryView {
    id: String,
    resource_id: String,
    uri: String,
    availability: &'static str,
    last_seen_unix_micros: Option<i64>,
    media_root: Option<String>,
    sequence_naming: Option<SequenceNamingView>,
}

#[derive(Debug, Serialize)]
struct DependencyMatchView {
    target: ObjectRefView,
    depth: u32,
}

#[derive(Debug, Serialize)]
struct ActivityEdgeSnapshotView {
    revision_sequence: u64,
    fingerprints: Vec<FingerprintSnapshotView>,
}

#[derive(Debug, Serialize)]
struct FingerprintSnapshotView {
    algorithm: String,
    version: u16,
    value_hex: String,
    observed_revision_sequence: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ArtifactEvaluationView {
    representation_id: String,
    state: &'static str,
    visited_representations: u32,
    truncated: bool,
    reasons: Vec<ArtifactReasonView>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ArtifactReasonView {
    ProducingActivityMissing {
        representation_id: String,
    },
    ProducingActivityAmbiguous {
        representation_id: String,
        activity_count: u32,
    },
    SnapshotAbsent {
        activity_id: String,
        representation_id: String,
        edge: &'static str,
    },
    FingerprintEvidenceMissing {
        activity_id: String,
        representation_id: String,
        edge: &'static str,
        fingerprint_algorithm: Option<String>,
        fingerprint_version: Option<u16>,
        snapshot_value_hex: Option<String>,
        current_value_hex: Option<String>,
    },
    FingerprintChanged {
        activity_id: String,
        representation_id: String,
        edge: &'static str,
        fingerprint_algorithm: String,
        fingerprint_version: u16,
        snapshot_value_hex: String,
        current_value_hex: String,
    },
    FingerprintRecomputationPending {
        activity_id: String,
        representation_id: String,
        edge: &'static str,
    },
    UpstreamNotCurrent {
        representation_id: String,
        upstream_state: &'static str,
    },
    TraversalTruncated {
        representation_id: String,
        traversal_limit: &'static str,
    },
    DependencySnapshotAbsent {
        activity_id: String,
        input_representation_id: String,
    },
    DependencyKnowledgeIncomplete {
        activity_id: String,
        input_representation_id: String,
        subject_representation_id: String,
        path: Vec<ArtifactDependencyPathView>,
        issue: &'static str,
    },
    DependencyPathChanged {
        activity_id: String,
        input_representation_id: String,
        path: Vec<ArtifactDependencyPathView>,
    },
    DependencyFingerprintChanged {
        activity_id: String,
        input_representation_id: String,
        representation_id: String,
        path: Vec<ArtifactDependencyPathView>,
        fingerprint_algorithm: String,
        fingerprint_version: u16,
        snapshot_value_hex: String,
        current_value_hex: String,
    },
    DependencyFingerprintRecomputationPending {
        activity_id: String,
        input_representation_id: String,
        representation_id: String,
        path: Vec<ArtifactDependencyPathView>,
    },
    DependencyFingerprintEvidenceMissing {
        activity_id: String,
        input_representation_id: String,
        representation_id: String,
        path: Vec<ArtifactDependencyPathView>,
        fingerprint_algorithm: Option<String>,
        fingerprint_version: Option<u16>,
        snapshot_value_hex: Option<String>,
        current_value_hex: Option<String>,
    },
}

#[derive(Debug, Serialize)]
struct ArtifactDependencyPathView {
    source_representation_id: String,
    dependency_position: u32,
    source_resource_id: Option<String>,
    kind: String,
    target: ObjectRefView,
    resolved_representation_id: Option<String>,
    authored_reference: String,
}

#[derive(Debug, Serialize)]
struct ArtifactReproducibilityView {
    representation_id: String,
    reproducible: bool,
    producing_activity_id: Option<String>,
    activity_kind: Option<String>,
    issues: Vec<ArtifactReproducibilityIssueView>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ArtifactReproducibilityIssueView {
    ProducingActivityMissing,
    ProducingActivityAmbiguous {
        activity_count: u32,
    },
    ToolIdentityMissing {
        activity_id: String,
    },
    ParametersMissing {
        activity_id: String,
    },
    InputRepresentationMissing {
        activity_id: String,
        representation_id: String,
    },
}

#[derive(Debug, Serialize)]
struct RevisionView {
    id: String,
    sequence: u64,
    transaction_id: String,
    committed_at_unix_micros: i64,
    origin: Option<RevisionOriginView>,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct RevisionOriginView {
    name: String,
    version: Option<String>,
    uri: Option<String>,
}

#[derive(Debug, Serialize)]
struct RevisionEventView {
    revision_id: String,
    position: u32,
    #[serde(flatten)]
    event: RevisionEventKindView,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RevisionEventKindView {
    AssetImported {
        asset_id: String,
    },
    RepresentationAdded {
        asset_id: String,
        representation_id: String,
    },
    ResourceAdded {
        resource_id: String,
    },
    RepresentationResourceAdded {
        representation_id: String,
        resource_id: String,
        structural_position: u32,
    },
    LocatorAdded {
        resource_id: String,
        locator_id: String,
    },
    LocatorRetired {
        resource_id: String,
        locator_id: String,
    },
    MediaRootAdded {
        media_root_id: String,
    },
    MediaRootEnabledChanged {
        media_root_id: String,
        enabled: bool,
    },
    MediaRootRemoved {
        media_root_id: String,
    },
    ExternalIdentifierAdded {
        target: ObjectRefView,
        identifier: RevisionIdentifierView,
    },
    ExternalIdentifierRemoved {
        target: ObjectRefView,
        identifier: RevisionIdentifierView,
    },
    MetadataAddedOrReplaced {
        target: ObjectRefView,
        vocabulary: String,
        property: String,
    },
    MetadataRemoved {
        target: ObjectRefView,
        vocabulary: String,
        property: String,
    },
    ActivityCreated {
        activity_id: String,
        activity_kind: String,
    },
    ActivityInputAdded {
        activity_id: String,
        representation_id: String,
        role: Option<String>,
    },
    ActivityOutputAdded {
        activity_id: String,
        representation_id: String,
        role: Option<String>,
    },
    ResourceFingerprintObserved {
        resource_id: String,
        algorithm: String,
        version: u16,
    },
    RepresentationFingerprintObserved {
        representation_id: String,
        algorithm: String,
        version: u16,
    },
    DependencySetRecorded {
        representation_id: String,
    },
    JobRequested {
        job_id: String,
    },
    JobClaimed {
        job_id: String,
    },
    JobClaimRenewed {
        job_id: String,
    },
    JobClaimReleased {
        job_id: String,
    },
    JobSucceeded {
        job_id: String,
    },
    JobFailed {
        job_id: String,
    },
    JobCancelled {
        job_id: String,
    },
}

#[derive(Debug, Serialize)]
struct RevisionIdentifierView {
    scheme: String,
    value: String,
    qualifier: Option<String>,
}

#[derive(Debug, Serialize)]
struct RepresentationRefView {
    representation_id: String,
}

#[derive(Clone, Copy)]
enum ActivityLookup {
    Producing,
    Consuming,
}

#[derive(Clone, Copy)]
enum ProvenanceDirection {
    Ancestors,
    Descendants,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match execute(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if !json || !print_conflict_json(&error) {
                eprintln!("error: {error:#}");
            }
            ExitCode::FAILURE
        }
    }
}

fn cli_decision_base(cli: &Cli) -> Option<CliDecisionBase> {
    cli.decision_base
        .map(CliDecisionBase::Scoped)
        .or_else(|| cli.base_revision.map(CliDecisionBase::Revision))
}

fn execute(cli: Cli) -> Result<()> {
    let base = cli_decision_base(&cli);
    match cli.command {
        Command::Init(args) => init(args, cli.json),
        Command::Inspect(args) => inspect(&args, cli.json),
        Command::Media(args) => match args.command {
            MediaCommand::Add(args) => media_add(&args, cli.json, base),
            MediaCommand::List(args) => media_list(&args, cli.json),
            MediaCommand::Show(args) => media_show(&args, cli.json),
            MediaCommand::Resolve(args) => media_resolve(args, cli.json, base),
            MediaCommand::Inventory(args) => media_inventory(&args, cli.json),
            MediaCommand::Fingerprint(args) => media_fingerprint(&args, cli.json, base),
            MediaCommand::VerifyContent(args) => media_verify_content(&args, cli.json),
            MediaCommand::Unresolved(args) => media_unresolved(&args, cli.json),
            MediaCommand::UnderRoot(args) => media_under_root(&args, cli.json),
            MediaCommand::FindByLocator(args) => media_find_by_locator(&args, cli.json),
            MediaCommand::FindByFingerprint(args) => media_find_by_fingerprint(&args, cli.json),
        },
        Command::Representation(args) => match args.command {
            RepresentationCommand::Add(args) => representation_add(&args, cli.json, base),
            RepresentationCommand::List(args) => representation_list(&args, cli.json),
            RepresentationCommand::Resources(args) => representation_resources(&args, cli.json),
            RepresentationCommand::Show(args) => representation_show(&args, cli.json),
            RepresentationCommand::UsingResource(args) => {
                representation_using_resource(&args, cli.json)
            }
        },
        Command::Root(args) => execute_roots(args.command, cli.json, base),
        Command::Locator(args) => match args.command {
            LocatorCommand::Retire(args) => locator_retire(&args, cli.json, base),
            LocatorCommand::List(args) => locator_list(&args, cli.json),
        },
        Command::Identifier(args) => match args.command {
            IdentifierCommand::Add(args) => identifier_mutate(args, false, cli.json, base),
            IdentifierCommand::Remove(args) => identifier_mutate(args, true, cli.json, base),
            IdentifierCommand::List(args) => identifier_list(&args, cli.json),
            IdentifierCommand::Find(args) => identifier_find(args, cli.json),
        },
        Command::Metadata(args) => match args.command {
            MetadataCommand::Add(args) => metadata_add(args, cli.json, base),
            MetadataCommand::AddText(args) => metadata_add_text(args, cli.json, base),
            MetadataCommand::List(args) => metadata_list(&args, cli.json),
            MetadataCommand::Remove(args) => metadata_remove(args, cli.json, base),
            MetadataCommand::Find(args) => metadata_find(args, cli.json),
        },
        Command::Activity(args) => match args.command {
            ActivityCommand::Add(args) => activity_add(*args, cli.json, base),
            ActivityCommand::List(args) => activity_list(&args, cli.json),
            ActivityCommand::Producing(args) => {
                activity_lookup(&args, ActivityLookup::Producing, cli.json)
            }
            ActivityCommand::Consuming(args) => {
                activity_lookup(&args, ActivityLookup::Consuming, cli.json)
            }
            ActivityCommand::Ancestors(args) => {
                activity_relatives(&args, ProvenanceDirection::Ancestors, cli.json)
            }
            ActivityCommand::Descendants(args) => {
                activity_relatives(&args, ProvenanceDirection::Descendants, cli.json)
            }
            ActivityCommand::Outputs(args) => activity_outputs(args, cli.json),
        },
        Command::Dependency(args) => match args.command {
            DependencyCommand::Record(args) => dependency_record(&args, cli.json, base),
            DependencyCommand::Show(args) => dependency_show(&args, cli.json),
            DependencyCommand::Dependencies(args) => dependency_dependencies(&args, cli.json),
            DependencyCommand::Dependents(args) => dependency_dependents(&args, cli.json),
        },
        Command::Artifact(args) => match args.command {
            ArtifactCommand::Evaluate(args) => artifact_evaluate(&args, cli.json),
            ArtifactCommand::Reproducibility(args) => artifact_reproducibility(&args, cli.json),
            ArtifactCommand::Stale(args) => artifact_stale(&args, cli.json),
        },
        Command::Job(args) => match args.command {
            JobCommand::Request(args) => job_request(args, cli.json, base),
            JobCommand::Claim(args) => job_claim(args, cli.json, base),
            JobCommand::Renew(args) => job_renew(&args, cli.json, base),
            JobCommand::Release(args) => job_release(&args, cli.json, base),
            JobCommand::Complete(args) => job_complete(&args, cli.json, base),
            JobCommand::Fail(args) => job_fail(args, cli.json, base),
            JobCommand::Cancel(args) => job_cancel(&args, cli.json, base),
            JobCommand::Show(args) => job_show(&args, cli.json),
            JobCommand::List(args) => job_list(&args, cli.json),
            JobCommand::Plan(args) => job_plan(&args, cli.json),
            JobCommand::Run(args) => job_run(&args, cli.json, base),
        },
        Command::Revisions(args) => match args.command {
            RevisionsCommand::Latest(args) => revisions_latest(&args, cli.json),
            RevisionsCommand::Since(args) => revisions_since(&args, cli.json),
            RevisionsCommand::Events(args) => revisions_events(&args, cli.json),
            RevisionsCommand::Changed(args) => revisions_changed(&args, cli.json),
            RevisionsCommand::Filtered(args) => revisions_filtered(&args, cli.json),
            RevisionsCommand::Wait(args) => revisions_wait(&args, cli.json),
        },
    }
}

fn init(args: InitArgs, json: bool) -> Result<()> {
    let production =
        SqliteProduction::create(&args.production, args.name).context("create production")?;
    let view = ProductionView {
        id: production.production().id().to_string(),
        path: production.path().display().to_string(),
        schema_version: production.production().schema_version(),
        display_name: production.production().display_name().map(str::to_owned),
    };
    if json {
        print_json(&view)
    } else {
        println!("created production {} at {}", view.id, view.path);
        Ok(())
    }
}

fn inspect(args: &ProductionQueryArgs, json: bool) -> Result<()> {
    if args.page.cursor.is_some() {
        bail!("inspect does not resume a closed snapshot; create a fresh inspection");
    }
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let view = production.read_session().context("open coherent read")?;
    let page = view
        .read()
        .assets_page(&query_page_request(&args.page)?)
        .context("inspect assets")?;
    let base = view.decision_base();
    let assets: Vec<_> = page
        .items()
        .iter()
        .map(|asset| {
            serde_json::json!({
                "id": asset.id().to_string(), "display_name": asset.display_name(),
                "created_at_unix_micros": asset.created_at().as_unix_micros()
            })
        })
        .collect();
    let roots: Vec<_> = view
        .read()
        .production()
        .media_roots()
        .iter()
        .map(root_view)
        .collect();
    if json {
        print_json(&serde_json::json!({
            "format_version": 1,
            "production_id": base.production_id().to_string(),
            "decision_base": base.to_string(),
            "revision_sequence": base.sequence(),
            "assets": assets,
            "media_roots": roots,
            "truncated": page.next_cursor().is_some()
        }))
    } else {
        println!(
            "production {} at revision {}",
            base.production_id(),
            base.sequence()
        );
        println!("decision base: {base}");
        for asset in page.items() {
            println!("{} {}", asset.id(), asset.display_name().unwrap_or("-"));
        }
        Ok(())
    }
}

fn media_add(
    args: &MediaAddArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let (prepared, inspection_paths) = prepare_cli_media(args)?;
    let (technical_metadata, inspections) = inspect_cli_media(args, &inspection_paths);
    let view = ImportView {
        asset_id: prepared.asset().id().to_string(),
        representation_id: prepared.representation().id().to_string(),
        resource_id: prepared.resources()[0].id().to_string(),
        locator_id: prepared.locators()[0].id().to_string(),
        uri: prepared.locators()[0].uri().to_owned(),
        resource_count: prepared.resources().len(),
        inspections,
    };
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "import")?;
    set_cli_revision_context(&mut transaction, "Import media")?;
    transaction
        .import_original(&prepared)
        .context("stage media import")?;
    for assertion in &technical_metadata {
        transaction
            .add_metadata_value(
                ObjectRef::Representation(prepared.representation().id()),
                assertion.property(),
                assertion.value(),
            )
            .context("stage technical metadata")?;
    }
    let receipt = transaction
        .commit_with_receipt()
        .context("commit media import")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!("imported asset {} from {}", view.asset_id, view.uri);
        for inspection in &view.inspections {
            println!("inspection {}: {}", inspection.path, inspection.status);
        }
        Ok(())
    }
}

fn prepare_cli_media(args: &MediaAddArgs) -> Result<(OriginalMediaImport, Vec<PathBuf>)> {
    let recognize = args.path.is_dir() || args.recognize_companions;
    if recognize {
        let fallback_rate = RationalRate::new(24, 1).context("prepare recognition rate")?;
        let recognized = MediaRecognizer::new(args.sequence_rate.unwrap_or(fallback_rate))
            .recognize(&args.path)
            .context("recognize media")?;
        let [recognized] = recognized.as_slice() else {
            bail!(
                "media recognition returned {} candidates; supply a path identifying one layout",
                recognized.len()
            );
        };
        if matches!(recognized, RecognizedMedia::ImageSequence { .. })
            && args.sequence_rate.is_none()
        {
            bail!("recognized image sequences require --sequence-rate NUMERATOR/DENOMINATOR");
        }
        let paths = recognized_inspection_paths(recognized);
        let prepared = prepare_recognized_original_media(
            recognized,
            args.name.clone(),
            Some("postproject-cli".to_owned()),
        )
        .context("prepare recognized media import")?;
        Ok((prepared, paths))
    } else {
        let prepared = prepare_original_media(
            &args.path,
            args.name.clone(),
            Some("postproject-cli".to_owned()),
        )
        .context("prepare media import")?;
        Ok((prepared, vec![args.path.clone()]))
    }
}

fn recognized_inspection_paths(recognized: &RecognizedMedia) -> Vec<PathBuf> {
    match recognized {
        RecognizedMedia::SingleFile(path) => vec![path.clone()],
        RecognizedMedia::ImageSequence {
            directory,
            naming,
            frames,
            missing_frames,
            ..
        } => (frames.start()..=frames.end())
            .find(|frame| !missing_frames.contains(frame))
            .map(|frame| vec![directory.join(naming.filename(frame))])
            .unwrap_or_default(),
        RecognizedMedia::OrderedParts(members) | RecognizedMedia::Package(members) => members
            .iter()
            .filter(|member| member.is_required())
            .map(|member| member.path().to_path_buf())
            .collect(),
        _ => Vec::new(),
    }
}

fn inspect_cli_media(
    args: &MediaAddArgs,
    paths: &[PathBuf],
) -> (Vec<MetadataAssertion>, Vec<InspectionView>) {
    if !args.inspect {
        return (Vec::new(), Vec::new());
    }
    let inspector = FfprobeInspector::with_executable(&args.ffprobe);
    let mut assertions = Vec::new();
    let mut views = Vec::new();
    for path in paths {
        let (status, reason) = match inspector.inspect(path) {
            Ok(InspectionOutcome::Inspected(metadata)) => {
                assertions.extend_from_slice(metadata.assertions());
                ("recorded", None)
            }
            Ok(InspectionOutcome::Unavailable { reason }) => ("unavailable", Some(reason)),
            Ok(InspectionOutcome::Failed { reason }) => ("failed", Some(reason)),
            Err(error) => ("failed", Some(error.to_string())),
            Ok(_) => ("failed", Some("unsupported inspection outcome".to_owned())),
        };
        views.push(InspectionView {
            path: path.display().to_string(),
            status,
            reason,
        });
    }
    (assertions, views)
}

fn representation_add(
    args: &RepresentationAddArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let asset_id = AssetId::from_str(&args.asset_id).context("parse asset ID")?;
    let kind = match args.kind {
        RepresentationKindArg::Original => RepresentationKind::Original,
        RepresentationKindArg::Proxy => RepresentationKind::Proxy,
        RepresentationKindArg::Optimized => RepresentationKind::Optimized,
        RepresentationKindArg::Derived => RepresentationKind::Derived,
    };
    let encoded = fs::read(&args.spec_file)
        .with_context(|| format!("read representation spec {}", args.spec_file.display()))?;
    let spec: RepresentationSourceSpec =
        serde_json::from_slice(&encoded).context("parse representation spec")?;
    let source = match spec {
        RepresentationSourceSpec::SingleFile { path } => MediaSource::File(path),
        RepresentationSourceSpec::ImageSequence {
            directory,
            prefix,
            suffix,
            padding,
            start,
            end,
            step,
            rate_numerator,
            rate_denominator,
            missing_frames,
        } => MediaSource::ImageSequence(ImageSequenceSource::new(
            directory,
            SequenceNaming::new(prefix, suffix, padding)?,
            FrameRange::new(start, end, step)?,
            RationalRate::new(rate_numerator, rate_denominator)?,
            missing_frames,
        )),
        RepresentationSourceSpec::OrderedParts { members } => {
            MediaSource::OrderedParts(file_resource_sources(members)?)
        }
        RepresentationSourceSpec::Package { members } => {
            MediaSource::Package(file_resource_sources(members)?)
        }
    };
    let prepared =
        prepare_representation(asset_id, kind, source).context("prepare representation")?;
    let view = RepresentationRefView {
        representation_id: prepared.representation().id().to_string(),
    };
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "representation")?;
    set_cli_revision_context(&mut transaction, "Add representation")?;
    transaction
        .add_representation(&prepared)
        .context("stage representation")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit representation")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!("added representation {}", view.representation_id);
        Ok(())
    }
}

fn media_fingerprint(
    args: &MediaFingerprintArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let resource_id = ResourceId::from_str(&args.resource_id).context("parse resource ID")?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let usage = resource_usage(&production, resource_id).context("load resource usage")?;
    let naming = sequence_naming_for(&production, resource_id, args)?;
    let observation = observe_resource_content(resource_id, &usage, &args.path, naming.as_ref())
        .context("fingerprint resource content")?;

    let mut transaction = begin_cli_transaction(&mut production, base_revision, "fingerprint")?;
    set_cli_revision_context(&mut transaction, "Record fingerprint observation")?;
    transaction
        .record_resource_fingerprint(resource_id, observation.resource())
        .context("stage resource fingerprint")?;
    if let Some(facts) = observation.file_facts() {
        transaction
            .record_resource_file_facts(resource_id, facts)
            .context("stage resource file facts")?;
    }
    for (representation_id, fingerprint) in observation.representations() {
        transaction
            .record_representation_fingerprint(*representation_id, fingerprint)
            .context("stage representation fingerprint")?;
    }
    let receipt = transaction
        .commit_with_receipt()
        .context("commit fingerprints")?;

    let view = FingerprintObservationView {
        resource_id: resource_id.to_string(),
        outcome: match observation.outcome() {
            ContentObservationOutcome::Unchanged => "unchanged",
            ContentObservationOutcome::Changed => "changed",
            _ => "first",
        },
        resource_fingerprint: fingerprint_view(
            observation.resource().algorithm(),
            observation.resource().version(),
            observation.resource().value(),
        ),
        representation_fingerprints: observation
            .representations()
            .iter()
            .map(
                |(representation_id, fingerprint)| RepresentationFingerprintView {
                    representation_id: representation_id.to_string(),
                    fingerprint: fingerprint_view(
                        fingerprint.algorithm(),
                        fingerprint.version(),
                        fingerprint.value(),
                    ),
                },
            )
            .collect(),
    };
    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!(
            "{} content of resource {}; fingerprints for {} representation(s)",
            view.outcome,
            view.resource_id,
            view.representation_fingerprints.len()
        );
        Ok(())
    }
}

/// Returns the explicit naming, or the one recorded for the directory when the
/// resource has locators with namings.
fn sequence_naming_for(
    production: &SqliteProduction,
    resource_id: ResourceId,
    args: &MediaFingerprintArgs,
) -> Result<Option<SequenceNaming>> {
    if let Some(naming) = &args.sequence_naming {
        return Ok(Some(naming.0.clone()));
    }
    let locators = production
        .locators(resource_id)
        .context("load resource locators")?;
    if locators
        .iter()
        .all(|locator| locator.sequence_naming().is_none())
    {
        return Ok(None);
    }
    recorded_sequence_naming(&locators, &args.path)
        .context("find the recorded sequence naming")?
        .context("no sequence naming is recorded for this directory; pass --sequence-naming")
        .map(Some)
}

fn fingerprint_view(algorithm: &str, version: u16, value: &[u8]) -> FingerprintView {
    FingerprintView {
        algorithm: algorithm.to_owned(),
        version,
        value_hex: hex::encode(value),
    }
}

fn media_verify_content(args: &MediaFingerprintArgs, json: bool) -> Result<()> {
    let resource_id = ResourceId::from_str(&args.resource_id).context("parse resource ID")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let usage = resource_usage(&production, resource_id).context("load resource usage")?;
    let (representation, resources) = usage
        .first()
        .context("resource is not used by any representation")?;
    let resource = resources
        .iter()
        .find(|resource| resource.id() == resource_id)
        .context("representation does not list the resource")?;
    let naming = sequence_naming_for(&production, resource_id, args)?;
    let verification = verify_resource_content(
        resource,
        representation.content_structure(),
        &args.path,
        naming.as_ref(),
    )
    .context("verify resource content")?;
    let view = ContentVerificationView {
        resource_id: resource_id.to_string(),
        verification: match verification {
            ContentVerification::Matches => "matches",
            ContentVerification::Differs => "differs",
            _ => "not_comparable",
        },
    };
    if json {
        print_json(&view)
    } else {
        println!("{}", view.verification);
        Ok(())
    }
}

fn file_resource_sources(members: Vec<FileResourceSpec>) -> Result<Vec<FileResourceSource>> {
    members
        .into_iter()
        .map(|member| {
            Ok(FileResourceSource::new(
                member.path,
                ResourceRole::new(member.role)?,
                member.required,
            ))
        })
        .collect()
}

fn media_list(args: &MediaListArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    if args.limit.is_some() || args.cursor.is_some() {
        let page_args = QueryPageArgs {
            limit: args.limit.unwrap_or(DEFAULT_QUERY_PAGE_SIZE),
            cursor: args.cursor.clone(),
        };
        let page = production
            .assets_page(&query_page_request(&page_args)?)
            .context("query assets")?;
        let view = query_page_view(&page, |asset| asset_summary(&production, asset))?;
        return print_query_page(&view, json, false, print_asset_summary);
    }
    let views = production
        .assets()
        .context("load assets")?
        .iter()
        .map(|asset| asset_summary(&production, asset))
        .collect::<Result<Vec<_>>>()?;

    if json {
        print_json(&views)
    } else {
        for asset in &views {
            print_asset_summary(asset);
        }
        Ok(())
    }
}

fn asset_summary(production: &SqliteProduction, asset: &Asset) -> Result<AssetSummary> {
    let representation_count = production
        .representations(asset.id())
        .context("load asset representations")?
        .len();
    Ok(AssetSummary {
        id: asset.id().to_string(),
        display_name: asset.display_name().map(str::to_owned),
        created_at_unix_micros: asset.created_at().as_unix_micros(),
        representation_count,
    })
}

fn print_asset_summary(asset: &AssetSummary) {
    println!(
        "{}\t{}\t{} representation(s)",
        asset.id,
        asset.display_name.as_deref().unwrap_or("-"),
        asset.representation_count
    );
}

fn media_unresolved(args: &ProductionQueryArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .unresolved_media(&query_page_request(&args.page)?)
        .context("query unresolved media")?;
    let view = query_page_view(&page, |id| Ok(representation_ref_view(*id)))?;
    print_query_page(&view, json, false, |item| {
        println!("{}", item.representation_id);
    })
}

fn media_under_root(args: &MediaUnderRootArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .representations_under_media_root(&args.root, &query_page_request(&args.page)?)
        .context("query representations under media root")?;
    let view = query_page_view(&page, |representation| {
        Ok(representation_summary_view(representation))
    })?;
    print_query_page(&view, json, false, print_representation_summary)
}

fn media_find_by_locator(args: &MediaFindByLocatorArgs, json: bool) -> Result<()> {
    let locator = LocatorIdentity::new(
        &args.uri,
        args.sequence_naming.as_ref().map(|naming| naming.0.clone()),
    )
    .context("validate locator identity")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .find_known_media_by_locator(&locator, &query_page_request(&args.page)?)
        .context("find known media by locator")?;
    print_known_media_page(&page, json)
}

fn media_find_by_fingerprint(args: &MediaFindByFingerprintArgs, json: bool) -> Result<()> {
    let value = hex::decode(&args.value_hex).context("decode fingerprint value")?;
    let fingerprint = ResourceFingerprint::new(&args.algorithm, args.version, value)
        .context("validate fingerprint")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .find_known_media_by_fingerprint(&fingerprint, &query_page_request(&args.page)?)
        .context("find known media by fingerprint")?;
    print_known_media_page(&page, json)
}

fn print_known_media_page(
    page: &QueryPage<postproject_core::KnownMediaMatch>,
    json: bool,
) -> Result<()> {
    let view = query_page_view(page, |item| {
        Ok(KnownMediaView {
            asset: item.asset().id().to_string(),
            representation: item.representation().id().to_string(),
            resource: item.resource().id().to_string(),
        })
    })?;
    print_query_page(&view, json, false, |item| {
        println!("{}\t{}\t{}", item.asset, item.representation, item.resource);
    })
}

fn representation_list(args: &RepresentationListArgs, json: bool) -> Result<()> {
    let asset_id = parse_asset_id(&args.asset_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .representations_page(asset_id, &query_page_request(&args.page)?)
        .context("query asset representations")?;
    let view = query_page_view(&page, |representation| {
        Ok(representation_summary_view(representation))
    })?;
    print_query_page(&view, json, false, print_representation_summary)
}

fn representation_resources(args: &RepresentationResourcesArgs, json: bool) -> Result<()> {
    let representation_id = parse_representation_id(&args.representation_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .resources_page(representation_id, &query_page_request(&args.page)?)
        .context("query representation resources")?;
    let view = query_page_view(&page, |resource| Ok(resource_summary_view(resource)))?;
    print_query_page(&view, json, false, |item| {
        println!(
            "{}\t{}\t{} fingerprint(s)",
            item.id,
            item.file_size_bytes
                .map_or_else(|| "-".to_owned(), |size| size.to_string()),
            item.fingerprints.len()
        );
    })
}

fn representation_show(args: &RepresentationShowArgs, json: bool) -> Result<()> {
    let representation_id = parse_representation_id(&args.representation_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let representation = production
        .representation(representation_id)
        .context("load representation")?;
    let view = representation_summary_view(&representation);
    if json {
        print_json(&view)
    } else {
        print_representation_summary(&view);
        Ok(())
    }
}

fn representation_using_resource(args: &RepresentationUsingResourceArgs, json: bool) -> Result<()> {
    let resource_id = ResourceId::from_str(&args.resource_id).context("parse resource ID")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .representations_using_resource(resource_id, &query_page_request(&args.page)?)
        .context("query representations using resource")?;
    let view = query_page_view(&page, |representation| {
        Ok(representation_summary_view(representation))
    })?;
    print_query_page(&view, json, false, print_representation_summary)
}

fn locator_list(args: &LocatorListArgs, json: bool) -> Result<()> {
    let resource_id = ResourceId::from_str(&args.resource_id).context("parse resource ID")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .locators_page(resource_id, &query_page_request(&args.page)?)
        .context("query resource locators")?;
    let view = query_page_view(&page, |locator| Ok(locator_query_view(locator)))?;
    print_query_page(&view, json, false, |item| {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            item.id,
            item.uri,
            item.availability,
            item.media_root.as_deref().unwrap_or("-"),
            item.sequence_naming
                .as_ref()
                .map_or("-", |naming| naming.pattern.as_str())
        );
    })
}

fn representation_ref_view(id: RepresentationId) -> RepresentationRefView {
    RepresentationRefView {
        representation_id: id.to_string(),
    }
}

fn representation_summary_view(representation: &Representation) -> RepresentationSummaryView {
    RepresentationSummaryView {
        id: representation.id().to_string(),
        asset_id: representation.asset_id().to_string(),
        kind: representation_kind(representation.kind()),
        structure: content_structure_kind(representation.content_structure().kind()),
        fingerprints: representation
            .fingerprints()
            .iter()
            .map(|fingerprint| FingerprintView {
                algorithm: fingerprint.algorithm().to_owned(),
                version: fingerprint.version(),
                value_hex: hex::encode(fingerprint.value()),
            })
            .collect(),
    }
}

fn print_representation_summary(item: &RepresentationSummaryView) {
    println!(
        "{}\t{}\t{}\t{}",
        item.id, item.asset_id, item.kind, item.structure
    );
}

fn resource_summary_view(resource: &Resource) -> ResourceSummaryView {
    let view = resource_view(resource, &[]);
    ResourceSummaryView {
        id: view.id,
        fingerprints: view.fingerprints,
        file_size_bytes: view.file_size_bytes,
    }
}

fn locator_query_view(locator: &Locator) -> LocatorQueryView {
    let view = LocatorView::from(locator);
    LocatorQueryView {
        id: view.id,
        resource_id: locator.resource_id().to_string(),
        uri: view.uri,
        availability: view.availability,
        last_seen_unix_micros: view.last_seen_unix_micros,
        media_root: locator.media_root().map(str::to_owned),
        sequence_naming: view.sequence_naming,
    }
}

fn media_show(args: &MediaAssetArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let asset_id = parse_asset_id(&args.asset_id)?;
    let asset = find_asset(&production, asset_id)?;
    let view = asset_view(&production, &asset)?;

    if json {
        print_json(&view)
    } else {
        println!(
            "asset {} ({})",
            view.id,
            view.display_name.as_deref().unwrap_or("unnamed")
        );
        for representation in &view.representations {
            println!(
                "  {} {} ({}): {} resource(s)",
                representation.kind,
                representation.id,
                representation.structure,
                representation.resources.len()
            );
            for resource in &representation.resources {
                for locator in &resource.locators {
                    match &locator.sequence_naming {
                        Some(naming) => println!(
                            "    {} {} [{}]",
                            locator.uri, naming.pattern, locator.availability
                        ),
                        None => println!("    {} [{}]", locator.uri, locator.availability),
                    }
                }
            }
        }
        Ok(())
    }
}

fn root_add(args: RootAddArgs, json: bool, base_revision: Option<CliDecisionBase>) -> Result<()> {
    let root = MediaRoot::new(
        MediaRootId::new(),
        args.name,
        args.label,
        None,
        args.priority,
        true,
    )
    .context("prepare media root")?;
    let view = root_view(&root);
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "root")?;
    set_cli_revision_context(&mut transaction, "Add media root")?;
    transaction
        .add_media_root(root)
        .context("stage media root")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit media root")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!("added media root {} ({})", view.name, view.id);
        Ok(())
    }
}

fn execute_roots(command: RootCommand, json: bool, base: Option<CliDecisionBase>) -> Result<()> {
    match command {
        RootCommand::Add(args) => root_add(args, json, base),
        RootCommand::List(args) => root_list(&args, json),
        RootCommand::Page(args) => root_page(&args, json),
        RootCommand::Enable(args) => root_set_enabled(&args, true, json, base),
        RootCommand::Disable(args) => root_set_enabled(&args, false, json, base),
        RootCommand::Remove(args) => root_remove(&args, json, base),
    }
}

fn root_list(args: &ProductionArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let roots = production
        .media_roots()?
        .iter()
        .map(root_view)
        .collect::<Vec<_>>();
    if json {
        print_json(&roots)
    } else {
        for root in roots {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                root.id,
                if root.enabled { "enabled" } else { "disabled" },
                root.priority,
                root.label.as_deref().unwrap_or("-"),
                root.name
            );
        }
        Ok(())
    }
}

fn root_page(args: &RootPageArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production.media_roots_page(&query_page_request(&args.page)?)?;
    let view = query_page_view(&page, |root| Ok(root_view(root)))?;
    print_query_page(&view, json, false, |root| {
        println!(
            "{}\t{}\t{}\t{}",
            root.id, root.name, root.priority, root.enabled
        );
    })
}

fn root_set_enabled(
    args: &RootMutationArgs,
    enabled: bool,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let base = base_revision.context("root state changes require --decision-base from inspect")?;
    let root_id = MediaRootId::from_str(&args.root_id).context("parse media-root ID")?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let root = production
        .production()
        .media_roots()
        .iter()
        .find(|root| root.id() == root_id)
        .context("media root does not exist")?;
    let view = RootView {
        enabled,
        ..root_view(root)
    };
    let mut transaction = begin_cli_transaction(&mut production, Some(base), "root")?;
    set_cli_revision_context(
        &mut transaction,
        if enabled {
            "Enable media root"
        } else {
            "Disable media root"
        },
    )?;
    transaction
        .set_media_root_enabled(root_id, enabled)
        .context("stage media-root state change")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit media-root state change")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!(
            "{} media root {}",
            if enabled { "enabled" } else { "disabled" },
            view.id
        );
        Ok(())
    }
}

fn root_remove(
    args: &RootMutationArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let base = base_revision.context("root removal requires --decision-base from inspect")?;
    let root_id = MediaRootId::from_str(&args.root_id).context("parse media-root ID")?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let root = production
        .production()
        .media_roots()
        .iter()
        .find(|root| root.id() == root_id)
        .context("media root does not exist")?;
    let view = root_view(root);
    let mut transaction = begin_cli_transaction(&mut production, Some(base), "root")?;
    set_cli_revision_context(&mut transaction, "Remove media root")?;
    transaction
        .remove_media_root(root_id)
        .context("stage media-root removal")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit media-root removal")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!("removed media root {} ({})", view.name, view.id);
        Ok(())
    }
}

fn root_view(root: &MediaRoot) -> RootView {
    RootView {
        id: root.id().to_string(),
        name: root.name().to_owned(),
        label: root.label().map(str::to_owned),
        legacy_uri: root.legacy_uri().map(str::to_owned),
        priority: root.priority(),
        enabled: root.is_enabled(),
    }
}

fn locator_retire(
    args: &LocatorRetireArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let base = base_revision.context("locator retirement requires --decision-base from inspect")?;
    let locator_id = LocatorId::from_str(&args.locator_id).context("parse locator ID")?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, Some(base), "locator")?;
    set_cli_revision_context(&mut transaction, "Retire media locator")?;
    transaction
        .retire_locator(locator_id)
        .context("stage locator retirement")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit locator retirement")?;

    if json {
        print_json_with_receipt(
            &serde_json::json!({ "id": locator_id.to_string() }),
            &receipt,
        )
    } else {
        println!("retired locator {locator_id}");
        Ok(())
    }
}

fn identifier_mutate(
    args: IdentifierMutationArgs,
    remove: bool,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    if remove && base_revision.is_none() {
        bail!("identifier removal requires --decision-base from inspect");
    }
    let target = parse_identifier_target(args.target.target_kind, &args.target.target_id)?;
    let scheme = IdentifierScheme::new(args.scheme).context("validate identifier scheme")?;
    let identifier = ExternalIdentifier::new(scheme, args.value, args.qualifier)
        .context("validate external identifier")?;
    let view = external_identifier_view(target, &identifier);
    let mut production =
        SqliteProduction::open(&args.target.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "identifier")?;
    set_cli_revision_context(
        &mut transaction,
        if remove {
            "Remove external identifier"
        } else {
            "Add external identifier"
        },
    )?;
    if remove {
        transaction
            .remove_external_identifier(target, &identifier)
            .context("stage external identifier removal")?;
    } else {
        transaction
            .add_external_identifier(target, &identifier)
            .context("stage external identifier attachment")?;
    }
    let receipt = transaction
        .commit_with_receipt()
        .context("commit external identifier mutation")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!(
            "{} {}:{} on {} {}",
            if remove { "removed" } else { "attached" },
            view.scheme,
            view.value,
            view.target_kind,
            view.target_id
        );
        Ok(())
    }
}

fn identifier_list(args: &IdentifierTargetArgs, json: bool) -> Result<()> {
    let target = parse_identifier_target(args.target_kind, &args.target_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let views: Vec<_> = production
        .external_identifiers(target)
        .context("load external identifiers")?
        .iter()
        .map(|identifier| external_identifier_view(target, identifier))
        .collect();

    if json {
        print_json(&views)
    } else {
        for view in views {
            println!(
                "{}\t{}\t{}",
                view.scheme,
                view.value,
                view.qualifier.as_deref().unwrap_or("-")
            );
        }
        Ok(())
    }
}

fn identifier_find(args: IdentifierFindArgs, json: bool) -> Result<()> {
    let scheme = IdentifierScheme::new(args.scheme).context("validate identifier scheme")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let views: Vec<_> = production
        .find_by_external_identifier(&scheme, &args.value, args.qualifier.as_deref())
        .context("find external identifier")?
        .into_iter()
        .map(object_ref_view)
        .collect::<Result<_>>()?;

    if json {
        print_json(&views)
    } else {
        for view in views {
            println!("{}\t{}", view.kind, view.id);
        }
        Ok(())
    }
}

fn metadata_add_text(
    args: MetadataAddTextArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let target = parse_metadata_target(args.target.target_kind, &args.target.target_id)?;
    let property = parse_metadata_property(args.vocabulary, args.property)?;
    let value = match args.language {
        Some(language) => MetadataValue::language_string(args.value, language),
        None => MetadataValue::string(args.value),
    }
    .context("validate metadata text")?;
    let assertion = MetadataAssertion::new(property.clone(), value.clone());
    let view = metadata_assertion_view(target, &assertion)?;
    let mut production =
        SqliteProduction::open(&args.target.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "metadata")?;
    set_cli_revision_context(&mut transaction, "Add metadata")?;
    transaction
        .add_metadata_value(target, &property, &value)
        .context("stage metadata value")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit metadata value")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!(
            "added {}:{} to {} {}",
            view.vocabulary, view.property, view.target_kind, view.target_id
        );
        Ok(())
    }
}

fn metadata_add(
    args: MetadataAddArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let target = parse_metadata_target(args.target.target_kind, &args.target.target_id)?;
    let property = parse_metadata_property(args.vocabulary, args.property)?;
    let value = read_metadata_value_file(&args.value_file)?;
    let assertion = MetadataAssertion::new(property.clone(), value.clone());
    let view = metadata_assertion_view(target, &assertion)?;
    let mut production =
        SqliteProduction::open(&args.target.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "metadata")?;
    set_cli_revision_context(&mut transaction, "Add metadata")?;
    transaction
        .add_metadata_value(target, &property, &value)
        .context("stage metadata value")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit metadata value")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!(
            "added {}:{} to {} {}",
            view.vocabulary, view.property, view.target_kind, view.target_id
        );
        Ok(())
    }
}

fn metadata_list(args: &MetadataTargetArgs, json: bool) -> Result<()> {
    let target = parse_metadata_target(args.target_kind, &args.target_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let views = production
        .metadata(target)
        .context("load metadata")?
        .iter()
        .map(|assertion| metadata_assertion_view(target, assertion))
        .collect::<Result<Vec<_>>>()?;

    print_metadata_assertions(&views, json)
}

fn metadata_remove(
    args: MetadataPropertyArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let base = base_revision.context("metadata removal requires --decision-base from inspect")?;
    let target = parse_metadata_target(args.target.target_kind, &args.target.target_id)?;
    let property = parse_metadata_property(args.vocabulary, args.property)?;
    let target_view = object_ref_view(target)?;
    let view = MetadataPropertyView {
        target_kind: target_view.kind,
        target_id: target_view.id,
        vocabulary: property.vocabulary().as_str().to_owned(),
        property: property.property().as_str().to_owned(),
    };
    let mut production =
        SqliteProduction::open(&args.target.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, Some(base), "metadata")?;
    set_cli_revision_context(&mut transaction, "Remove metadata")?;
    transaction
        .remove_metadata_property(target, &property)
        .context("stage metadata property removal")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit metadata property removal")?;

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!(
            "removed {}:{} from {} {}",
            view.vocabulary, view.property, view.target_kind, view.target_id
        );
        Ok(())
    }
}

fn metadata_find(args: MetadataFindArgs, json: bool) -> Result<()> {
    let property = parse_metadata_property(args.vocabulary, args.property)?;
    let exact_value = args
        .value_file
        .as_deref()
        .map(read_metadata_value_file)
        .transpose()?;
    let query =
        MetadataQuery::new(property, exact_value).context("validate metadata query predicate")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .metadata_query(&query, &query_page_request(&args.page)?)
        .context("query metadata property")?;
    let view = query_page_view(&page, |matched| {
        metadata_assertion_view(matched.target(), matched.assertion())
    })?;
    if json {
        print_json(&view)
    } else {
        print_metadata_assertions(&view.items, false)?;
        print_page_trailer(&view, false);
        Ok(())
    }
}

fn read_metadata_value_file(path: &Path) -> Result<MetadataValue> {
    let encoded = fs::read_to_string(path)
        .with_context(|| format!("read metadata value {}", path.display()))?;
    let input: MetadataValueInput =
        serde_json::from_str(&encoded).context("parse typed metadata JSON")?;
    input.into_value()
}

fn activity_list(args: &ProductionArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let views: Vec<_> = production
        .activities()
        .context("load activities")?
        .iter()
        .map(activity_view)
        .collect();

    print_activity_views(&views, json)
}

fn activity_lookup(
    args: &ActivityRepresentationQueryArgs,
    lookup: ActivityLookup,
    json: bool,
) -> Result<()> {
    let representation_id = parse_representation_id(&args.representation_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let request = query_page_request(&args.page)?;
    let page = match lookup {
        ActivityLookup::Producing => {
            production.activities_producing_page(representation_id, &request)
        }
        ActivityLookup::Consuming => {
            production.activities_consuming_page(representation_id, &request)
        }
    }
    .context("query representation activities")?;
    let view = query_page_view(&page, |activity| Ok(activity_view(activity)))?;
    print_query_page(&view, json, false, print_activity_summary)
}

fn print_activity_views(views: &[ActivityView], json: bool) -> Result<()> {
    if json {
        print_json(&views)
    } else {
        for view in views {
            print_activity_summary(view);
        }
        Ok(())
    }
}

fn print_activity_summary(view: &ActivityView) {
    println!(
        "{}\t{}\t{} input(s)\t{} output(s)",
        view.id,
        view.kind,
        view.inputs.len(),
        view.outputs.len()
    );
}

fn activity_relatives(
    args: &ProvenanceQueryArgs,
    direction: ProvenanceDirection,
    json: bool,
) -> Result<()> {
    let representation_id = parse_representation_id(&args.representation_id)?;
    let limits = ProvenanceQueryLimits::new(args.max_depth, args.max_representations)
        .context("validate provenance query bounds")?;
    let request = query_page_request(&args.page)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = match direction {
        ProvenanceDirection::Ancestors => {
            production.ancestors_page(representation_id, limits, &request)
        }
        ProvenanceDirection::Descendants => {
            production.descendants_page(representation_id, limits, &request)
        }
    }
    .context("traverse provenance")?;
    let view = query_page_view(&page, |item| {
        Ok(ProvenanceMatchView {
            representation_id: item.representation_id().to_string(),
            depth: item.depth(),
        })
    })?;
    print_query_page(&view, json, true, |item| {
        println!("{}\t{}", item.representation_id, item.depth);
    })
}

fn activity_outputs(args: ActivityOutputsArgs, json: bool) -> Result<()> {
    let query = if let Some(kind) = args.kind {
        ActivityOutputQuery::Kind(ActivityKind::new(kind).context("validate activity kind")?)
    } else if let Some(name) = args.tool_name {
        ActivityOutputQuery::Tool(
            ToolIdentity::new(name, args.tool_version, args.tool_uri)
                .context("validate tool identity")?,
        )
    } else {
        bail!("either --kind or --tool-name is required");
    };
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .activity_outputs(&query, &query_page_request(&args.page)?)
        .context("query activity outputs")?;
    let view = query_page_view(&page, |id| Ok(representation_ref_view(*id)))?;
    print_query_page(&view, json, false, |item| {
        println!("{}", item.representation_id);
    })
}

fn dependency_show(args: &ActivityRepresentationArgs, json: bool) -> Result<()> {
    let representation_id = parse_representation_id(&args.representation_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let dependencies = production
        .dependency_set(representation_id)
        .context("load dependency set")?
        .as_ref()
        .map(dependency_set_view)
        .transpose()?;
    print_dependency_set(dependencies.as_ref(), json)
}

fn dependency_record(
    args: &DependencyRecordArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let base =
        base_revision.context("dependency replacement requires --decision-base from inspect")?;
    let representation_id = parse_representation_id(&args.representation_id)?;
    let encoded = fs::read(&args.spec_file)
        .with_context(|| format!("read dependency spec {}", args.spec_file.display()))?;
    let specs: Vec<DependencySpec> =
        serde_json::from_slice(&encoded).context("parse dependency spec JSON")?;
    let dependencies = specs
        .into_iter()
        .map(DependencySpec::into_dependency)
        .collect::<Result<Vec<_>>>()?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, Some(base), "dependency")?;
    set_cli_revision_context(&mut transaction, "Record dependency set")?;
    transaction
        .record_dependency_set(representation_id, &dependencies)
        .context("stage dependency set")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit dependency set")?;
    drop(transaction);
    let stored = production
        .dependency_set(representation_id)
        .context("reload dependency set")?
        .context("committed dependency set is missing")?;
    let view = dependency_set_view(&stored)?;
    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        print_dependency_set(Some(&view), false)
    }
}

fn print_dependency_set(view: Option<&DependencySetView>, json: bool) -> Result<()> {
    if json {
        print_json(&view)
    } else if let Some(view) = view {
        println!(
            "{}\t{}\t{} dependency(ies)",
            view.source_representation_id,
            view.status,
            view.dependencies.len()
        );
        for dependency in &view.dependencies {
            println!(
                "{}\t{}:{}\t{}",
                dependency.kind,
                dependency.target.kind,
                dependency.target.id,
                dependency.authored_reference
            );
        }
        Ok(())
    } else {
        println!("no dependency observation");
        Ok(())
    }
}

impl DependencySpec {
    fn into_dependency(self) -> Result<Dependency> {
        let source_resource_id = self
            .source_resource_id
            .map(|value| ResourceId::from_str(&value).context("parse source resource ID"))
            .transpose()?;
        let target = match self.target {
            DependencyTargetSpec::Asset { id } => DependencyTarget::Asset(
                AssetId::from_str(&id).context("parse dependency target asset ID")?,
            ),
            DependencyTargetSpec::Representation { id } => DependencyTarget::Representation(
                RepresentationId::from_str(&id)
                    .context("parse dependency target representation ID")?,
            ),
        };
        let resolved_representation_id = self
            .resolved_representation_id
            .map(|value| {
                RepresentationId::from_str(&value)
                    .context("parse resolved dependency representation ID")
            })
            .transpose()?;
        Dependency::new(
            source_resource_id,
            DependencyKind::new(self.kind).context("validate dependency kind")?,
            target,
            resolved_representation_id,
            self.required,
            self.authored_reference,
        )
        .context("validate dependency")
    }
}

fn dependency_dependents(args: &DependencyTargetArgs, json: bool) -> Result<()> {
    let target = parse_dependency_target(args.target_kind, &args.target_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .dependents(
            target,
            DependencyQueryLimits::new(args.query.max_depth, args.query.max_representations)?,
            &query_page_request(&args.query.page)?,
        )
        .context("load dependents")?;
    print_dependency_query_page(&page, json)
}

fn dependency_dependencies(args: &DependencyRepresentationQueryArgs, json: bool) -> Result<()> {
    let source = parse_representation_id(&args.representation_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .dependencies(
            source,
            DependencyQueryLimits::new(args.query.max_depth, args.query.max_representations)?,
            &query_page_request(&args.query.page)?,
        )
        .context("load dependencies")?;
    print_dependency_query_page(&page, json)
}

fn print_dependency_query_page(
    page: &postproject_core::QueryPage<postproject_core::DependencyQueryMatch>,
    json: bool,
) -> Result<()> {
    let view = QueryPageView {
        items: page
            .items()
            .iter()
            .map(|item| {
                Ok(DependencyMatchView {
                    target: object_ref_view(match item.target() {
                        DependencyTarget::Asset(id) => ObjectRef::Asset(id),
                        DependencyTarget::Representation(id) => ObjectRef::Representation(id),
                        _ => unreachable!("unsupported dependency target"),
                    })?,
                    depth: item.depth(),
                })
            })
            .collect::<Result<Vec<_>>>()?,
        next_cursor: page.next_cursor().map(|cursor| cursor.as_str().to_owned()),
        traversal_truncated: page.traversal_truncated(),
    };
    if json {
        print_json(&view)
    } else {
        for item in view.items {
            println!("{}\t{}\t{}", item.target.kind, item.target.id, item.depth);
        }
        if let Some(cursor) = view.next_cursor {
            println!("next_cursor\t{cursor}");
        }
        println!("traversal_truncated\t{}", view.traversal_truncated);
        Ok(())
    }
}

fn dependency_set_view(set: &DependencySet) -> Result<DependencySetView> {
    Ok(DependencySetView {
        source_representation_id: set.source_representation_id().to_string(),
        recorded_at_revision: set.recorded_at_revision(),
        status: dependency_set_status_name(set.status())?,
        dependencies: set
            .dependencies()
            .iter()
            .map(dependency_view)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn dependency_view(dependency: &Dependency) -> Result<DependencyView> {
    let target = match dependency.target() {
        DependencyTarget::Asset(id) => ObjectRef::Asset(id),
        DependencyTarget::Representation(id) => ObjectRef::Representation(id),
        _ => bail!("dependency target kind is not supported by this CLI"),
    };
    Ok(DependencyView {
        source_resource_id: dependency.source_resource_id().map(|id| id.to_string()),
        kind: dependency.kind().as_str().to_owned(),
        target: object_ref_view(target)?,
        resolved_representation_id: dependency
            .resolved_representation_id()
            .map(|id| id.to_string()),
        required: dependency.is_required(),
        authored_reference: dependency.authored_reference().to_owned(),
    })
}

fn dependency_set_status_name(status: DependencySetStatus) -> Result<&'static str> {
    match status {
        DependencySetStatus::Current => Ok("current"),
        DependencySetStatus::NeedsExtraction => Ok("needs_extraction"),
        _ => bail!("dependency-set status is not supported by this CLI"),
    }
}

fn activity_add(
    args: ActivityAddArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let kind = ActivityKind::new(args.kind).context("validate activity kind")?;
    let inputs = args
        .inputs
        .into_iter()
        .map(|edge| ActivityInput::new(edge.representation_id, edge.role))
        .collect();
    let outputs = args
        .outputs
        .into_iter()
        .map(|edge| ActivityOutput::new(edge.representation_id, edge.role))
        .collect();
    let mut activity = Activity::new(ActivityId::new(), kind, inputs, outputs)
        .context("validate activity")?
        .with_timing(
            args.started_at_unix_micros.map(Timestamp::from_unix_micros),
            args.finished_at_unix_micros
                .map(Timestamp::from_unix_micros),
        )
        .context("validate activity timing")?;
    if let Some(name) = args.tool_name {
        let tool = ToolIdentity::new(name, args.tool_version, args.tool_uri)
            .context("validate activity tool")?;
        activity = activity.with_tool(tool);
    } else if args.tool_version.is_some() || args.tool_uri.is_some() {
        bail!("--tool-version and --tool-uri require --tool-name");
    }
    let agent_identifier = match (args.agent_identifier_scheme, args.agent_identifier_value) {
        (Some(scheme), Some(value)) => Some(
            ExternalIdentifier::new(
                IdentifierScheme::new(scheme).context("validate agent identifier scheme")?,
                value,
                args.agent_identifier_qualifier,
            )
            .context("validate agent identifier")?,
        ),
        (None, None) if args.agent_identifier_qualifier.is_none() => None,
        _ => bail!(
            "agent identifier scheme and value must be supplied together; qualifier is optional"
        ),
    };
    if args.agent_name.is_some() || agent_identifier.is_some() {
        let agent = AgentIdentity::new(args.agent_name, agent_identifier)
            .context("validate activity agent")?;
        activity = activity.with_agent(agent);
    }
    let activity_id = activity.id();
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "activity")?;
    set_cli_revision_context(&mut transaction, "Record activity")?;
    transaction
        .create_activity(&activity)
        .context("stage activity")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit activity")?;
    drop(transaction);
    let stored_activity = production
        .activities()
        .context("reload activity snapshots")?
        .into_iter()
        .find(|candidate| candidate.id() == activity_id)
        .context("committed activity is missing")?;
    let view = activity_view(&stored_activity);

    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!("recorded activity {} ({})", view.id, view.kind);
        Ok(())
    }
}

fn activity_view(activity: &Activity) -> ActivityView {
    ActivityView {
        id: activity.id().to_string(),
        kind: activity.kind().as_str().to_owned(),
        started_at_unix_micros: activity
            .started_at()
            .map(postproject_core::Timestamp::as_unix_micros),
        finished_at_unix_micros: activity
            .finished_at()
            .map(postproject_core::Timestamp::as_unix_micros),
        tool: activity.tool().map(|tool| ToolView {
            name: tool.name().to_owned(),
            version: tool.version().map(str::to_owned),
            uri: tool.uri().map(str::to_owned),
        }),
        agent: activity.agent().map(|agent| AgentView {
            name: agent.name().map(str::to_owned),
            identifier: agent.identifier().map(|identifier| AgentIdentifierView {
                scheme: identifier.scheme().as_str().to_owned(),
                value: identifier.value().to_owned(),
                qualifier: identifier.qualifier().map(str::to_owned),
            }),
        }),
        inputs: activity
            .inputs()
            .iter()
            .map(|input| ActivityEdgeView {
                representation_id: input.representation_id().to_string(),
                role: input.role().map(|role| role.as_str().to_owned()),
                snapshot: input.snapshot().map(activity_edge_snapshot_view),
            })
            .collect(),
        outputs: activity
            .outputs()
            .iter()
            .map(|output| ActivityEdgeView {
                representation_id: output.representation_id().to_string(),
                role: output.role().map(|role| role.as_str().to_owned()),
                snapshot: output.snapshot().map(activity_edge_snapshot_view),
            })
            .collect(),
    }
}

fn job_request(
    args: JobRequestArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let input_ids = args
        .inputs
        .iter()
        .map(|value| parse_representation_id(value))
        .collect::<Result<Vec<_>>>()?;
    let output_kind = match args.output_kind {
        RepresentationKindArg::Original => RepresentationKind::Original,
        RepresentationKindArg::Proxy => RepresentationKind::Proxy,
        RepresentationKindArg::Optimized => RepresentationKind::Optimized,
        RepresentationKindArg::Derived => RepresentationKind::Derived,
    };
    let job = Job::new(
        JobId::new(),
        JobKind::new(args.kind).context("validate job kind")?,
        input_ids,
        RequestedJobOutput::new(
            parse_asset_id(&args.output_asset_id)?,
            output_kind,
            args.target_root,
        )
        .context("validate requested output")?,
    )
    .context("validate job request")?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "job request")?;
    set_cli_revision_context(&mut transaction, "Request job")?;
    transaction.request_job(&job).context("request job")?;
    if let Some(profile) = args.profile {
        let property = executor_profile_property()?;
        let value = MetadataValue::string(profile).context("validate executor profile")?;
        transaction
            .add_metadata_value(ObjectRef::Job(job.id()), &property, &value)
            .context("record executor profile")?;
    }
    let receipt = transaction
        .commit_with_receipt()
        .context("commit job request")?;

    let view = job_view(&job);
    if json {
        print_json_with_receipt(&view, &receipt)
    } else {
        println!("requested job {} ({})", view.id, view.kind);
        Ok(())
    }
}

fn job_claim(args: JobClaimArgs, json: bool, base_revision: Option<CliDecisionBase>) -> Result<()> {
    let job_id = parse_job_id(&args.job_id)?;
    let tool = ToolIdentity::new(args.tool_name, args.tool_version, args.tool_uri)
        .context("validate job worker tool")?;
    let agent_identifier = match (args.agent_identifier_scheme, args.agent_identifier_value) {
        (Some(scheme), Some(value)) => Some(
            ExternalIdentifier::new(
                IdentifierScheme::new(scheme).context("validate agent identifier scheme")?,
                value,
                args.agent_identifier_qualifier,
            )
            .context("validate agent identifier")?,
        ),
        (None, None) if args.agent_identifier_qualifier.is_none() => None,
        _ => bail!(
            "agent identifier scheme and value must be supplied together; qualifier is optional"
        ),
    };
    let agent = if args.agent_name.is_some() || agent_identifier.is_some() {
        Some(
            AgentIdentity::new(args.agent_name, agent_identifier)
                .context("validate job worker agent")?,
        )
    } else {
        None
    };
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "job claim")?;
    set_cli_revision_context(&mut transaction, "Claim job")?;
    transaction
        .claim_job(
            job_id,
            &tool,
            agent.as_ref(),
            Timestamp::from_unix_micros(args.now_unix_micros),
            Timestamp::from_unix_micros(args.expires_at_unix_micros),
        )
        .context("claim job")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit job claim")?;
    drop(transaction);
    print_job_result(&production, job_id, json, "claimed", &receipt)
}

fn job_renew(
    args: &JobLeaseArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let job_id = parse_job_id(&args.job_id)?;
    let claim_id = parse_job_claim_id(&args.claim_id)?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction =
        begin_cli_transaction(&mut production, base_revision, "job claim renewal")?;
    set_cli_revision_context(&mut transaction, "Renew job claim")?;
    transaction
        .renew_job_claim(
            job_id,
            claim_id,
            Timestamp::from_unix_micros(args.now_unix_micros),
            Timestamp::from_unix_micros(args.expires_at_unix_micros),
        )
        .context("renew job claim")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit job claim renewal")?;
    drop(transaction);
    print_job_result(&production, job_id, json, "renewed", &receipt)
}

fn job_release(
    args: &JobClaimTokenArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let job_id = parse_job_id(&args.job_id)?;
    let claim_id = parse_job_claim_id(&args.claim_id)?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction =
        begin_cli_transaction(&mut production, base_revision, "job claim release")?;
    set_cli_revision_context(&mut transaction, "Release job claim")?;
    transaction
        .release_job_claim(job_id, claim_id)
        .context("release job claim")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit job claim release")?;
    drop(transaction);
    print_job_result(&production, job_id, json, "released", &receipt)
}

fn job_complete(
    args: &JobCompleteArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let job_id = parse_job_id(&args.job_id)?;
    let claim_id = parse_job_claim_id(&args.claim_id)?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let job = production.job(job_id).context("load claimed job")?;
    let JobState::Claimed(claim) = job.state() else {
        bail!("job must be claimed before completion");
    };
    let output = prepare_representation(
        job.requested_output().asset_id(),
        job.requested_output().representation_kind(),
        &args.output,
    )
    .context("prepare job output")?;
    let now = Timestamp::from_unix_micros(args.now_unix_micros);
    let inputs = job
        .inputs()
        .iter()
        .copied()
        .map(|id| ActivityInput::new(id, None))
        .collect();
    let outputs = vec![ActivityOutput::new(output.representation().id(), None)];
    let mut activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new(job.kind().as_str()).context("validate completion activity kind")?,
        inputs,
        outputs,
    )
    .context("validate completion activity")?
    .with_timing(None, Some(now))
    .context("validate completion timing")?
    .with_tool(claim.tool().clone());
    if let Some(agent) = claim.agent() {
        activity = activity.with_agent(agent.clone());
    }
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "job completion")?;
    set_cli_revision_context(&mut transaction, "Complete job")?;
    transaction
        .complete_job(job_id, claim_id, now, &output, &activity)
        .context("complete job")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit job completion")?;
    drop(transaction);
    print_job_result(&production, job_id, json, "completed", &receipt)
}

fn job_fail(args: JobFailArgs, json: bool, base_revision: Option<CliDecisionBase>) -> Result<()> {
    let job_id = parse_job_id(&args.job_id)?;
    let claim_id = parse_job_claim_id(&args.claim_id)?;
    let failure = JobFailure::new(args.diagnostic).context("validate job failure")?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction = begin_cli_transaction(&mut production, base_revision, "job failure")?;
    set_cli_revision_context(&mut transaction, "Fail job")?;
    transaction
        .fail_job(
            job_id,
            claim_id,
            Timestamp::from_unix_micros(args.now_unix_micros),
            &failure,
        )
        .context("fail job")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit job failure")?;
    drop(transaction);
    print_job_result(&production, job_id, json, "failed", &receipt)
}

fn job_cancel(args: &JobIdArgs, json: bool, base_revision: Option<CliDecisionBase>) -> Result<()> {
    let job_id = parse_job_id(&args.job_id)?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let mut transaction =
        begin_cli_transaction(&mut production, base_revision, "job cancellation")?;
    set_cli_revision_context(&mut transaction, "Cancel job")?;
    transaction.cancel_job(job_id).context("cancel job")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit job cancellation")?;
    drop(transaction);
    print_job_result(&production, job_id, json, "cancelled", &receipt)
}

fn print_job_result(
    production: &SqliteProduction,
    job_id: JobId,
    json: bool,
    action: &str,
    receipt: &CommitReceipt,
) -> Result<()> {
    let job = production.job(job_id).context("reload job")?;
    let view = job_view(&job);
    if json {
        print_json_with_receipt(&view, receipt)
    } else {
        println!("{action} job {}", view.id);
        Ok(())
    }
}

fn job_show(args: &JobIdArgs, json: bool) -> Result<()> {
    let job_id = parse_job_id(&args.job_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let view = job_view(&production.job(job_id).context("load job")?);
    if json {
        print_json(&view)
    } else {
        println!("{}\t{}\t{}", view.id, view.state, view.kind);
        Ok(())
    }
}

fn job_list(args: &JobListArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .jobs(
            &JobQuery::new(
                args.state.map(job_state_kind),
                args.kind
                    .as_deref()
                    .map(JobKind::new)
                    .transpose()
                    .context("validate job kind filter")?,
            ),
            &query_page_request(&args.page)?,
        )
        .context("load jobs")?;
    let view = QueryPageView {
        items: page.items().iter().map(job_view).collect::<Vec<_>>(),
        next_cursor: page.next_cursor().map(|cursor| cursor.as_str().to_owned()),
        traversal_truncated: false,
    };
    if json {
        print_json(&view)
    } else {
        for item in view.items {
            println!("{}\t{}\t{}", item.id, item.state, item.kind);
        }
        if let Some(cursor) = view.next_cursor {
            println!("next_cursor\t{cursor}");
        }
        Ok(())
    }
}

fn query_page_view<T, U>(
    page: &QueryPage<T>,
    view: impl FnMut(&T) -> Result<U>,
) -> Result<QueryPageView<U>> {
    Ok(QueryPageView {
        items: page.items().iter().map(view).collect::<Result<Vec<_>>>()?,
        next_cursor: page.next_cursor().map(|cursor| cursor.as_str().to_owned()),
        traversal_truncated: page.traversal_truncated(),
    })
}

/// Prints one query page; `traversal` adds the explicit truncation line used
/// by bounded traversal queries.
fn print_query_page<T: Serialize>(
    view: &QueryPageView<T>,
    json: bool,
    traversal: bool,
    print_item: impl Fn(&T),
) -> Result<()> {
    if json {
        print_json(view)
    } else {
        for item in &view.items {
            print_item(item);
        }
        print_page_trailer(view, traversal);
        Ok(())
    }
}

fn print_page_trailer<T>(view: &QueryPageView<T>, traversal: bool) {
    if let Some(cursor) = &view.next_cursor {
        println!("next_cursor\t{cursor}");
    }
    if traversal {
        println!("traversal_truncated\t{}", view.traversal_truncated);
    }
}

fn query_page_request(args: &QueryPageArgs) -> Result<QueryPageRequest> {
    let cursor = args
        .cursor
        .as_deref()
        .map(QueryCursor::new)
        .transpose()
        .context("validate query cursor")?;
    QueryPageRequest::new(args.limit, cursor).context("validate query page")
}

const fn job_state_kind(state: JobStateArg) -> JobStateKind {
    match state {
        JobStateArg::Requested => JobStateKind::Requested,
        JobStateArg::Claimed => JobStateKind::Claimed,
        JobStateArg::Succeeded => JobStateKind::Succeeded,
        JobStateArg::Failed => JobStateKind::Failed,
        JobStateArg::Cancelled => JobStateKind::Cancelled,
    }
}

fn job_plan(args: &JobPlanArgs, json: bool) -> Result<()> {
    let artifact_ids = args
        .artifacts
        .iter()
        .map(|value| parse_representation_id(value))
        .collect::<Result<Vec<_>>>()?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let plans = production
        .plan_regeneration(&artifact_ids)
        .context("plan artifact regeneration")?;
    let views = plans
        .iter()
        .map(|plan| {
            let target = ObjectRef::Job(plan.job().id());
            let parameters = plan
                .parameters()
                .iter()
                .map(|assertion| metadata_assertion_view(target, assertion))
                .collect::<Result<Vec<_>>>()?;
            Ok(RegenerationPlanView {
                artifact_representation_id: plan.artifact_representation_id().to_string(),
                job: job_view(plan.job()),
                parameters,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if json {
        print_json(&views)
    } else {
        for view in views {
            println!(
                "{}\t{}\t{}",
                view.artifact_representation_id, view.job.id, view.job.kind
            );
        }
        Ok(())
    }
}

struct PreparedExecutorJob {
    job: Job,
    profile: String,
    parameters: Vec<MetadataAssertion>,
    input: PathBuf,
    target_root: PathBuf,
}

fn job_run(args: &JobRunArgs, json: bool, base_revision: Option<CliDecisionBase>) -> Result<()> {
    if args.timeout_seconds == 0 {
        bail!("executor timeout must be greater than zero");
    }
    if args.lease_seconds == 0 {
        bail!("job lease must be greater than zero");
    }
    let root_mappings = prepare_root_mappings(&args.root_mappings)?;
    let lease = Duration::from_secs(args.lease_seconds);
    let heartbeat_interval = lease / 3;
    let executor = FfmpegExecutor::with_executable(&args.ffmpeg)
        .with_timeout(Duration::from_secs(args.timeout_seconds))
        .context("configure executor timeout")?
        .with_heartbeat_interval(heartbeat_interval)
        .context("configure executor heartbeat")?;
    match executor.capability().context("probe ffmpeg capability")? {
        ExecutorCapability::Available { .. } => {}
        ExecutorCapability::Unavailable { reason } | ExecutorCapability::Failed { reason } => {
            bail!("reference executor unavailable: {reason}");
        }
        _ => bail!("reference executor reported an unknown capability state"),
    }

    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let jobs = requested_jobs(&production)?;
    let mut prepared = Vec::new();
    for job in jobs {
        if let Some(candidate) = prepare_executor_job(&production, job, &root_mappings)? {
            prepared.push(candidate);
            if args.once {
                break;
            }
        }
    }

    let mut views = Vec::with_capacity(prepared.len());
    for candidate in prepared {
        views.push(run_executor_job(
            &mut production,
            &executor,
            &candidate,
            lease,
            base_revision,
        )?);
    }
    if json {
        print_json(&views)
    } else {
        if views.is_empty() {
            println!("no eligible requested jobs");
        }
        for view in views {
            println!(
                "{}\t{}\t{}",
                view.job.id,
                view.job.state,
                view.output.as_deref().unwrap_or("-")
            );
        }
        Ok(())
    }
}

fn requested_jobs(production: &SqliteProduction) -> Result<Vec<Job>> {
    let query = JobQuery::new(Some(JobStateKind::Requested), None);
    let mut cursor = None;
    let mut jobs = Vec::new();
    loop {
        let page = production
            .jobs(
                &query,
                &QueryPageRequest::new(1_000, cursor).context("prepare requested-job page")?,
            )
            .context("load requested jobs")?;
        jobs.extend_from_slice(page.items());
        let Some(next) = page.next_cursor().cloned() else {
            return Ok(jobs);
        };
        cursor = Some(next);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "eligibility validates job metadata, domain shape, resolution, and target mapping"
)]
fn prepare_executor_job(
    production: &SqliteProduction,
    job: Job,
    root_mappings: &[MediaRootMapping],
) -> Result<Option<PreparedExecutorJob>> {
    if !matches!(
        job.kind().as_str(),
        postproject_media::GENERATE_PROXY_JOB_KIND | postproject_media::GENERATE_THUMBNAIL_JOB_KIND
    ) {
        return Ok(None);
    }
    let parameters = production
        .metadata(ObjectRef::Job(job.id()))
        .context("load executor job parameters")?;
    let property = executor_profile_property()?;
    let profiles = parameters
        .iter()
        .filter(|assertion| assertion.property() == &property)
        .collect::<Vec<_>>();
    let [profile] = profiles.as_slice() else {
        bail!(
            "eligible job {} must carry exactly one executor profile",
            job.id()
        );
    };
    let profile = profile
        .value()
        .as_string()
        .ok_or_else(|| anyhow::anyhow!("executor profile on job {} must be a string", job.id()))?;
    if !FfmpegExecutor::supports(job.kind().as_str(), profile) {
        bail!(
            "job {} has unsupported executor profile {profile} for {}",
            job.id(),
            job.kind().as_str()
        );
    }
    let output_kind_matches = match job.kind().as_str() {
        postproject_media::GENERATE_PROXY_JOB_KIND => {
            job.requested_output().representation_kind() == RepresentationKind::Proxy
        }
        postproject_media::GENERATE_THUMBNAIL_JOB_KIND => {
            job.requested_output().representation_kind() == RepresentationKind::Derived
        }
        _ => false,
    };
    if !output_kind_matches {
        bail!(
            "job {} requests an output kind incompatible with {}",
            job.id(),
            job.kind().as_str()
        );
    }
    let [input_id] = job.inputs() else {
        bail!(
            "reference executor job {} must have exactly one input",
            job.id()
        );
    };
    let representations = production
        .representations(job.requested_output().asset_id())
        .context("load executor input asset representations")?;
    let representation = representations
        .iter()
        .find(|representation| representation.id() == *input_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "reference executor input {} must belong to requested output asset {}",
                input_id,
                job.requested_output().asset_id()
            )
        })?;
    if representation
        .content_structure()
        .single_resource_id()
        .is_none()
    {
        bail!("reference executor input {input_id} must be a single-file representation");
    }
    let input = load_resolution_input(production, representation.clone(), false)?;
    let scope = SearchScope::new(production.media_roots()?, root_mappings.to_vec());
    let resolution = resolve_representations(
        &MediaResolver::default(),
        &scope,
        std::slice::from_ref(&input),
        &FfprobeInspector::default(),
    )?
    .pop()
    .context("resolve reference executor input")?;
    if resolution.availability() != RepresentationAvailability::Online {
        bail!("reference executor input {input_id} is not unambiguously online");
    }
    let [resource] = resolution.resources() else {
        bail!("reference executor input must resolve to one resource");
    };
    let [candidate] = resource.candidates() else {
        bail!("reference executor input must resolve to one candidate");
    };
    let input = local_file_path(candidate.uri()).context("convert executor input file URI")?;
    let root_name = job.requested_output().target_root().ok_or_else(|| {
        anyhow::anyhow!(
            "reference executor job {} must name a target root",
            job.id()
        )
    })?;
    let matching_roots = root_mappings
        .iter()
        .filter(|mapping| mapping.name() == root_name)
        .collect::<Vec<_>>();
    let [target_root] = matching_roots.as_slice() else {
        bail!(
            "target root {root_name} must have exactly one machine mapping for job {}",
            job.id()
        );
    };
    Ok(Some(PreparedExecutorJob {
        job,
        profile: profile.to_owned(),
        parameters,
        input,
        target_root: target_root.directory().to_path_buf(),
    }))
}

fn run_executor_job(
    production: &mut SqliteProduction,
    executor: &FfmpegExecutor,
    prepared: &PreparedExecutorJob,
    lease: Duration,
    base_revision: Option<CliDecisionBase>,
) -> Result<JobRunView> {
    let job_id = prepared.job.id();
    let started = Timestamp::now().context("read executor start time")?;
    let claim_tool = ToolIdentity::new(
        "PostProject reference executor",
        Some(env!("CARGO_PKG_VERSION").to_owned()),
        Some("https://postproject.org/".to_owned()),
    )
    .context("construct executor identity")?;
    let claim = {
        let mut transaction =
            begin_cli_transaction(production, base_revision, "executor job claim")?;
        set_cli_revision_context(&mut transaction, "Claim reference-executor job")?;
        let claim = transaction
            .claim_job(
                job_id,
                &claim_tool,
                None,
                started,
                timestamp_after(started, lease)?,
            )
            .context("claim executor job")?;
        transaction.commit().context("commit executor job claim")?;
        claim
    };
    let request = ExecutionRequest::new(
        job_id,
        claim.id(),
        prepared.job.kind().clone(),
        &prepared.profile,
        &prepared.input,
        &prepared.target_root,
    )
    .context("prepare local execution")?;
    let outcome = {
        let mut heartbeat = || {
            let now = Timestamp::now()?;
            let mut transaction = match base_revision {
                Some(CliDecisionBase::Revision(base_revision)) => {
                    production.begin_transaction_at(base_revision)?
                }
                Some(CliDecisionBase::Scoped(base)) => production.begin_edit(base)?,
                None => production.begin_transaction()?,
            };
            let origin = OriginIdentity::new(
                "postproject-cli",
                Some(env!("CARGO_PKG_VERSION").to_owned()),
                None,
            )?;
            let context =
                RevisionContext::new(Some(origin), Some("Renew executor job claim".to_owned()))?;
            transaction.set_revision_context(context)?;
            transaction.renew_job_claim(job_id, claim.id(), now, timestamp_after(now, lease)?)?;
            transaction.commit()
        };
        executor.execute(&request, &mut heartbeat)
    };
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            let diagnostic = format!("reference executor error: {error}");
            let _ = fail_executor_job(production, job_id, claim.id(), &diagnostic, base_revision);
            return Err(error).context("execute claimed job");
        }
    };
    match outcome {
        ExecutionOutcome::Unavailable { reason } => {
            let mut transaction =
                begin_cli_transaction(production, base_revision, "unavailable executor release")?;
            set_cli_revision_context(&mut transaction, "Release unavailable executor job")?;
            transaction
                .release_job_claim(job_id, claim.id())
                .context("release unavailable executor job")?;
            transaction
                .commit()
                .context("commit unavailable executor release")?;
            bail!("reference executor unavailable: {reason}");
        }
        ExecutionOutcome::Failed { diagnostic } => {
            fail_executor_job(production, job_id, claim.id(), &diagnostic, base_revision)?;
            let job = production.job(job_id).context("reload failed job")?;
            Ok(JobRunView {
                job: job_view(&job),
                output: None,
            })
        }
        ExecutionOutcome::Completed {
            output,
            ffmpeg_version,
        } => complete_executor_job(
            production,
            prepared,
            claim.id(),
            started,
            &output,
            ffmpeg_version,
            base_revision,
        ),
        _ => bail!("reference executor returned an unknown outcome"),
    }
}

fn complete_executor_job(
    production: &mut SqliteProduction,
    prepared: &PreparedExecutorJob,
    claim_id: JobClaimId,
    started: Timestamp,
    output_path: &Path,
    ffmpeg_version: String,
    base_revision: Option<CliDecisionBase>,
) -> Result<JobRunView> {
    let job_id = prepared.job.id();
    let output = match prepare_representation(
        prepared.job.requested_output().asset_id(),
        prepared.job.requested_output().representation_kind(),
        output_path,
    ) {
        Ok(output) => output,
        Err(error) => {
            let _ = fs::remove_file(output_path);
            let diagnostic = format!("cannot prepare executor output: {error}");
            fail_executor_job(production, job_id, claim_id, &diagnostic, base_revision)?;
            return Err(error).context("prepare executor output");
        }
    };
    let finished = Timestamp::now().context("read executor completion time")?;
    let inputs = prepared
        .job
        .inputs()
        .iter()
        .copied()
        .map(|id| ActivityInput::new(id, None))
        .collect();
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new(prepared.job.kind().as_str())
            .context("validate executor activity kind")?,
        inputs,
        vec![ActivityOutput::new(output.representation().id(), None)],
    )
    .context("validate executor activity")?
    .with_timing(Some(started), Some(finished))
    .context("validate executor activity timing")?
    .with_tool(
        ToolIdentity::new(
            "ffmpeg",
            Some(ffmpeg_version),
            Some("https://ffmpeg.org/".to_owned()),
        )
        .context("validate ffmpeg tool identity")?,
    );
    let activity_id = activity.id();
    let completion = (|| -> Result<()> {
        let mut transaction =
            begin_cli_transaction(production, base_revision, "executor job completion")?;
        set_cli_revision_context(&mut transaction, "Complete reference-executor job")?;
        transaction
            .complete_job(job_id, claim_id, finished, &output, &activity)
            .context("complete executor job")?;
        for parameter in &prepared.parameters {
            transaction
                .add_metadata_value(
                    ObjectRef::Activity(activity_id),
                    parameter.property(),
                    parameter.value(),
                )
                .context("copy executor activity parameter")?;
        }
        transaction.commit().context("commit executor completion")
    })();
    if let Err(error) = completion {
        let _ = fs::remove_file(output_path);
        return Err(error);
    }
    let job = production.job(job_id).context("reload completed job")?;
    Ok(JobRunView {
        job: job_view(&job),
        output: Some(output_path.display().to_string()),
    })
}

fn fail_executor_job(
    production: &mut SqliteProduction,
    job_id: JobId,
    claim_id: JobClaimId,
    diagnostic: &str,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let failure =
        JobFailure::new(bounded_job_diagnostic(diagnostic)).context("validate executor failure")?;
    let now = Timestamp::now().context("read executor failure time")?;
    let mut transaction = begin_cli_transaction(production, base_revision, "executor job failure")?;
    set_cli_revision_context(&mut transaction, "Fail reference-executor job")?;
    transaction
        .fail_job(job_id, claim_id, now, &failure)
        .context("fail executor job")?;
    transaction.commit().context("commit executor job failure")
}

fn executor_profile_property() -> Result<MetadataProperty> {
    Ok(MetadataProperty::new(
        VocabularyId::new(EXECUTOR_PARAMETER_VOCABULARY)
            .context("validate executor parameter vocabulary")?,
        PropertyId::new(EXECUTOR_PROFILE_PROPERTY).context("validate executor profile property")?,
    ))
}

fn timestamp_after(now: Timestamp, duration: Duration) -> postproject_core::Result<Timestamp> {
    let micros = i64::try_from(duration.as_micros()).map_err(|error| {
        postproject_core::Error::new(
            postproject_core::ErrorKind::InvalidArgument,
            format!("job lease is too large: {error}"),
        )
    })?;
    let expires = now.as_unix_micros().checked_add(micros).ok_or_else(|| {
        postproject_core::Error::new(
            postproject_core::ErrorKind::InvalidArgument,
            "job lease expiry is outside the supported timestamp range",
        )
    })?;
    Ok(Timestamp::from_unix_micros(expires))
}

fn bounded_job_diagnostic(diagnostic: &str) -> String {
    if diagnostic.len() <= MAX_JOB_DIAGNOSTIC_BYTES {
        return diagnostic.to_owned();
    }
    let mut end = MAX_JOB_DIAGNOSTIC_BYTES;
    while !diagnostic.is_char_boundary(end) {
        end -= 1;
    }
    diagnostic[..end].to_owned()
}

fn job_view(job: &Job) -> JobView {
    let (claim_id, claim_expires_at_unix_micros, claim_tool, claim_agent) = match job.state() {
        JobState::Claimed(claim) => (
            Some(claim.id().to_string()),
            Some(claim.expires_at().as_unix_micros()),
            Some(ToolView {
                name: claim.tool().name().to_owned(),
                version: claim.tool().version().map(str::to_owned),
                uri: claim.tool().uri().map(str::to_owned),
            }),
            claim.agent().map(|agent| AgentView {
                name: agent.name().map(str::to_owned),
                identifier: agent.identifier().map(|identifier| AgentIdentifierView {
                    scheme: identifier.scheme().as_str().to_owned(),
                    value: identifier.value().to_owned(),
                    qualifier: identifier.qualifier().map(str::to_owned),
                }),
            }),
        ),
        _ => (None, None, None, None),
    };
    let (completion_activity_id, completion_representation_id) = match job.state() {
        JobState::Succeeded(completion) => (
            Some(completion.activity_id().to_string()),
            Some(completion.representation_id().to_string()),
        ),
        _ => (None, None),
    };
    JobView {
        id: job.id().to_string(),
        kind: job.kind().as_str().to_owned(),
        inputs: job.inputs().iter().map(ToString::to_string).collect(),
        output_asset_id: job.requested_output().asset_id().to_string(),
        output_kind: representation_kind(job.requested_output().representation_kind()),
        target_root: job.requested_output().target_root().map(str::to_owned),
        state: job_state(job.state()),
        claim_id,
        claim_expires_at_unix_micros,
        claim_tool,
        claim_agent,
        completion_activity_id,
        completion_representation_id,
        failure_diagnostic: match job.state() {
            JobState::Failed(failure) => Some(failure.diagnostic().to_owned()),
            _ => None,
        },
    }
}

fn activity_edge_snapshot_view(
    snapshot: &postproject_core::ActivityEdgeSnapshot,
) -> ActivityEdgeSnapshotView {
    ActivityEdgeSnapshotView {
        revision_sequence: snapshot.revision_sequence(),
        fingerprints: snapshot
            .fingerprints()
            .iter()
            .map(|fingerprint| FingerprintSnapshotView {
                algorithm: fingerprint.algorithm().to_owned(),
                version: fingerprint.version(),
                value_hex: hex::encode(fingerprint.value()),
                observed_revision_sequence: fingerprint.observed_revision_sequence(),
            })
            .collect(),
    }
}

fn artifact_reproducibility(args: &ActivityRepresentationArgs, json: bool) -> Result<()> {
    let representation_id = parse_representation_id(&args.representation_id)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let report = production
        .artifact_reproducibility(representation_id)
        .context("evaluate artifact reproducibility")?;
    let view = ArtifactReproducibilityView {
        representation_id: report.representation_id().to_string(),
        reproducible: report.is_reproducible(),
        producing_activity_id: report.producing_activity_id().map(|id| id.to_string()),
        activity_kind: report.activity_kind().map(|kind| kind.as_str().to_owned()),
        issues: report
            .issues()
            .iter()
            .map(artifact_reproducibility_issue_view)
            .collect::<Result<Vec<_>>>()?,
    };

    if json {
        print_json(&view)
    } else {
        println!(
            "{}\t{}",
            view.representation_id,
            if view.reproducible {
                "reproducible"
            } else {
                "not reproducible"
            }
        );
        for issue in &view.issues {
            println!("issue\t{}", artifact_reproducibility_issue_name(issue));
        }
        Ok(())
    }
}

fn artifact_reproducibility_issue_view(
    issue: &ArtifactReproducibilityIssue,
) -> Result<ArtifactReproducibilityIssueView> {
    match issue {
        ArtifactReproducibilityIssue::ProducingActivityMissing => {
            Ok(ArtifactReproducibilityIssueView::ProducingActivityMissing)
        }
        ArtifactReproducibilityIssue::ProducingActivityAmbiguous { activity_count } => Ok(
            ArtifactReproducibilityIssueView::ProducingActivityAmbiguous {
                activity_count: *activity_count,
            },
        ),
        ArtifactReproducibilityIssue::ToolIdentityMissing { activity_id } => {
            Ok(ArtifactReproducibilityIssueView::ToolIdentityMissing {
                activity_id: activity_id.to_string(),
            })
        }
        ArtifactReproducibilityIssue::ParametersMissing { activity_id } => {
            Ok(ArtifactReproducibilityIssueView::ParametersMissing {
                activity_id: activity_id.to_string(),
            })
        }
        ArtifactReproducibilityIssue::InputRepresentationMissing {
            activity_id,
            representation_id,
        } => Ok(
            ArtifactReproducibilityIssueView::InputRepresentationMissing {
                activity_id: activity_id.to_string(),
                representation_id: representation_id.to_string(),
            },
        ),
        _ => bail!("unsupported artifact reproducibility issue"),
    }
}

fn artifact_reproducibility_issue_name(issue: &ArtifactReproducibilityIssueView) -> &'static str {
    match issue {
        ArtifactReproducibilityIssueView::ProducingActivityMissing => "producing_activity_missing",
        ArtifactReproducibilityIssueView::ProducingActivityAmbiguous { .. } => {
            "producing_activity_ambiguous"
        }
        ArtifactReproducibilityIssueView::ToolIdentityMissing { .. } => "tool_identity_missing",
        ArtifactReproducibilityIssueView::ParametersMissing { .. } => "parameters_missing",
        ArtifactReproducibilityIssueView::InputRepresentationMissing { .. } => {
            "input_representation_missing"
        }
    }
}

fn artifact_evaluate(args: &ArtifactEvaluateArgs, json: bool) -> Result<()> {
    let representation_id = parse_representation_id(&args.representation_id)?;
    let limits = ArtifactEvaluationLimits::new(args.max_depth, args.max_representations)
        .context("validate artifact evaluation bounds")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let evaluation = production
        .evaluate_artifact(representation_id, limits)
        .context("evaluate artifact")?;
    let view = ArtifactEvaluationView {
        representation_id: evaluation.representation_id().to_string(),
        state: artifact_knowledge_state_name(evaluation.state())?,
        visited_representations: evaluation.visited_representations(),
        truncated: evaluation.is_truncated(),
        reasons: evaluation
            .reasons()
            .iter()
            .map(artifact_reason_view)
            .collect::<Result<Vec<_>>>()?,
    };

    if json {
        print_json(&view)
    } else {
        println!(
            "{}\t{}\t{} representation(s) visited",
            view.representation_id, view.state, view.visited_representations
        );
        for reason in &view.reasons {
            let (kind, representation_id) = artifact_reason_summary(reason);
            println!("reason\t{kind}\t{representation_id}");
        }
        Ok(())
    }
}

fn artifact_stale(args: &ArtifactStaleArgs, json: bool) -> Result<()> {
    let source = args
        .source
        .as_deref()
        .map(parse_representation_id)
        .transpose()?;
    let limits = ArtifactEvaluationLimits::new(args.max_depth, args.max_representations)
        .context("validate artifact evaluation bounds")?;
    let request = query_page_request(&args.page)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .stale_artifacts(StaleArtifactQuery::new(source, limits), &request)
        .context("query stale artifacts")?;
    let view = query_page_view(&page, |id| Ok(representation_ref_view(*id)))?;
    print_query_page(&view, json, true, |item| {
        println!("{}", item.representation_id);
    })
}

fn artifact_reason_view(reason: &ArtifactKnowledgeReason) -> Result<ArtifactReasonView> {
    match reason {
        ArtifactKnowledgeReason::ProducingActivityMissing { representation_id } => {
            Ok(ArtifactReasonView::ProducingActivityMissing {
                representation_id: representation_id.to_string(),
            })
        }
        ArtifactKnowledgeReason::ProducingActivityAmbiguous {
            representation_id,
            activity_count,
        } => Ok(ArtifactReasonView::ProducingActivityAmbiguous {
            representation_id: representation_id.to_string(),
            activity_count: *activity_count,
        }),
        ArtifactKnowledgeReason::SnapshotAbsent {
            activity_id,
            representation_id,
            edge,
        } => Ok(ArtifactReasonView::SnapshotAbsent {
            activity_id: activity_id.to_string(),
            representation_id: representation_id.to_string(),
            edge: artifact_edge_name(*edge)?,
        }),
        ArtifactKnowledgeReason::FingerprintEvidenceMissing {
            activity_id,
            representation_id,
            edge,
            algorithm,
            version,
            snapshot_value,
            current_value,
        } => Ok(ArtifactReasonView::FingerprintEvidenceMissing {
            activity_id: activity_id.to_string(),
            representation_id: representation_id.to_string(),
            edge: artifact_edge_name(*edge)?,
            fingerprint_algorithm: algorithm.clone(),
            fingerprint_version: *version,
            snapshot_value_hex: snapshot_value.as_ref().map(hex::encode),
            current_value_hex: current_value.as_ref().map(hex::encode),
        }),
        ArtifactKnowledgeReason::FingerprintChanged {
            activity_id,
            representation_id,
            edge,
            algorithm,
            version,
            snapshot_value,
            current_value,
        } => Ok(ArtifactReasonView::FingerprintChanged {
            activity_id: activity_id.to_string(),
            representation_id: representation_id.to_string(),
            edge: artifact_edge_name(*edge)?,
            fingerprint_algorithm: algorithm.clone(),
            fingerprint_version: *version,
            snapshot_value_hex: hex::encode(snapshot_value),
            current_value_hex: hex::encode(current_value),
        }),
        ArtifactKnowledgeReason::FingerprintRecomputationPending {
            activity_id,
            representation_id,
            edge,
        } => Ok(ArtifactReasonView::FingerprintRecomputationPending {
            activity_id: activity_id.to_string(),
            representation_id: representation_id.to_string(),
            edge: artifact_edge_name(*edge)?,
        }),
        ArtifactKnowledgeReason::UpstreamNotCurrent {
            representation_id,
            state,
        } => Ok(ArtifactReasonView::UpstreamNotCurrent {
            representation_id: representation_id.to_string(),
            upstream_state: artifact_knowledge_state_name(*state)?,
        }),
        ArtifactKnowledgeReason::TraversalTruncated {
            limit,
            representation_id,
        } => Ok(ArtifactReasonView::TraversalTruncated {
            representation_id: representation_id.to_string(),
            traversal_limit: artifact_traversal_limit_name(*limit)?,
        }),
        _ => artifact_dependency_reason_view(reason),
    }
}

fn artifact_dependency_reason_view(reason: &ArtifactKnowledgeReason) -> Result<ArtifactReasonView> {
    match reason {
        ArtifactKnowledgeReason::DependencySnapshotAbsent {
            activity_id,
            representation_id,
        } => Ok(ArtifactReasonView::DependencySnapshotAbsent {
            activity_id: activity_id.to_string(),
            input_representation_id: representation_id.to_string(),
        }),
        ArtifactKnowledgeReason::DependencyKnowledgeIncomplete {
            activity_id,
            input_representation_id,
            subject_representation_id,
            path,
            issue,
        } => Ok(ArtifactReasonView::DependencyKnowledgeIncomplete {
            activity_id: activity_id.to_string(),
            input_representation_id: input_representation_id.to_string(),
            subject_representation_id: subject_representation_id.to_string(),
            path: artifact_dependency_path_view(path)?,
            issue: artifact_dependency_issue_name(*issue)?,
        }),
        ArtifactKnowledgeReason::DependencyPathChanged {
            activity_id,
            input_representation_id,
            path,
        } => Ok(ArtifactReasonView::DependencyPathChanged {
            activity_id: activity_id.to_string(),
            input_representation_id: input_representation_id.to_string(),
            path: artifact_dependency_path_view(path)?,
        }),
        ArtifactKnowledgeReason::DependencyFingerprintChanged {
            activity_id,
            input_representation_id,
            representation_id,
            path,
            algorithm,
            version,
            snapshot_value,
            current_value,
        } => Ok(ArtifactReasonView::DependencyFingerprintChanged {
            activity_id: activity_id.to_string(),
            input_representation_id: input_representation_id.to_string(),
            representation_id: representation_id.to_string(),
            path: artifact_dependency_path_view(path)?,
            fingerprint_algorithm: algorithm.clone(),
            fingerprint_version: *version,
            snapshot_value_hex: hex::encode(snapshot_value),
            current_value_hex: hex::encode(current_value),
        }),
        ArtifactKnowledgeReason::DependencyFingerprintRecomputationPending {
            activity_id,
            input_representation_id,
            representation_id,
            path,
        } => Ok(
            ArtifactReasonView::DependencyFingerprintRecomputationPending {
                activity_id: activity_id.to_string(),
                input_representation_id: input_representation_id.to_string(),
                representation_id: representation_id.to_string(),
                path: artifact_dependency_path_view(path)?,
            },
        ),
        ArtifactKnowledgeReason::DependencyFingerprintEvidenceMissing {
            activity_id,
            input_representation_id,
            representation_id,
            path,
            algorithm,
            version,
            snapshot_value,
            current_value,
        } => Ok(ArtifactReasonView::DependencyFingerprintEvidenceMissing {
            activity_id: activity_id.to_string(),
            input_representation_id: input_representation_id.to_string(),
            representation_id: representation_id.to_string(),
            path: artifact_dependency_path_view(path)?,
            fingerprint_algorithm: algorithm.clone(),
            fingerprint_version: *version,
            snapshot_value_hex: snapshot_value.as_ref().map(hex::encode),
            current_value_hex: current_value.as_ref().map(hex::encode),
        }),
        _ => bail!("unsupported artifact dependency reason"),
    }
}

fn artifact_dependency_path_view(
    path: &[ArtifactDependencyPathSegment],
) -> Result<Vec<ArtifactDependencyPathView>> {
    path.iter()
        .map(|segment| {
            let target = match segment.target() {
                DependencyTarget::Asset(id) => ObjectRef::Asset(id),
                DependencyTarget::Representation(id) => ObjectRef::Representation(id),
                _ => bail!("unsupported dependency target"),
            };
            Ok(ArtifactDependencyPathView {
                source_representation_id: segment.source_representation_id().to_string(),
                dependency_position: segment.dependency_position(),
                source_resource_id: segment.source_resource_id().map(|id| id.to_string()),
                kind: segment.kind().as_str().to_owned(),
                target: object_ref_view(target)?,
                resolved_representation_id: segment
                    .resolved_representation_id()
                    .map(|id| id.to_string()),
                authored_reference: segment.authored_reference().to_owned(),
            })
        })
        .collect()
}

fn artifact_dependency_issue_name(issue: ArtifactDependencyIssue) -> Result<&'static str> {
    match issue {
        ArtifactDependencyIssue::NeedsExtraction => Ok("needs_extraction"),
        ArtifactDependencyIssue::Unresolved => Ok("unresolved"),
        ArtifactDependencyIssue::DepthTruncated => Ok("depth_truncated"),
        ArtifactDependencyIssue::RepresentationsTruncated => Ok("representations_truncated"),
        _ => bail!("unsupported artifact dependency issue"),
    }
}

fn artifact_reason_summary(reason: &ArtifactReasonView) -> (&'static str, &str) {
    match reason {
        ArtifactReasonView::ProducingActivityMissing { representation_id } => {
            ("producing_activity_missing", representation_id)
        }
        ArtifactReasonView::ProducingActivityAmbiguous {
            representation_id, ..
        } => ("producing_activity_ambiguous", representation_id),
        ArtifactReasonView::SnapshotAbsent {
            representation_id, ..
        } => ("snapshot_absent", representation_id),
        ArtifactReasonView::FingerprintEvidenceMissing {
            representation_id, ..
        } => ("fingerprint_evidence_missing", representation_id),
        ArtifactReasonView::FingerprintChanged {
            representation_id, ..
        } => ("fingerprint_changed", representation_id),
        ArtifactReasonView::FingerprintRecomputationPending {
            representation_id, ..
        } => ("fingerprint_recomputation_pending", representation_id),
        ArtifactReasonView::UpstreamNotCurrent {
            representation_id, ..
        } => ("upstream_not_current", representation_id),
        ArtifactReasonView::TraversalTruncated {
            representation_id, ..
        } => ("traversal_truncated", representation_id),
        ArtifactReasonView::DependencySnapshotAbsent {
            input_representation_id,
            ..
        } => ("dependency_snapshot_absent", input_representation_id),
        ArtifactReasonView::DependencyKnowledgeIncomplete {
            subject_representation_id,
            ..
        } => ("dependency_knowledge_incomplete", subject_representation_id),
        ArtifactReasonView::DependencyPathChanged {
            input_representation_id,
            ..
        } => ("dependency_path_changed", input_representation_id),
        ArtifactReasonView::DependencyFingerprintChanged {
            representation_id, ..
        } => ("dependency_fingerprint_changed", representation_id),
        ArtifactReasonView::DependencyFingerprintRecomputationPending {
            representation_id, ..
        } => (
            "dependency_fingerprint_recomputation_pending",
            representation_id,
        ),
        ArtifactReasonView::DependencyFingerprintEvidenceMissing {
            representation_id, ..
        } => ("dependency_fingerprint_evidence_missing", representation_id),
    }
}

fn artifact_knowledge_state_name(state: ArtifactKnowledgeState) -> Result<&'static str> {
    match state {
        ArtifactKnowledgeState::Current => Ok("current"),
        ArtifactKnowledgeState::Stale => Ok("stale"),
        ArtifactKnowledgeState::Indeterminate => Ok("indeterminate"),
        ArtifactKnowledgeState::Diverged => Ok("diverged"),
        _ => bail!("unsupported artifact knowledge state"),
    }
}

fn artifact_edge_name(edge: ArtifactEdgeKind) -> Result<&'static str> {
    match edge {
        ArtifactEdgeKind::Input => Ok("input"),
        ArtifactEdgeKind::Output => Ok("output"),
        _ => bail!("unsupported artifact edge kind"),
    }
}

fn artifact_traversal_limit_name(limit: ArtifactTraversalLimitKind) -> Result<&'static str> {
    match limit {
        ArtifactTraversalLimitKind::Depth => Ok("depth"),
        ArtifactTraversalLimitKind::Representations => Ok("representations"),
        _ => bail!("unsupported artifact traversal limit"),
    }
}

fn revisions_latest(args: &ProductionArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let revision = production
        .latest_revision()
        .context("load latest revision")?
        .as_ref()
        .map(revision_view);
    if json {
        print_json(&revision)
    } else if let Some(revision) = revision {
        print_revision(&revision);
        Ok(())
    } else {
        println!("no revisions");
        Ok(())
    }
}

fn revisions_since(args: &RevisionsSinceArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let revisions = production
        .changes_since(args.after, args.limit)
        .context("load revision page")?;
    let views: Vec<_> = revisions.iter().map(revision_view).collect();
    if json {
        print_json(&views)
    } else {
        for revision in &views {
            print_revision(revision);
        }
        Ok(())
    }
}

fn parse_revision_event_type(value: &str) -> std::result::Result<RevisionEventType, String> {
    RevisionEventType::from_str(value).map_err(|error| error.to_string())
}

#[derive(Debug, Serialize)]
struct FilteredRevisionPageView {
    revisions: Vec<RevisionView>,
    through_sequence: u64,
}

#[derive(Debug, Serialize)]
struct RevisionWaitView {
    result: &'static str,
    revisions: Vec<RevisionView>,
}

fn revisions_filtered(args: &RevisionsFilteredArgs, json: bool) -> Result<()> {
    let filter = RevisionEventFilter::new(args.kinds.iter().copied())
        .context("build revision event filter")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .changes_since_filtered(args.after, &filter, args.limit)
        .context("load filtered revision page")?;
    let view = FilteredRevisionPageView {
        revisions: page.revisions().iter().map(revision_view).collect(),
        through_sequence: page.through_sequence(),
    };
    if json {
        print_json(&view)
    } else {
        for revision in &view.revisions {
            print_revision(revision);
        }
        println!("through\t{}", view.through_sequence);
        Ok(())
    }
}

fn revisions_wait(args: &RevisionsWaitArgs, json: bool) -> Result<()> {
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let after = match args.after {
        Some(after) => after,
        None => production
            .latest_revision()
            .context("load latest revision")?
            .map_or(0, |revision| revision.sequence()),
    };
    let mut waiter = production
        .revision_waiter()
        .context("create revision waiter")?;
    let outcome = waiter
        .wait_for_revisions(after, args.limit, Duration::from_millis(args.timeout_ms))
        .context("wait for revisions")?;
    let view = match outcome {
        RevisionWaitOutcome::Revisions(revisions) => RevisionWaitView {
            result: "revisions",
            revisions: revisions.iter().map(revision_view).collect(),
        },
        RevisionWaitOutcome::TimedOut => RevisionWaitView {
            result: "timed_out",
            revisions: Vec::new(),
        },
        RevisionWaitOutcome::Closed => RevisionWaitView {
            result: "closed",
            revisions: Vec::new(),
        },
        RevisionWaitOutcome::Cancelled => RevisionWaitView {
            result: "cancelled",
            revisions: Vec::new(),
        },
        _ => bail!("revision wait outcome is not supported by this CLI"),
    };
    if json {
        print_json(&view)
    } else if view.revisions.is_empty() {
        println!("{}", view.result);
        Ok(())
    } else {
        for revision in &view.revisions {
            print_revision(revision);
        }
        Ok(())
    }
}

fn revisions_changed(args: &RevisionsChangedArgs, json: bool) -> Result<()> {
    let request = query_page_request(&args.page)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let page = production
        .objects_changed_since(args.after, &request)
        .context("query changed objects")?;
    let view = query_page_view(&page, |target| object_ref_view(*target))?;
    print_query_page(&view, json, false, |item| {
        println!("{}\t{}", item.kind, item.id);
    })
}

fn revisions_events(args: &RevisionEventsArgs, json: bool) -> Result<()> {
    let revision_id = RevisionId::from_str(&args.revision_id).context("parse revision ID")?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let events = production
        .events_for_revision(revision_id)
        .context("load revision events")?;
    let views = events
        .iter()
        .map(revision_event_view)
        .collect::<Result<Vec<_>>>()?;
    if json {
        print_json(&views)
    } else {
        for event in &views {
            let payload = serde_json::to_string(&event.event).context("format revision event")?;
            println!("{}\t{payload}", event.position);
        }
        Ok(())
    }
}

fn revision_view(revision: &Revision) -> RevisionView {
    RevisionView {
        id: revision.id().to_string(),
        sequence: revision.sequence(),
        transaction_id: revision.transaction_id().to_string(),
        committed_at_unix_micros: revision.committed_at().as_unix_micros(),
        origin: revision.origin().map(|origin| RevisionOriginView {
            name: origin.name().to_owned(),
            version: origin.version().map(str::to_owned),
            uri: origin.uri().map(str::to_owned),
        }),
        message: revision.message().map(str::to_owned),
    }
}

fn print_revision(revision: &RevisionView) {
    println!(
        "{}\t{}\t{}",
        revision.sequence,
        revision.id,
        revision.message.as_deref().unwrap_or("")
    );
}

#[allow(
    clippy::too_many_lines,
    reason = "the complete semantic event catalog is clearest as one exhaustive mapping"
)]
fn revision_event_view(event: &RevisionEvent) -> Result<RevisionEventView> {
    let kind = match event.kind() {
        RevisionEventKind::AssetImported { asset_id } => RevisionEventKindView::AssetImported {
            asset_id: asset_id.to_string(),
        },
        RevisionEventKind::RepresentationAdded {
            asset_id,
            representation_id,
        } => RevisionEventKindView::RepresentationAdded {
            asset_id: asset_id.to_string(),
            representation_id: representation_id.to_string(),
        },
        RevisionEventKind::ResourceAdded { resource_id } => RevisionEventKindView::ResourceAdded {
            resource_id: resource_id.to_string(),
        },
        RevisionEventKind::RepresentationResourceAdded {
            representation_id,
            resource_id,
            position,
        } => RevisionEventKindView::RepresentationResourceAdded {
            representation_id: representation_id.to_string(),
            resource_id: resource_id.to_string(),
            structural_position: *position,
        },
        RevisionEventKind::LocatorAdded {
            resource_id,
            locator_id,
        } => RevisionEventKindView::LocatorAdded {
            resource_id: resource_id.to_string(),
            locator_id: locator_id.to_string(),
        },
        RevisionEventKind::LocatorRetired {
            resource_id,
            locator_id,
        } => RevisionEventKindView::LocatorRetired {
            resource_id: resource_id.to_string(),
            locator_id: locator_id.to_string(),
        },
        RevisionEventKind::MediaRootAdded { media_root_id } => {
            RevisionEventKindView::MediaRootAdded {
                media_root_id: media_root_id.to_string(),
            }
        }
        RevisionEventKind::MediaRootEnabledChanged {
            media_root_id,
            enabled,
        } => RevisionEventKindView::MediaRootEnabledChanged {
            media_root_id: media_root_id.to_string(),
            enabled: *enabled,
        },
        RevisionEventKind::MediaRootRemoved { media_root_id } => {
            RevisionEventKindView::MediaRootRemoved {
                media_root_id: media_root_id.to_string(),
            }
        }
        RevisionEventKind::ExternalIdentifierAdded { target, identifier } => {
            RevisionEventKindView::ExternalIdentifierAdded {
                target: object_ref_view(*target)?,
                identifier: revision_identifier_view(identifier),
            }
        }
        RevisionEventKind::ExternalIdentifierRemoved { target, identifier } => {
            RevisionEventKindView::ExternalIdentifierRemoved {
                target: object_ref_view(*target)?,
                identifier: revision_identifier_view(identifier),
            }
        }
        RevisionEventKind::MetadataAddedOrReplaced { target, property } => {
            RevisionEventKindView::MetadataAddedOrReplaced {
                target: object_ref_view(*target)?,
                vocabulary: property.vocabulary().as_str().to_owned(),
                property: property.property().as_str().to_owned(),
            }
        }
        RevisionEventKind::MetadataRemoved { target, property } => {
            RevisionEventKindView::MetadataRemoved {
                target: object_ref_view(*target)?,
                vocabulary: property.vocabulary().as_str().to_owned(),
                property: property.property().as_str().to_owned(),
            }
        }
        RevisionEventKind::ActivityCreated { activity_id, kind } => {
            RevisionEventKindView::ActivityCreated {
                activity_id: activity_id.to_string(),
                activity_kind: kind.as_str().to_owned(),
            }
        }
        RevisionEventKind::ActivityInputAdded {
            activity_id,
            representation_id,
            role,
        } => RevisionEventKindView::ActivityInputAdded {
            activity_id: activity_id.to_string(),
            representation_id: representation_id.to_string(),
            role: role.as_ref().map(|role| role.as_str().to_owned()),
        },
        RevisionEventKind::ActivityOutputAdded {
            activity_id,
            representation_id,
            role,
        } => RevisionEventKindView::ActivityOutputAdded {
            activity_id: activity_id.to_string(),
            representation_id: representation_id.to_string(),
            role: role.as_ref().map(|role| role.as_str().to_owned()),
        },
        RevisionEventKind::ResourceFingerprintObserved {
            resource_id,
            algorithm,
            version,
        } => RevisionEventKindView::ResourceFingerprintObserved {
            resource_id: resource_id.to_string(),
            algorithm: algorithm.clone(),
            version: *version,
        },
        RevisionEventKind::RepresentationFingerprintObserved {
            representation_id,
            algorithm,
            version,
        } => RevisionEventKindView::RepresentationFingerprintObserved {
            representation_id: representation_id.to_string(),
            algorithm: algorithm.clone(),
            version: *version,
        },
        RevisionEventKind::DependencySetRecorded { representation_id } => {
            RevisionEventKindView::DependencySetRecorded {
                representation_id: representation_id.to_string(),
            }
        }
        RevisionEventKind::JobRequested { job_id } => RevisionEventKindView::JobRequested {
            job_id: job_id.to_string(),
        },
        RevisionEventKind::JobClaimed { job_id } => RevisionEventKindView::JobClaimed {
            job_id: job_id.to_string(),
        },
        RevisionEventKind::JobClaimRenewed { job_id } => RevisionEventKindView::JobClaimRenewed {
            job_id: job_id.to_string(),
        },
        RevisionEventKind::JobClaimReleased { job_id } => RevisionEventKindView::JobClaimReleased {
            job_id: job_id.to_string(),
        },
        RevisionEventKind::JobSucceeded { job_id } => RevisionEventKindView::JobSucceeded {
            job_id: job_id.to_string(),
        },
        RevisionEventKind::JobFailed { job_id } => RevisionEventKindView::JobFailed {
            job_id: job_id.to_string(),
        },
        RevisionEventKind::JobCancelled { job_id } => RevisionEventKindView::JobCancelled {
            job_id: job_id.to_string(),
        },
        _ => bail!("revision event kind is not supported by this CLI"),
    };
    Ok(RevisionEventView {
        revision_id: event.revision_id().to_string(),
        position: event.position(),
        event: kind,
    })
}

fn revision_identifier_view(identifier: &ExternalIdentifier) -> RevisionIdentifierView {
    RevisionIdentifierView {
        scheme: identifier.scheme().as_str().to_owned(),
        value: identifier.value().to_owned(),
        qualifier: identifier.qualifier().map(str::to_owned),
    }
}

fn print_metadata_assertions(views: &[MetadataAssertionView], json: bool) -> Result<()> {
    if json {
        print_json(&views)
    } else {
        for view in views {
            let value = serde_json::to_string(&view.value).context("format metadata value")?;
            println!(
                "{}\t{}:{}\t{}",
                view.target_id, view.vocabulary, view.property, value
            );
        }
        Ok(())
    }
}

fn media_inventory(args: &MediaInventoryArgs, json: bool) -> Result<()> {
    let root_mappings = prepare_root_mappings(&args.root_mappings)?;
    let production = SqliteProduction::open(&args.production).context("open production")?;
    let report = InventoryScanner::default()
        .scan(&production, &root_mappings, args.cache.as_deref())
        .context("inventory production media")?;
    let view = InventoryView::from(&report);
    if json {
        print_json(&view)
    } else {
        for item in &view.items {
            println!(
                "{}\t{}\t{}",
                item.category,
                item.uri.as_deref().unwrap_or("-"),
                item.detail.as_deref().unwrap_or("-")
            );
        }
        println!(
            "visited={} fingerprints={} cache_hits={}",
            view.stats.entries_visited,
            view.stats.fingerprints_computed,
            view.stats.fingerprint_cache_hits
        );
        Ok(())
    }
}

fn prepare_root_mappings(mappings: &[RootMappingArg]) -> Result<Vec<MediaRootMapping>> {
    mappings
        .iter()
        .map(|mapping| MediaRootMapping::new(&mapping.name, &mapping.directory))
        .collect::<postproject_core::Result<Vec<_>>>()
        .context("prepare root mappings")
}

fn media_resolve(
    args: MediaResolveArgs,
    json: bool,
    base_revision: Option<CliDecisionBase>,
) -> Result<()> {
    let root_mappings = prepare_root_mappings(&args.root_mappings)?;
    let mut production = SqliteProduction::open(&args.production).context("open production")?;
    let asset_id = parse_asset_id(&args.asset_id)?;
    find_asset(&production, asset_id)?;
    let representations = production
        .representations(asset_id)
        .context("load asset representations")?
        .into_iter()
        .map(|representation| load_resolution_input(&production, representation, args.verify))
        .collect::<Result<Vec<_>>>()?;
    let resolver = MediaResolver::new(ResolverOptions {
        verification: if args.verify {
            VerificationMode::Content
        } else {
            VerificationMode::Presence
        },
        ..ResolverOptions::default()
    })
    .context("configure resolver")?;
    let scope = args.search_directories.iter().fold(
        SearchScope::new(production.media_roots()?, root_mappings),
        SearchScope::with_search_directory,
    );
    let inspector = FfprobeInspector::with_executable(&args.ffprobe);
    let resolutions = resolve_representations(&resolver, &scope, &representations, &inspector)?;

    let (confirmed_sequence_naming, receipt) = match args.confirm.as_deref() {
        Some(uri) => {
            let (naming, receipt) = confirm_candidate(
                &mut production,
                &resolutions,
                uri,
                args.confirm_naming.as_ref(),
                base_revision,
            )?;
            (naming, Some(receipt))
        }
        None => (None, None),
    };

    let view = ResolveView {
        asset_id: asset_id.to_string(),
        resolutions: resolutions.iter().map(ResolutionView::from).collect(),
        confirmed_uri: args.confirm,
        confirmed_sequence_naming,
    };
    if json {
        match receipt {
            Some(receipt) => print_json_with_receipt(&view, &receipt),
            None => print_json(&view),
        }
    } else {
        for resolution in &view.resolutions {
            println!(
                "{}: {}",
                resolution.representation_id, resolution.availability
            );
            for resource in &resolution.resources {
                println!("  {}: {}", resource.resource_id, resource.state);
                for candidate in &resource.candidates {
                    match &candidate.sequence_naming {
                        Some(naming) => println!(
                            "    {} {} ({} bp)",
                            candidate.uri, naming.pattern, candidate.confidence_basis_points
                        ),
                        None => println!(
                            "    {} ({} bp)",
                            candidate.uri, candidate.confidence_basis_points
                        ),
                    }
                }
            }
        }
        match (&view.confirmed_uri, &view.confirmed_sequence_naming) {
            (Some(uri), Some(naming)) => println!("confirmed {uri} {}", naming.pattern),
            (Some(uri), None) => println!("confirmed {uri}"),
            _ => {}
        }
        Ok(())
    }
}

/// Records the one candidate at `uri`, with its root and sequence naming.
fn confirm_candidate(
    production: &mut SqliteProduction,
    resolutions: &[RepresentationResolution],
    uri: &str,
    confirm_naming: Option<&SequenceNamingArg>,
    base_revision: Option<CliDecisionBase>,
) -> Result<(Option<SequenceNamingView>, CommitReceipt)> {
    let wanted = confirm_naming.map(|naming| &naming.0);
    let matching: Vec<_> = resolutions
        .iter()
        .flat_map(RepresentationResolution::resources)
        .flat_map(|resolution| {
            resolution
                .candidates()
                .iter()
                .filter(move |candidate| {
                    candidate.uri() == uri
                        && wanted.is_none_or(|wanted| candidate.sequence_naming() == Some(wanted))
                })
                .map(move |candidate| (resolution.resource_id(), candidate))
        })
        .collect();
    if matching.len() != 1 {
        bail!(
            "confirmation URI must identify exactly one candidate from this resolution; \
             choose among image-sequence namings with --confirm-naming"
        );
    }
    let (resource_id, candidate) = matching[0];
    let locator = prepare_confirmed_locator(
        resource_id,
        uri.to_owned(),
        candidate.media_root(),
        candidate.sequence_naming().cloned(),
    )
    .context("prepare confirmed locator")?;
    let mut transaction = begin_cli_transaction(production, base_revision, "confirmation")?;
    set_cli_revision_context(&mut transaction, "Confirm media locator")?;
    transaction
        .add_locator(&locator)
        .context("stage confirmed locator")?;
    let receipt = transaction
        .commit_with_receipt()
        .context("commit confirmed locator")?;
    Ok((
        candidate.sequence_naming().map(SequenceNamingView::from),
        receipt,
    ))
}

/// A representation with the stored knowledge its resolution needs.
struct ResolutionInput {
    representation: Representation,
    technical_metadata: Option<TechnicalMetadata>,
    resources: Vec<(Resource, Vec<Locator>)>,
}

fn load_resolution_input(
    production: &SqliteProduction,
    representation: Representation,
    verify: bool,
) -> Result<ResolutionInput> {
    let technical_metadata = if verify {
        let assertions = production
            .metadata(ObjectRef::Representation(representation.id()))
            .context("load representation technical metadata")?;
        TechnicalMetadata::from_assertions(&assertions)
    } else {
        None
    };
    let resources = production
        .resources(representation.id())
        .context("load representation resources")?
        .into_iter()
        .map(|resource| {
            let locators = production
                .locators(resource.id())
                .context("load resource locators")?;
            Ok((resource, locators))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ResolutionInput {
        representation,
        technical_metadata,
        resources,
    })
}

/// Resolves every resource of every representation in one resolver call, so
/// the search scope is scanned once.
fn resolve_representations(
    resolver: &MediaResolver,
    scope: &SearchScope,
    inputs: &[ResolutionInput],
    inspector: &dyn MediaInspector,
) -> Result<Vec<RepresentationResolution>> {
    let items = inputs
        .iter()
        .flat_map(|input| {
            input.resources.iter().map(move |(resource, locators)| {
                let item = ResolutionItem::new(
                    resource,
                    input.representation.content_structure(),
                    locators,
                );
                match &input.technical_metadata {
                    Some(expected) => item.with_technical_evidence(expected, inspector),
                    None => item,
                }
            })
        })
        .collect::<Vec<_>>();
    let mut results = resolver
        .resolve(&items, scope)
        .context("resolve representation resources")?
        .into_iter();
    inputs
        .iter()
        .map(|input| {
            RepresentationResolution::aggregate(
                input.representation.id(),
                input.representation.content_structure(),
                results.by_ref().take(input.resources.len()).collect(),
            )
            .context("aggregate representation availability")
        })
        .collect()
}

fn parse_asset_id(value: &str) -> Result<AssetId> {
    AssetId::from_str(value).context("parse asset ID")
}

fn parse_representation_id(value: &str) -> Result<RepresentationId> {
    RepresentationId::from_str(value).context("parse representation ID")
}

fn parse_job_id(value: &str) -> Result<JobId> {
    JobId::from_str(value).context("parse job ID")
}

fn parse_job_claim_id(value: &str) -> Result<JobClaimId> {
    JobClaimId::from_str(value).context("parse job claim ID")
}

fn parse_dependency_target(kind: DependencyTargetKind, value: &str) -> Result<DependencyTarget> {
    match kind {
        DependencyTargetKind::Asset => AssetId::from_str(value)
            .map(DependencyTarget::Asset)
            .context("parse asset ID"),
        DependencyTargetKind::Representation => RepresentationId::from_str(value)
            .map(DependencyTarget::Representation)
            .context("parse representation ID"),
    }
}

fn parse_identifier_target(kind: IdentifierTargetKind, value: &str) -> Result<ObjectRef> {
    match kind {
        IdentifierTargetKind::Asset => AssetId::from_str(value)
            .map(ObjectRef::Asset)
            .context("parse asset ID"),
        IdentifierTargetKind::Representation => RepresentationId::from_str(value)
            .map(ObjectRef::Representation)
            .context("parse representation ID"),
        IdentifierTargetKind::Resource => ResourceId::from_str(value)
            .map(ObjectRef::Resource)
            .context("parse resource ID"),
    }
}

fn parse_metadata_target(kind: MetadataTargetKind, value: &str) -> Result<ObjectRef> {
    match kind {
        MetadataTargetKind::Production => ProductionId::from_str(value)
            .map(ObjectRef::Production)
            .context("parse production ID"),
        MetadataTargetKind::Asset => AssetId::from_str(value)
            .map(ObjectRef::Asset)
            .context("parse asset ID"),
        MetadataTargetKind::Representation => RepresentationId::from_str(value)
            .map(ObjectRef::Representation)
            .context("parse representation ID"),
        MetadataTargetKind::Resource => ResourceId::from_str(value)
            .map(ObjectRef::Resource)
            .context("parse resource ID"),
        MetadataTargetKind::Activity => ActivityId::from_str(value)
            .map(ObjectRef::Activity)
            .context("parse activity ID"),
    }
}

fn parse_metadata_property(vocabulary: String, property: String) -> Result<MetadataProperty> {
    Ok(MetadataProperty::new(
        VocabularyId::new(vocabulary).context("validate metadata vocabulary")?,
        PropertyId::new(property).context("validate metadata property")?,
    ))
}

impl MetadataValueInput {
    fn into_value(self) -> Result<MetadataValue> {
        match self {
            Self::String { value } => MetadataValue::string(value).context("validate string"),
            Self::LangString { value, language } => {
                MetadataValue::language_string(value, language).context("validate language string")
            }
            Self::I64 { value } => Ok(MetadataValue::i64(value)),
            Self::U64 { value } => Ok(MetadataValue::u64(value)),
            Self::Decimal { coefficient, scale } => {
                let coefficient = coefficient
                    .parse::<i128>()
                    .context("parse decimal coefficient")?;
                Ok(MetadataValue::decimal(
                    DecimalValue::new(coefficient, scale).context("validate decimal")?,
                ))
            }
            Self::Bool { value } => Ok(MetadataValue::boolean(value)),
            Self::Timestamp { unix_micros } => Ok(MetadataValue::timestamp(
                Timestamp::from_unix_micros(unix_micros),
            )),
            Self::Uri { value } => MetadataValue::uri(value).context("validate URI"),
            Self::Bytes { hex } => {
                MetadataValue::bytes(hex::decode(hex).context("decode hexadecimal metadata bytes")?)
                    .context("validate bytes")
            }
            Self::Rational {
                numerator,
                denominator,
            } => Ok(MetadataValue::rational(
                RationalValue::new(numerator, denominator).context("validate rational")?,
            )),
            Self::List { values } => MetadataValue::list(
                values
                    .into_iter()
                    .map(Self::into_value)
                    .collect::<Result<Vec<_>>>()?,
            )
            .context("validate list"),
            Self::Struct { fields } => MetadataValue::structure(
                fields
                    .into_iter()
                    .map(MetadataFieldInput::into_field)
                    .collect::<Result<Vec<_>>>()?,
            )
            .context("validate structure"),
            Self::Reference { target } => Ok(MetadataValue::reference(parse_metadata_target(
                target.target_kind,
                &target.target_id,
            )?)),
        }
    }
}

impl MetadataFieldInput {
    fn into_field(self) -> Result<MetadataField> {
        Ok(MetadataField::new(
            PropertyId::new(self.name).context("validate metadata field name")?,
            self.value.into_value()?,
        ))
    }
}

fn external_identifier_view(
    target: ObjectRef,
    identifier: &ExternalIdentifier,
) -> ExternalIdentifierView {
    let target = object_ref_view(target).expect("supported CLI target");
    ExternalIdentifierView {
        target_kind: target.kind,
        target_id: target.id,
        scheme: identifier.scheme().as_str().to_owned(),
        value: identifier.value().to_owned(),
        qualifier: identifier.qualifier().map(str::to_owned),
    }
}

fn object_ref_view(target: ObjectRef) -> Result<ObjectRefView> {
    match target {
        ObjectRef::Production(id) => Ok(ObjectRefView {
            kind: "production",
            id: id.to_string(),
        }),
        ObjectRef::Asset(id) => Ok(ObjectRefView {
            kind: "asset",
            id: id.to_string(),
        }),
        ObjectRef::Representation(id) => Ok(ObjectRefView {
            kind: "representation",
            id: id.to_string(),
        }),
        ObjectRef::Resource(id) => Ok(ObjectRefView {
            kind: "resource",
            id: id.to_string(),
        }),
        ObjectRef::Activity(id) => Ok(ObjectRefView {
            kind: "activity",
            id: id.to_string(),
        }),
        ObjectRef::Job(id) => Ok(ObjectRefView {
            kind: "job",
            id: id.to_string(),
        }),
        _ => bail!("object kind is not supported by this CLI"),
    }
}

fn metadata_assertion_view(
    target: ObjectRef,
    assertion: &MetadataAssertion,
) -> Result<MetadataAssertionView> {
    let target = object_ref_view(target)?;
    Ok(MetadataAssertionView {
        target_kind: target.kind,
        target_id: target.id,
        vocabulary: assertion.property().vocabulary().as_str().to_owned(),
        property: assertion.property().property().as_str().to_owned(),
        value: metadata_value_view(assertion.value())?,
    })
}

fn metadata_value_view(value: &MetadataValue) -> Result<MetadataValueView> {
    match value.kind() {
        MetadataValueKind::String => Ok(MetadataValueView::String {
            value: value
                .as_string()
                .context("metadata string has wrong internal type")?
                .to_owned(),
        }),
        MetadataValueKind::LangString => {
            let (text, language) = value
                .as_language_string()
                .context("metadata language string has wrong internal type")?;
            Ok(MetadataValueView::LangString {
                value: text.to_owned(),
                language: language.to_owned(),
            })
        }
        MetadataValueKind::I64 => Ok(MetadataValueView::I64 {
            value: value
                .as_i64()
                .context("metadata integer has wrong internal type")?,
        }),
        MetadataValueKind::U64 => Ok(MetadataValueView::U64 {
            value: value
                .as_u64()
                .context("metadata unsigned integer has wrong internal type")?,
        }),
        MetadataValueKind::Decimal => {
            let decimal = value
                .as_decimal()
                .context("metadata decimal has wrong internal type")?;
            Ok(MetadataValueView::Decimal {
                coefficient: decimal.coefficient().to_string(),
                scale: decimal.scale(),
            })
        }
        MetadataValueKind::Bool => Ok(MetadataValueView::Bool {
            value: value
                .as_bool()
                .context("metadata boolean has wrong internal type")?,
        }),
        MetadataValueKind::Timestamp => Ok(MetadataValueView::Timestamp {
            unix_micros: value
                .as_timestamp()
                .context("metadata timestamp has wrong internal type")?
                .as_unix_micros(),
        }),
        MetadataValueKind::Uri => Ok(MetadataValueView::Uri {
            value: value
                .as_uri()
                .context("metadata URI has wrong internal type")?
                .to_owned(),
        }),
        MetadataValueKind::Bytes => Ok(MetadataValueView::Bytes {
            hex: hex::encode(
                value
                    .as_bytes()
                    .context("metadata bytes have wrong internal type")?,
            ),
        }),
        MetadataValueKind::Rational => {
            let rational = value
                .as_rational()
                .context("metadata rational has wrong internal type")?;
            Ok(MetadataValueView::Rational {
                numerator: rational.numerator(),
                denominator: rational.denominator(),
            })
        }
        MetadataValueKind::List => Ok(MetadataValueView::List {
            values: value
                .as_list()
                .context("metadata list has wrong internal type")?
                .iter()
                .map(metadata_value_view)
                .collect::<Result<_>>()?,
        }),
        MetadataValueKind::Struct => Ok(MetadataValueView::Struct {
            fields: value
                .as_structure()
                .context("metadata structure has wrong internal type")?
                .iter()
                .map(metadata_field_view)
                .collect::<Result<_>>()?,
        }),
        MetadataValueKind::Reference => Ok(MetadataValueView::Reference {
            target: object_ref_view(
                value
                    .as_reference()
                    .context("metadata reference has wrong internal type")?,
            )?,
        }),
        _ => bail!("metadata value kind is not supported by this CLI"),
    }
}

fn metadata_field_view(field: &MetadataField) -> Result<MetadataFieldView> {
    Ok(MetadataFieldView {
        name: field.name().as_str().to_owned(),
        value: metadata_value_view(field.value())?,
    })
}

fn find_asset(production: &SqliteProduction, asset_id: AssetId) -> Result<Asset> {
    production
        .asset(asset_id)
        .with_context(|| format!("load asset {asset_id}"))
}

fn asset_view(production: &SqliteProduction, asset: &Asset) -> Result<AssetView> {
    let mut representations = Vec::new();
    for representation in production
        .representations(asset.id())
        .context("load asset representations")?
    {
        let mut resources = Vec::new();
        for resource in production
            .resources(representation.id())
            .context("load representation resources")?
        {
            let locators = production
                .locators(resource.id())
                .context("load resource locators")?;
            resources.push(resource_view(&resource, &locators));
        }
        representations.push(representation_view(&representation, resources));
    }
    Ok(AssetView {
        id: asset.id().to_string(),
        display_name: asset.display_name().map(str::to_owned),
        import_source: asset.import_source().map(str::to_owned),
        created_at_unix_micros: asset.created_at().as_unix_micros(),
        representations,
    })
}

fn representation_view(
    representation: &Representation,
    resources: Vec<ResourceView>,
) -> RepresentationView {
    RepresentationView {
        id: representation.id().to_string(),
        kind: representation_kind(representation.kind()),
        structure: content_structure_kind(representation.content_structure().kind()),
        resources,
    }
}

fn resource_view(resource: &Resource, locators: &[Locator]) -> ResourceView {
    ResourceView {
        id: resource.id().to_string(),
        fingerprints: resource
            .fingerprints()
            .iter()
            .map(|fingerprint| FingerprintView {
                algorithm: fingerprint.algorithm().to_owned(),
                version: fingerprint.version(),
                value_hex: hex::encode(fingerprint.value()),
            })
            .collect(),
        file_size_bytes: resource
            .file_facts()
            .map(postproject_core::FileFacts::size_bytes),
        locators: locators.iter().map(LocatorView::from).collect(),
    }
}

impl From<&Locator> for LocatorView {
    fn from(locator: &Locator) -> Self {
        Self {
            id: locator.id().to_string(),
            uri: locator.uri().to_owned(),
            availability: locator_availability(locator.availability()),
            last_seen_unix_micros: locator
                .last_seen()
                .map(postproject_core::Timestamp::as_unix_micros),
            sequence_naming: locator.sequence_naming().map(SequenceNamingView::from),
        }
    }
}

impl From<&InventoryReport> for InventoryView {
    fn from(report: &InventoryReport) -> Self {
        let stats = report.stats();
        Self {
            items: report
                .items()
                .iter()
                .map(|item| InventoryItemView {
                    category: inventory_category(item.category()),
                    representation_id: item.representation_id().map(|id| id.to_string()),
                    resource_id: item.resource_id().map(|id| id.to_string()),
                    uri: item.uri().map(str::to_owned),
                    detail: item.detail().map(str::to_owned),
                })
                .collect(),
            stats: InventoryStatsView {
                entries_visited: stats.entries_visited,
                fingerprints_computed: stats.fingerprints_computed,
                fingerprint_cache_hits: stats.fingerprint_cache_hits,
                cache_rebuilt: stats.cache_rebuilt,
            },
        }
    }
}

impl From<&RepresentationResolution> for ResolutionView {
    fn from(resolution: &RepresentationResolution) -> Self {
        Self {
            representation_id: resolution.representation_id().to_string(),
            availability: representation_availability(resolution.availability()),
            resources: resolution
                .resources()
                .iter()
                .map(ResourceResolutionView::from)
                .collect(),
            issues: resolution
                .issues()
                .iter()
                .map(AvailabilityIssueView::from)
                .collect(),
        }
    }
}

impl From<&ResourceResolution> for ResourceResolutionView {
    fn from(resolution: &ResourceResolution) -> Self {
        Self {
            resource_id: resolution.resource_id().to_string(),
            state: resource_resolution_state(resolution.state()),
            candidates: resolution
                .candidates()
                .iter()
                .map(|candidate| CandidateView {
                    uri: candidate.uri().to_owned(),
                    confidence_basis_points: candidate.confidence().basis_points(),
                    media_root: candidate.media_root().map(str::to_owned),
                    sequence_naming: candidate.sequence_naming().map(SequenceNamingView::from),
                    evidence: candidate
                        .evidence()
                        .iter()
                        .map(EvidenceView::from)
                        .collect(),
                })
                .collect(),
            evidence: resolution
                .evidence()
                .iter()
                .map(EvidenceView::from)
                .collect(),
        }
    }
}

impl From<&AvailabilityIssue> for AvailabilityIssueView {
    fn from(issue: &AvailabilityIssue) -> Self {
        Self {
            resource_id: issue.resource_id().to_string(),
            required: issue.is_required(),
            kind: availability_issue_kind(issue.kind()),
            frames: issue.frames().to_vec(),
        }
    }
}

impl From<&ResolutionEvidence> for EvidenceView {
    fn from(evidence: &ResolutionEvidence) -> Self {
        Self {
            kind: evidence_kind(evidence.kind()),
            detail: evidence.detail().map(str::to_owned),
        }
    }
}

fn begin_cli_transaction<'production>(
    production: &'production mut SqliteProduction,
    base_revision: Option<CliDecisionBase>,
    operation: &str,
) -> Result<SqliteTransaction<'production>> {
    match base_revision {
        Some(CliDecisionBase::Revision(base_revision)) => {
            production.begin_transaction_at(base_revision)
        }
        Some(CliDecisionBase::Scoped(base)) => production.begin_edit(base),
        None => production.begin_transaction(),
    }
    .with_context(|| format!("begin {operation} transaction"))
}

fn set_cli_revision_context(
    transaction: &mut impl ProductionStoreTransaction,
    message: &str,
) -> Result<()> {
    let context = cli_revision_context(message)?;
    transaction
        .set_revision_context(context)
        .context("set CLI revision context")
}

fn cli_revision_context(message: &str) -> Result<RevisionContext> {
    let origin = OriginIdentity::new(
        "postproject-cli",
        Some(env!("CARGO_PKG_VERSION").to_owned()),
        None,
    )
    .context("build CLI revision origin")?;
    RevisionContext::new(Some(origin), Some(message.to_owned()))
        .context("build CLI revision context")
}

fn print_conflict_json(error: &anyhow::Error) -> bool {
    let Some(transaction_conflict) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<postproject_core::Error>())
        .and_then(postproject_core::Error::transaction_conflict_detail)
        .map(transaction_conflict_view)
    else {
        return false;
    };
    let view = ErrorView {
        error: ErrorDetailView {
            message: format!("{error:#}"),
            transaction_conflict,
        },
    };
    if serde_json::to_writer_pretty(std::io::stdout().lock(), &view).is_ok() {
        println!();
        true
    } else {
        false
    }
}

fn transaction_conflict_view(conflict: &TransactionConflict) -> TransactionConflictView {
    TransactionConflictView {
        key: conflict_key_view(conflict.key()),
        base_revision_id: conflict.base_revision().map(|id| id.to_string()),
        base_revision_sequence: conflict.base_sequence(),
        superseding_revision_id: conflict.superseding_revision().to_string(),
        superseding_revision_sequence: conflict.superseding_sequence(),
    }
}

fn conflict_key_view(key: &SemanticConflictKey) -> ConflictKeyView {
    match key {
        SemanticConflictKey::LocatorSet(id) => {
            simple_conflict_key("locator_set", "resource", id.to_string())
        }
        SemanticConflictKey::MetadataProperty { target, property } => {
            let (target_kind, target_id) = conflict_object_target(*target);
            ConflictKeyView {
                kind: "metadata_property",
                target_kind,
                target_id,
                namespace_name: Some(property.vocabulary().as_str().to_owned()),
                local_name: Some(property.property().as_str().to_owned()),
                qualifier: None,
                version: None,
            }
        }
        SemanticConflictKey::DependencySet(id) => {
            simple_conflict_key("dependency_set", "representation", id.to_string())
        }
        SemanticConflictKey::MediaRoot(id) => {
            simple_conflict_key("media_root", "media_root", id.to_string())
        }
        SemanticConflictKey::ExternalIdentifier { target, identifier } => {
            let (target_kind, target_id) = conflict_object_target(*target);
            ConflictKeyView {
                kind: "external_identifier",
                target_kind,
                target_id,
                namespace_name: Some(identifier.scheme().as_str().to_owned()),
                local_name: Some(identifier.value().to_owned()),
                qualifier: identifier.qualifier().map(str::to_owned),
                version: None,
            }
        }
        SemanticConflictKey::ResourceFingerprint {
            resource_id,
            algorithm,
            version,
        } => fingerprint_conflict_key(
            "resource_fingerprint",
            "resource",
            resource_id.to_string(),
            algorithm,
            *version,
        ),
        SemanticConflictKey::RepresentationFingerprint {
            representation_id,
            algorithm,
            version,
        } => fingerprint_conflict_key(
            "representation_fingerprint",
            "representation",
            representation_id.to_string(),
            algorithm,
            *version,
        ),
        _ => simple_conflict_key("unknown", "unknown", String::new()),
    }
}

fn conflict_object_target(target: ObjectRef) -> (&'static str, String) {
    match target {
        ObjectRef::Production(id) => ("production", id.to_string()),
        ObjectRef::Asset(id) => ("asset", id.to_string()),
        ObjectRef::Representation(id) => ("representation", id.to_string()),
        ObjectRef::Resource(id) => ("resource", id.to_string()),
        ObjectRef::Activity(id) => ("activity", id.to_string()),
        ObjectRef::Job(id) => ("job", id.to_string()),
        _ => ("unknown", String::new()),
    }
}

fn simple_conflict_key(
    kind: &'static str,
    target_kind: &'static str,
    target_id: String,
) -> ConflictKeyView {
    ConflictKeyView {
        kind,
        target_kind,
        target_id,
        namespace_name: None,
        local_name: None,
        qualifier: None,
        version: None,
    }
}

fn fingerprint_conflict_key(
    kind: &'static str,
    target_kind: &'static str,
    target_id: String,
    algorithm: &str,
    version: u16,
) -> ConflictKeyView {
    ConflictKeyView {
        kind,
        target_kind,
        target_id,
        namespace_name: Some(algorithm.to_owned()),
        local_name: None,
        qualifier: None,
        version: Some(version),
    }
}

fn print_json_with_receipt(value: &impl Serialize, receipt: &CommitReceipt) -> Result<()> {
    let mut output = serde_json::to_value(value).context("encode committed result")?;
    let object = output
        .as_object_mut()
        .context("committed result must be an object")?;
    object.insert(
        "commit_receipt".to_owned(),
        serde_json::json!({
            "production_id": receipt.production_id().to_string(),
            "revision": receipt.revision().map(|revision| serde_json::json!({
                "id": revision.id().to_string(), "sequence": revision.sequence()
            }))
        }),
    );
    print_json(&output)
}

fn print_json(value: &impl Serialize) -> Result<()> {
    serde_json::to_writer_pretty(std::io::stdout().lock(), value).context("write JSON output")?;
    println!();
    Ok(())
}

const fn representation_kind(kind: RepresentationKind) -> &'static str {
    match kind {
        RepresentationKind::Original => "original",
        RepresentationKind::Proxy => "proxy",
        RepresentationKind::Optimized => "optimized",
        RepresentationKind::Derived => "derived",
        _ => "unknown",
    }
}

const fn job_state(state: &JobState) -> &'static str {
    match state {
        JobState::Requested => "requested",
        JobState::Claimed(_) => "claimed",
        JobState::Succeeded(_) => "succeeded",
        JobState::Failed(_) => "failed",
        JobState::Cancelled => "cancelled",
        _ => "unknown",
    }
}

const fn inventory_category(category: InventoryCategory) -> &'static str {
    match category {
        InventoryCategory::KnownOnline => "known_online",
        InventoryCategory::Partial => "partial",
        InventoryCategory::Missing => "missing",
        InventoryCategory::NewCandidate => "new_candidate",
        InventoryCategory::Changed => "changed",
        InventoryCategory::DuplicateCandidate => "duplicate_candidate",
        InventoryCategory::AmbiguousRelinkCandidate => "ambiguous_relink_candidate",
        InventoryCategory::RootUnmapped => "root_unmapped",
        InventoryCategory::RootUnavailable => "root_unavailable",
        _ => "unknown",
    }
}

const fn content_structure_kind(kind: postproject_core::ContentStructureKind) -> &'static str {
    match kind {
        postproject_core::ContentStructureKind::SingleResource => "single_resource",
        postproject_core::ContentStructureKind::ImageSequence => "image_sequence",
        postproject_core::ContentStructureKind::OrderedParts => "ordered_parts",
        postproject_core::ContentStructureKind::Package => "package",
        _ => "unknown",
    }
}

const fn locator_availability(availability: LocatorAvailability) -> &'static str {
    match availability {
        LocatorAvailability::Online => "online",
        LocatorAvailability::Offline => "offline",
        _ => "unknown",
    }
}

const fn resource_resolution_state(state: ResourceResolutionState) -> &'static str {
    match state {
        ResourceResolutionState::OnlineAtKnownLocator => "online_at_known_locator",
        ResourceResolutionState::ResolvedExact => "resolved_exact",
        ResourceResolutionState::ResolvedProbable => "resolved_probable",
        ResourceResolutionState::Offline => "offline",
        ResourceResolutionState::Ambiguous => "ambiguous",
        ResourceResolutionState::Error => "error",
        _ => "unknown",
    }
}

const fn representation_availability(state: RepresentationAvailability) -> &'static str {
    match state {
        RepresentationAvailability::Online => "online",
        RepresentationAvailability::Partial => "partial",
        RepresentationAvailability::Offline => "offline",
        RepresentationAvailability::Ambiguous => "ambiguous",
        RepresentationAvailability::Error => "error",
        _ => "unknown",
    }
}

const fn availability_issue_kind(kind: AvailabilityIssueKind) -> &'static str {
    match kind {
        AvailabilityIssueKind::OfflineResource => "offline_resource",
        AvailabilityIssueKind::AmbiguousResource => "ambiguous_resource",
        AvailabilityIssueKind::ResourceError => "resource_error",
        AvailabilityIssueKind::MissingFrames => "missing_frames",
        _ => "unknown",
    }
}

const fn evidence_kind(kind: EvidenceKind) -> &'static str {
    match kind {
        EvidenceKind::KnownLocatorAvailable => "known_locator_available",
        EvidenceKind::ExactFingerprintMatch => "exact_fingerprint_match",
        EvidenceKind::FullHashMatch => "full_hash_match",
        EvidenceKind::PartialFingerprintMatch => "partial_fingerprint_match",
        EvidenceKind::FileSizeMatch => "file_size_match",
        EvidenceKind::FileNameMatch => "file_name_match",
        EvidenceKind::RelativePathSimilarity => "relative_path_similarity",
        EvidenceKind::MediaRootRelation => "media_root_relation",
        EvidenceKind::MediaRootUnmapped => "media_root_unmapped",
        EvidenceKind::MediaRootUnavailable => "media_root_unavailable",
        EvidenceKind::ConflictingCandidate => "conflicting_candidate",
        EvidenceKind::DiscoveryError => "discovery_error",
        EvidenceKind::FingerprintMismatch => "fingerprint_mismatch",
        EvidenceKind::FingerprintNotVerified => "fingerprint_not_verified",
        EvidenceKind::SearchTruncated => "search_truncated",
        _ => "unknown",
    }
}
