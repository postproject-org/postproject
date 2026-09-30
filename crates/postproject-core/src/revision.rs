//! Durable semantic revision values for production-local change feeds.

use std::{collections::BTreeSet, fmt, str::FromStr, time::Duration};

use crate::{
    ActivityId, ActivityKind, ActivityRole, AssetId, Error, ErrorKind, ExternalIdentifier, JobId,
    LocatorId, MediaRootId, MetadataProperty, ObjectRef, RepresentationId, ResourceId, Result,
    RevisionId, Timestamp, ToolIdentity, TransactionId,
};

/// Maximum UTF-8 byte length of a revision message.
pub const MAX_REVISION_MESSAGE_BYTES: usize = 4_096;
/// Maximum revisions returned by one change-feed page.
pub const MAX_REVISION_PAGE_SIZE: u32 = 1_000;
/// Longest caller-supplied timeout accepted by one revision wait.
pub const MAX_REVISION_WAIT: Duration = Duration::from_secs(60);

/// Identity of the integrating application or process that committed a revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginIdentity(ToolIdentity);

impl OriginIdentity {
    /// Creates a bounded origin identity with an optional version and URI.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] under the same conditions as
    /// [`ToolIdentity::new`].
    pub fn new(
        name: impl Into<String>,
        version: Option<String>,
        uri: Option<String>,
    ) -> Result<Self> {
        ToolIdentity::new(name, version, uri).map(Self)
    }

    /// Returns the integrating application or process name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.0.name()
    }

    /// Returns the optional exact application version or build.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.0.version()
    }

    /// Returns the optional canonical application or vendor URI.
    #[must_use]
    pub fn uri(&self) -> Option<&str> {
        self.0.uri()
    }
}

/// One committed production mutation transaction in local sequence order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Revision {
    id: RevisionId,
    sequence: u64,
    transaction_id: TransactionId,
    committed_at: Timestamp,
    origin: Option<OriginIdentity>,
    message: Option<String>,
}

/// Optional origin and message applied to one transaction's revision.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RevisionContext {
    origin: Option<OriginIdentity>,
    message: Option<String>,
}

impl RevisionContext {
    /// Creates validated context for a future committed revision.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for an empty, oversized, or
    /// NUL-containing message.
    pub fn new(origin: Option<OriginIdentity>, message: Option<String>) -> Result<Self> {
        validate_revision_message(message.as_deref())?;
        Ok(Self { origin, message })
    }

    /// Returns the optional integrating application/process identity.
    #[must_use]
    pub const fn origin(&self) -> Option<&OriginIdentity> {
        self.origin.as_ref()
    }

    /// Returns the optional human-facing revision message.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

/// One semantic mutation recorded in a durable revision.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RevisionEventKind {
    /// A logical asset and its import aggregate were created.
    AssetImported {
        /// Imported logical asset.
        asset_id: AssetId,
    },
    /// A representation was attached to an asset.
    RepresentationAdded {
        /// Owning logical asset.
        asset_id: AssetId,
        /// Added representation.
        representation_id: RepresentationId,
    },
    /// A storage resource was created.
    ResourceAdded {
        /// Added resource.
        resource_id: ResourceId,
    },
    /// A resource was attached to a representation's content structure.
    RepresentationResourceAdded {
        /// Owning representation.
        representation_id: RepresentationId,
        /// Attached resource.
        resource_id: ResourceId,
        /// Stable structural position within the representation.
        position: u32,
    },
    /// A resource locator was added or explicitly confirmed.
    LocatorAdded {
        /// Located resource.
        resource_id: ResourceId,
        /// Added locator.
        locator_id: LocatorId,
    },
    /// A superseded resource locator was retired.
    LocatorRetired {
        /// Resource that owned the retired locator.
        resource_id: ResourceId,
        /// Retired locator.
        locator_id: LocatorId,
    },
    /// A resolver media root was added.
    MediaRootAdded {
        /// Added media root.
        media_root_id: MediaRootId,
    },
    /// A resolver media root was enabled or disabled.
    MediaRootEnabledChanged {
        /// Updated media root.
        media_root_id: MediaRootId,
        /// New resolver participation state.
        enabled: bool,
    },
    /// A resolver media root was removed.
    MediaRootRemoved {
        /// Removed media root.
        media_root_id: MediaRootId,
    },
    /// An exact external identifier attachment was added.
    ExternalIdentifierAdded {
        /// Object receiving the identifier.
        target: ObjectRef,
        /// Added external identifier.
        identifier: ExternalIdentifier,
    },
    /// An exact external identifier attachment was removed.
    ExternalIdentifierRemoved {
        /// Object losing the identifier.
        target: ObjectRef,
        /// Removed external identifier.
        identifier: ExternalIdentifier,
    },
    /// One metadata property's values were appended or replaced.
    MetadataAddedOrReplaced {
        /// Object whose metadata changed.
        target: ObjectRef,
        /// Property that consumers should re-query.
        property: MetadataProperty,
    },
    /// One metadata property was removed.
    MetadataRemoved {
        /// Object whose metadata changed.
        target: ObjectRef,
        /// Removed property.
        property: MetadataProperty,
    },
    /// A production activity was created.
    ActivityCreated {
        /// Added activity.
        activity_id: ActivityId,
        /// Extensible activity kind.
        kind: ActivityKind,
    },
    /// A production activity input edge was added.
    ActivityInputAdded {
        /// Owning activity.
        activity_id: ActivityId,
        /// Consumed representation.
        representation_id: RepresentationId,
        /// Optional semantic edge role.
        role: Option<ActivityRole>,
    },
    /// A production activity output edge was added.
    ActivityOutputAdded {
        /// Owning activity.
        activity_id: ActivityId,
        /// Produced representation.
        representation_id: RepresentationId,
        /// Optional semantic edge role.
        role: Option<ActivityRole>,
    },
    /// A resource fingerprint domain received a new current observation.
    ResourceFingerprintObserved {
        /// Re-observed storage resource.
        resource_id: ResourceId,
        /// Fingerprint algorithm identifier.
        algorithm: String,
        /// Fingerprint algorithm format version.
        version: u16,
    },
    /// A representation fingerprint domain received a new current observation.
    RepresentationFingerprintObserved {
        /// Re-observed representation.
        representation_id: RepresentationId,
        /// Fingerprint algorithm identifier.
        algorithm: String,
        /// Fingerprint algorithm format version.
        version: u16,
    },
    /// A representation's complete dependency observation was replaced.
    DependencySetRecorded {
        /// Representation whose authored dependency set changed.
        representation_id: RepresentationId,
    },
    /// A durable work request was created.
    JobRequested {
        /// Requested job.
        job_id: JobId,
    },
    /// A worker claimed a requested or expired job.
    JobClaimed {
        /// Claimed job.
        job_id: JobId,
    },
    /// The current worker extended a job lease.
    JobClaimRenewed {
        /// Renewed job.
        job_id: JobId,
    },
    /// The current worker released a job claim.
    JobClaimReleased {
        /// Released job.
        job_id: JobId,
    },
    /// A job completed with its durable activity and output.
    JobSucceeded {
        /// Completed job.
        job_id: JobId,
    },
    /// A job ended with a diagnostic and no output.
    JobFailed {
        /// Failed job.
        job_id: JobId,
    },
    /// A requested or claimed job was cancelled.
    JobCancelled {
        /// Cancelled job.
        job_id: JobId,
    },
}

/// Payload-free discriminant of a [`RevisionEventKind`].
///
/// Event types select revisions for a filtered change-feed page. Their stable
/// names are the `snake_case` form of the event kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RevisionEventType {
    /// A logical asset and its import aggregate were created.
    AssetImported,
    /// A representation was attached to an asset.
    RepresentationAdded,
    /// A storage resource was created.
    ResourceAdded,
    /// A resource was attached to a representation's content structure.
    RepresentationResourceAdded,
    /// A resource locator was added or explicitly confirmed.
    LocatorAdded,
    /// A superseded resource locator was retired.
    LocatorRetired,
    /// A resolver media root was added.
    MediaRootAdded,
    /// A resolver media root was enabled or disabled.
    MediaRootEnabledChanged,
    /// A resolver media root was removed.
    MediaRootRemoved,
    /// An exact external identifier attachment was added.
    ExternalIdentifierAdded,
    /// An exact external identifier attachment was removed.
    ExternalIdentifierRemoved,
    /// One metadata property's values were appended or replaced.
    MetadataAddedOrReplaced,
    /// One metadata property was removed.
    MetadataRemoved,
    /// A production activity was created.
    ActivityCreated,
    /// A production activity input edge was added.
    ActivityInputAdded,
    /// A production activity output edge was added.
    ActivityOutputAdded,
    /// A resource fingerprint domain received a new current observation.
    ResourceFingerprintObserved,
    /// A representation fingerprint domain received a new current observation.
    RepresentationFingerprintObserved,
    /// A representation's complete dependency observation was replaced.
    DependencySetRecorded,
    /// A durable work request was created.
    JobRequested,
    /// A worker claimed a requested or expired job.
    JobClaimed,
    /// The current worker extended a job lease.
    JobClaimRenewed,
    /// The current worker released a job claim.
    JobClaimReleased,
    /// A job completed with its durable activity and output.
    JobSucceeded,
    /// A job ended with a diagnostic and no output.
    JobFailed,
    /// A requested or claimed job was cancelled.
    JobCancelled,
}

impl RevisionEventType {
    /// Every event type in catalog order.
    pub const ALL: &'static [Self] = &[
        Self::AssetImported,
        Self::RepresentationAdded,
        Self::ResourceAdded,
        Self::RepresentationResourceAdded,
        Self::LocatorAdded,
        Self::LocatorRetired,
        Self::MediaRootAdded,
        Self::MediaRootEnabledChanged,
        Self::MediaRootRemoved,
        Self::ExternalIdentifierAdded,
        Self::ExternalIdentifierRemoved,
        Self::MetadataAddedOrReplaced,
        Self::MetadataRemoved,
        Self::ActivityCreated,
        Self::ActivityInputAdded,
        Self::ActivityOutputAdded,
        Self::ResourceFingerprintObserved,
        Self::RepresentationFingerprintObserved,
        Self::DependencySetRecorded,
        Self::JobRequested,
        Self::JobClaimed,
        Self::JobClaimRenewed,
        Self::JobClaimReleased,
        Self::JobSucceeded,
        Self::JobFailed,
        Self::JobCancelled,
    ];

    /// Returns the stable `snake_case` event type name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AssetImported => "asset_imported",
            Self::RepresentationAdded => "representation_added",
            Self::ResourceAdded => "resource_added",
            Self::RepresentationResourceAdded => "representation_resource_added",
            Self::LocatorAdded => "locator_added",
            Self::LocatorRetired => "locator_retired",
            Self::MediaRootAdded => "media_root_added",
            Self::MediaRootEnabledChanged => "media_root_enabled_changed",
            Self::MediaRootRemoved => "media_root_removed",
            Self::ExternalIdentifierAdded => "external_identifier_added",
            Self::ExternalIdentifierRemoved => "external_identifier_removed",
            Self::MetadataAddedOrReplaced => "metadata_added_or_replaced",
            Self::MetadataRemoved => "metadata_removed",
            Self::ActivityCreated => "activity_created",
            Self::ActivityInputAdded => "activity_input_added",
            Self::ActivityOutputAdded => "activity_output_added",
            Self::ResourceFingerprintObserved => "resource_fingerprint_observed",
            Self::RepresentationFingerprintObserved => "representation_fingerprint_observed",
            Self::DependencySetRecorded => "dependency_set_recorded",
            Self::JobRequested => "job_requested",
            Self::JobClaimed => "job_claimed",
            Self::JobClaimRenewed => "job_claim_renewed",
            Self::JobClaimReleased => "job_claim_released",
            Self::JobSucceeded => "job_succeeded",
            Self::JobFailed => "job_failed",
            Self::JobCancelled => "job_cancelled",
        }
    }

    /// Returns the explicit optimistic-concurrency policy for this event type.
    #[must_use]
    pub const fn concurrency_classification(self) -> crate::ConcurrencyClassification {
        use crate::ConcurrencyClassification::{
            Conflict, ExistingGuard, Merge, OperationDependent,
        };
        match self {
            Self::AssetImported
            | Self::RepresentationAdded
            | Self::ResourceAdded
            | Self::RepresentationResourceAdded
            | Self::ActivityCreated
            | Self::ActivityInputAdded
            | Self::ActivityOutputAdded
            | Self::JobRequested => Merge,
            Self::LocatorAdded
            | Self::LocatorRetired
            | Self::MediaRootEnabledChanged
            | Self::MediaRootRemoved
            | Self::ExternalIdentifierAdded
            | Self::ExternalIdentifierRemoved
            | Self::MetadataRemoved
            | Self::ResourceFingerprintObserved
            | Self::RepresentationFingerprintObserved
            | Self::DependencySetRecorded => Conflict,
            Self::MediaRootAdded
            | Self::JobClaimed
            | Self::JobClaimRenewed
            | Self::JobClaimReleased
            | Self::JobSucceeded
            | Self::JobFailed
            | Self::JobCancelled => ExistingGuard,
            Self::MetadataAddedOrReplaced => OperationDependent,
        }
    }
}

impl fmt::Display for RevisionEventType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RevisionEventType {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|event_type| event_type.as_str() == value)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidArgument,
                    format!("unknown revision event type: {value}"),
                )
            })
    }
}

impl RevisionEventKind {
    /// Returns the payload-free type of this event.
    #[must_use]
    pub const fn event_type(&self) -> RevisionEventType {
        match self {
            Self::AssetImported { .. } => RevisionEventType::AssetImported,
            Self::RepresentationAdded { .. } => RevisionEventType::RepresentationAdded,
            Self::ResourceAdded { .. } => RevisionEventType::ResourceAdded,
            Self::RepresentationResourceAdded { .. } => {
                RevisionEventType::RepresentationResourceAdded
            }
            Self::LocatorAdded { .. } => RevisionEventType::LocatorAdded,
            Self::LocatorRetired { .. } => RevisionEventType::LocatorRetired,
            Self::MediaRootAdded { .. } => RevisionEventType::MediaRootAdded,
            Self::MediaRootEnabledChanged { .. } => RevisionEventType::MediaRootEnabledChanged,
            Self::MediaRootRemoved { .. } => RevisionEventType::MediaRootRemoved,
            Self::ExternalIdentifierAdded { .. } => RevisionEventType::ExternalIdentifierAdded,
            Self::ExternalIdentifierRemoved { .. } => RevisionEventType::ExternalIdentifierRemoved,
            Self::MetadataAddedOrReplaced { .. } => RevisionEventType::MetadataAddedOrReplaced,
            Self::MetadataRemoved { .. } => RevisionEventType::MetadataRemoved,
            Self::ActivityCreated { .. } => RevisionEventType::ActivityCreated,
            Self::ActivityInputAdded { .. } => RevisionEventType::ActivityInputAdded,
            Self::ActivityOutputAdded { .. } => RevisionEventType::ActivityOutputAdded,
            Self::ResourceFingerprintObserved { .. } => {
                RevisionEventType::ResourceFingerprintObserved
            }
            Self::RepresentationFingerprintObserved { .. } => {
                RevisionEventType::RepresentationFingerprintObserved
            }
            Self::DependencySetRecorded { .. } => RevisionEventType::DependencySetRecorded,
            Self::JobRequested { .. } => RevisionEventType::JobRequested,
            Self::JobClaimed { .. } => RevisionEventType::JobClaimed,
            Self::JobClaimRenewed { .. } => RevisionEventType::JobClaimRenewed,
            Self::JobClaimReleased { .. } => RevisionEventType::JobClaimReleased,
            Self::JobSucceeded { .. } => RevisionEventType::JobSucceeded,
            Self::JobFailed { .. } => RevisionEventType::JobFailed,
            Self::JobCancelled { .. } => RevisionEventType::JobCancelled,
        }
    }
}

/// Non-empty set of event types that selects revisions for a filtered page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionEventFilter {
    types: BTreeSet<RevisionEventType>,
}

impl RevisionEventFilter {
    /// Creates a filter matching revisions with at least one event of `types`.
    ///
    /// Duplicate types are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when `types` is empty.
    pub fn new(types: impl IntoIterator<Item = RevisionEventType>) -> Result<Self> {
        let types: BTreeSet<_> = types.into_iter().collect();
        if types.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "revision event filter must name at least one event type",
            ));
        }
        Ok(Self { types })
    }

    /// Returns the selected event types in catalog order.
    #[must_use]
    pub fn types(&self) -> impl ExactSizeIterator<Item = RevisionEventType> + '_ {
        self.types.iter().copied()
    }

    /// Reports whether an event of `event_type` selects its revision.
    #[must_use]
    pub fn matches(&self, event_type: RevisionEventType) -> bool {
        self.types.contains(&event_type)
    }
}

/// One page of revisions that contain at least one event of a filter's types.
///
/// Every matching revision with a sequence greater than the requested sequence
/// and at most [`Self::through_sequence`] is in the page. The through sequence
/// is the cursor for the next filtered page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilteredRevisionPage {
    revisions: Vec<Revision>,
    through_sequence: u64,
}

impl FilteredRevisionPage {
    /// Creates a page from ascending matching revisions and its covered range end.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when revisions are not strictly
    /// ascending or one lies beyond `through_sequence`.
    pub fn new(revisions: Vec<Revision>, through_sequence: u64) -> Result<Self> {
        let ascending = revisions
            .windows(2)
            .all(|pair| pair[0].sequence() < pair[1].sequence());
        if !ascending
            || revisions
                .last()
                .is_some_and(|revision| revision.sequence() > through_sequence)
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "filtered revisions must ascend and end at or before the through sequence",
            ));
        }
        Ok(Self {
            revisions,
            through_sequence,
        })
    }

    /// Returns the matching revisions in ascending sequence order.
    #[must_use]
    pub fn revisions(&self) -> &[Revision] {
        &self.revisions
    }

    /// Consumes the page and returns its matching revisions.
    #[must_use]
    pub fn into_revisions(self) -> Vec<Revision> {
        self.revisions
    }

    /// Returns the last sequence this page accounts for; the next cursor.
    #[must_use]
    pub const fn through_sequence(&self) -> u64 {
        self.through_sequence
    }
}

/// Outcome of one bounded wait for revisions after a sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RevisionWaitOutcome {
    /// A non-empty ascending page of revisions after the requested sequence.
    Revisions(Vec<Revision>),
    /// No revision after the requested sequence appeared within the timeout.
    TimedOut,
    /// The production the waiter belongs to was closed. Terminal.
    Closed,
    /// The waiter was cancelled. Terminal.
    Cancelled,
}

/// Bounded blocking wait for revisions committed after a sequence.
///
/// Implementations observe commits from the same process and from other
/// processes sharing the production. Closed and cancelled outcomes are
/// terminal for the waiter.
pub trait RevisionWaiter {
    /// Waits until at least one revision after `after_sequence` exists, then
    /// returns up to `limit` of them in ascending order.
    ///
    /// A zero `timeout` checks once without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when `limit` is zero or exceeds
    /// [`MAX_REVISION_PAGE_SIZE`], or when `timeout` exceeds
    /// [`MAX_REVISION_WAIT`]; storage errors are returned unchanged.
    fn wait_for_revisions(
        &mut self,
        after_sequence: u64,
        limit: u32,
        timeout: Duration,
    ) -> Result<RevisionWaitOutcome>;
}

/// Validates the page limit and timeout of one revision wait.
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidArgument`] for a zero or excessive limit or an
/// excessive timeout.
pub fn validate_revision_wait(limit: u32, timeout: Duration) -> Result<()> {
    if limit == 0 || limit > MAX_REVISION_PAGE_SIZE {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("revision page limit must be 1-{MAX_REVISION_PAGE_SIZE}"),
        ));
    }
    if timeout > MAX_REVISION_WAIT {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!(
                "revision wait timeout must not exceed {} ms",
                MAX_REVISION_WAIT.as_millis()
            ),
        ));
    }
    Ok(())
}

/// One deterministically ordered semantic event within a revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionEvent {
    revision_id: RevisionId,
    position: u32,
    kind: RevisionEventKind,
}

impl RevisionEvent {
    /// Creates an event at its stable zero-based revision position.
    #[must_use]
    pub const fn new(revision_id: RevisionId, position: u32, kind: RevisionEventKind) -> Self {
        Self {
            revision_id,
            position,
            kind,
        }
    }

    /// Returns the revision that owns the event.
    #[must_use]
    pub const fn revision_id(&self) -> RevisionId {
        self.revision_id
    }

    /// Returns the zero-based stable position within the revision.
    #[must_use]
    pub const fn position(&self) -> u32 {
        self.position
    }

    /// Returns the semantic mutation payload.
    #[must_use]
    pub const fn kind(&self) -> &RevisionEventKind {
        &self.kind
    }
}

impl Revision {
    /// Creates a complete durable revision value.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for sequence zero or an empty,
    /// oversized, or NUL-containing message.
    pub fn new(
        id: RevisionId,
        sequence: u64,
        transaction_id: TransactionId,
        committed_at: Timestamp,
        origin: Option<OriginIdentity>,
        message: Option<String>,
    ) -> Result<Self> {
        if sequence == 0 {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "revision sequence must be greater than zero",
            ));
        }
        validate_revision_message(message.as_deref())?;
        Ok(Self {
            id,
            sequence,
            transaction_id,
            committed_at,
            origin,
            message,
        })
    }

    /// Returns the stable revision identity.
    #[must_use]
    pub const fn id(&self) -> RevisionId {
        self.id
    }

    /// Returns the monotonically increasing production-local sequence.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Returns the transaction that produced this revision.
    #[must_use]
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }

    /// Returns the durable commit timestamp.
    #[must_use]
    pub const fn committed_at(&self) -> Timestamp {
        self.committed_at
    }

    /// Returns the optional integrating application/process identity.
    #[must_use]
    pub const fn origin(&self) -> Option<&OriginIdentity> {
        self.origin.as_ref()
    }

    /// Returns the optional human-facing commit message.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

fn validate_revision_message(message: Option<&str>) -> Result<()> {
    if message.is_some_and(|message| {
        message.is_empty() || message.len() > MAX_REVISION_MESSAGE_BYTES || message.contains('\0')
    }) {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!(
                "revision message must contain 1-{MAX_REVISION_MESSAGE_BYTES} UTF-8 bytes without NUL"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_preserves_transaction_context() {
        let origin = OriginIdentity::new(
            "Editorial host",
            Some("2.4.1".to_owned()),
            Some("https://example.com/editor".to_owned()),
        )
        .expect("valid origin");
        let revision = Revision::new(
            RevisionId::new(),
            7,
            TransactionId::new(),
            Timestamp::from_unix_micros(42),
            Some(origin),
            Some("Import camera original".to_owned()),
        )
        .expect("valid revision");

        assert_eq!(revision.sequence(), 7);
        assert_eq!(revision.committed_at().as_unix_micros(), 42);
        assert_eq!(revision.origin().unwrap().name(), "Editorial host");
        assert_eq!(revision.message(), Some("Import camera original"));
    }

    #[test]
    fn revision_rejects_ambiguous_context() {
        let create = |sequence, message| {
            Revision::new(
                RevisionId::new(),
                sequence,
                TransactionId::new(),
                Timestamp::from_unix_micros(0),
                None,
                message,
            )
        };

        assert!(create(0, None).is_err());
        assert!(create(1, Some(String::new())).is_err());
        assert!(create(1, Some("bad\0message".to_owned())).is_err());
        assert!(create(1, Some("x".repeat(MAX_REVISION_MESSAGE_BYTES + 1))).is_err());
    }

    #[test]
    fn events_identify_semantic_targets_in_stable_order() {
        let revision_id = RevisionId::new();
        let asset_id = AssetId::new();
        let representation_id = RepresentationId::new();
        let events = [
            RevisionEvent::new(
                revision_id,
                0,
                RevisionEventKind::AssetImported { asset_id },
            ),
            RevisionEvent::new(
                revision_id,
                1,
                RevisionEventKind::RepresentationAdded {
                    asset_id,
                    representation_id,
                },
            ),
        ];

        assert_eq!(events[0].revision_id(), revision_id);
        assert_eq!(events[1].position(), 1);
        assert!(matches!(
            events[1].kind(),
            RevisionEventKind::RepresentationAdded {
                representation_id: id,
                ..
            } if *id == representation_id
        ));
    }

    #[test]
    fn event_types_round_trip_their_stable_names() {
        for event_type in RevisionEventType::ALL {
            assert_eq!(
                event_type.as_str().parse::<RevisionEventType>().unwrap(),
                *event_type
            );
        }
        assert_eq!(RevisionEventType::ALL.len(), 26);
        assert!("row_updated".parse::<RevisionEventType>().is_err());
        assert_eq!(
            RevisionEventKind::JobSucceeded {
                job_id: JobId::new()
            }
            .event_type(),
            RevisionEventType::JobSucceeded
        );
    }

    #[test]
    fn every_event_type_has_an_explicit_concurrency_classification() {
        use crate::ConcurrencyClassification::{
            Conflict, ExistingGuard, Merge, OperationDependent,
        };

        let classifications: Vec<_> = RevisionEventType::ALL
            .iter()
            .map(|event| event.concurrency_classification())
            .collect();
        assert_eq!(classifications.len(), 26);
        assert!(classifications.contains(&Merge));
        assert!(classifications.contains(&Conflict));
        assert!(classifications.contains(&ExistingGuard));
        assert!(classifications.contains(&OperationDependent));
    }

    #[test]
    fn filters_and_filtered_pages_reject_ambiguous_values() {
        assert!(RevisionEventFilter::new([]).is_err());
        let filter = RevisionEventFilter::new([
            RevisionEventType::JobFailed,
            RevisionEventType::AssetImported,
            RevisionEventType::JobFailed,
        ])
        .unwrap();
        assert_eq!(
            filter.types().collect::<Vec<_>>(),
            [
                RevisionEventType::AssetImported,
                RevisionEventType::JobFailed
            ]
        );
        assert!(filter.matches(RevisionEventType::JobFailed));
        assert!(!filter.matches(RevisionEventType::JobClaimed));

        let revision = |sequence| {
            Revision::new(
                RevisionId::new(),
                sequence,
                TransactionId::new(),
                Timestamp::from_unix_micros(0),
                None,
                None,
            )
            .unwrap()
        };
        assert!(FilteredRevisionPage::new(vec![revision(2), revision(4)], 9).is_ok());
        assert!(FilteredRevisionPage::new(vec![revision(4), revision(2)], 9).is_err());
        assert!(FilteredRevisionPage::new(vec![revision(4)], 3).is_err());
        assert_eq!(
            FilteredRevisionPage::new(Vec::new(), 7)
                .unwrap()
                .through_sequence(),
            7
        );
    }

    #[test]
    fn revision_waits_are_bounded() {
        assert!(validate_revision_wait(1, Duration::ZERO).is_ok());
        assert!(validate_revision_wait(MAX_REVISION_PAGE_SIZE, MAX_REVISION_WAIT).is_ok());
        assert!(validate_revision_wait(0, Duration::ZERO).is_err());
        assert!(validate_revision_wait(MAX_REVISION_PAGE_SIZE + 1, Duration::ZERO).is_err());
        assert!(validate_revision_wait(1, MAX_REVISION_WAIT + Duration::from_millis(1)).is_err());
    }
}
