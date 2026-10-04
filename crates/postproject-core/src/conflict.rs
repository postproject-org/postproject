//! Semantic optimistic-concurrency values.

use crate::{
    ExternalIdentifier, MediaRootId, MetadataProperty, ObjectRef, RepresentationId, ResourceId,
    RevisionId,
};

/// Stable category of a semantic conflict key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ConflictKeyKind {
    /// The complete current locator set of one resource.
    LocatorSet,
    /// One metadata property on one object.
    MetadataProperty,
    /// The complete dependency observation of one representation.
    DependencySet,
    /// One configured media root.
    MediaRoot,
    /// One exact external-identifier attachment.
    ExternalIdentifier,
    /// One current resource-fingerprint domain.
    ResourceFingerprint,
    /// One current representation-fingerprint domain.
    RepresentationFingerprint,
}

impl ConflictKeyKind {
    /// Returns the stable `snake_case` category name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocatorSet => "locator_set",
            Self::MetadataProperty => "metadata_property",
            Self::DependencySet => "dependency_set",
            Self::MediaRoot => "media_root",
            Self::ExternalIdentifier => "external_identifier",
            Self::ResourceFingerprint => "resource_fingerprint",
            Self::RepresentationFingerprint => "representation_fingerprint",
        }
    }
}

/// One non-mergeable semantic fact touched by a transaction.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum SemanticConflictKey {
    /// The complete current locator set of one resource.
    LocatorSet(ResourceId),
    /// One metadata property on one object.
    MetadataProperty {
        /// Object whose property is replaced or removed.
        target: ObjectRef,
        /// Exact vocabulary and property identity.
        property: MetadataProperty,
    },
    /// The complete dependency observation of one representation.
    DependencySet(RepresentationId),
    /// One configured media root.
    MediaRoot(MediaRootId),
    /// One exact external-identifier attachment on one object.
    ExternalIdentifier {
        /// Object carrying the attachment.
        target: ObjectRef,
        /// Exact external identifier.
        identifier: ExternalIdentifier,
    },
    /// One current resource-fingerprint algorithm and version.
    ResourceFingerprint {
        /// Resource carrying the fingerprint.
        resource_id: ResourceId,
        /// Exact fingerprint algorithm identifier.
        algorithm: String,
        /// Fingerprint algorithm format version.
        version: u16,
    },
    /// One current representation-fingerprint algorithm and version.
    RepresentationFingerprint {
        /// Representation carrying the fingerprint.
        representation_id: RepresentationId,
        /// Exact fingerprint algorithm identifier.
        algorithm: String,
        /// Fingerprint algorithm format version.
        version: u16,
    },
}

impl SemanticConflictKey {
    /// Returns this key's stable category.
    #[must_use]
    pub const fn kind(&self) -> ConflictKeyKind {
        match self {
            Self::LocatorSet(_) => ConflictKeyKind::LocatorSet,
            Self::MetadataProperty { .. } => ConflictKeyKind::MetadataProperty,
            Self::DependencySet(_) => ConflictKeyKind::DependencySet,
            Self::MediaRoot(_) => ConflictKeyKind::MediaRoot,
            Self::ExternalIdentifier { .. } => ConflictKeyKind::ExternalIdentifier,
            Self::ResourceFingerprint { .. } => ConflictKeyKind::ResourceFingerprint,
            Self::RepresentationFingerprint { .. } => ConflictKeyKind::RepresentationFingerprint,
        }
    }
}

/// Machine-readable details for one optimistic transaction conflict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionConflict {
    key: SemanticConflictKey,
    base_revision: Option<RevisionId>,
    base_sequence: u64,
    superseding_revision: RevisionId,
    superseding_sequence: u64,
}

impl TransactionConflict {
    /// Creates conflict details from the compared revisions.
    #[doc(hidden)]
    #[must_use]
    pub const fn new(
        key: SemanticConflictKey,
        base_revision: Option<RevisionId>,
        base_sequence: u64,
        superseding_revision: RevisionId,
        superseding_sequence: u64,
    ) -> Self {
        Self {
            key,
            base_revision,
            base_sequence,
            superseding_revision,
            superseding_sequence,
        }
    }

    /// Returns the semantic fact that changed.
    #[must_use]
    pub const fn key(&self) -> &SemanticConflictKey {
        &self.key
    }

    /// Returns the base revision, or `None` for a decision from an empty journal.
    #[must_use]
    pub const fn base_revision(&self) -> Option<RevisionId> {
        self.base_revision
    }

    /// Returns the monotonic sequence of the supplied base revision.
    #[must_use]
    pub const fn base_sequence(&self) -> u64 {
        self.base_sequence
    }

    /// Returns the revision that changed the semantic fact after the base.
    #[must_use]
    pub const fn superseding_revision(&self) -> RevisionId {
        self.superseding_revision
    }

    /// Returns the superseding revision's monotonic sequence.
    #[must_use]
    pub const fn superseding_sequence(&self) -> u64 {
        self.superseding_sequence
    }
}

/// Concurrency policy assigned to a semantic revision event type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ConcurrencyClassification {
    /// Independent changes may merge from a stale base.
    Merge,
    /// The staging operation records a semantic conflict key.
    Conflict,
    /// A more specific state, token, lease, uniqueness, or existence guard applies.
    ExistingGuard,
    /// The event covers operations with different policies.
    OperationDependent,
}
