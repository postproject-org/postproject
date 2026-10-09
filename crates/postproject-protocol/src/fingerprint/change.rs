use postproject_core::{FingerprintSnapshot, ObjectRef, RevisionEventKind, SemanticConflictKey};

use super::{FingerprintRecomputation, revision_sequence};
use crate::{
    Result,
    fields::{malformed, unsupported},
};

/// An authored observation, followed by individually framed recomputation markers.
/// Previous evidence and archive order describe the original transition, including
/// an unchanged representation value that clears a recorded dirty marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FingerprintChangeStart {
    target: ObjectRef,
    previous: Option<FingerprintSnapshot>,
    current: FingerprintSnapshot,
    archived_position: Option<u64>,
    marker_count: u64,
    cleared_marker: Option<FingerprintRecomputation>,
    dependency_invalidated: bool,
}

impl FingerprintChangeStart {
    /// Creates a complete transition header from original storage observations.
    ///
    /// # Errors
    /// Rejects invalid domains, impossible archive/no-op combinations and marker
    /// facts inconsistent with the target kind. Storage checks original state.
    pub fn new(
        target: ObjectRef,
        previous: Option<FingerprintSnapshot>,
        current: FingerprintSnapshot,
        archived_position: Option<u64>,
        marker_count: u64,
        cleared_marker: Option<FingerprintRecomputation>,
        dependency_invalidated: bool,
    ) -> Result<Self> {
        if let Some(sequence) = current.observed_revision_sequence() {
            revision_sequence(sequence)?;
        }
        if marker_count > i64::MAX.unsigned_abs()
            || archived_position.is_some_and(|position| position > i64::MAX.unsigned_abs())
        {
            return Err(malformed());
        }
        match target {
            ObjectRef::Resource(_) if cleared_marker.is_none() && !dependency_invalidated => {}
            ObjectRef::Representation(id)
                if marker_count == 0
                    && cleared_marker.is_none_or(|marker| marker.representation_id() == id) => {}
            _ => return Err(unsupported()),
        }
        if let Some(previous) = &previous {
            if let Some(sequence) = previous.observed_revision_sequence() {
                revision_sequence(sequence)?;
            }
            if previous.algorithm() != current.algorithm()
                || previous.version() != current.version()
            {
                return Err(malformed());
            }
            if previous.value() == current.value() {
                if previous != &current || archived_position.is_some() || cleared_marker.is_none() {
                    return Err(malformed());
                }
            } else if archived_position.is_none()
                || current.observed_revision_sequence().is_none()
                || previous
                    .observed_revision_sequence()
                    .zip(current.observed_revision_sequence())
                    .is_some_and(|(old, new)| old > new)
            {
                return Err(malformed());
            }
        } else if archived_position.is_some() || current.observed_revision_sequence().is_none() {
            return Err(malformed());
        }
        Ok(Self {
            target,
            previous,
            current,
            archived_position,
            marker_count,
            cleared_marker,
            dependency_invalidated,
        })
    }

    /// Returns the typed owner.
    #[must_use]
    pub const fn target(&self) -> ObjectRef {
        self.target
    }

    /// Returns the previous current value, including its original boundary.
    #[must_use]
    pub const fn previous(&self) -> Option<&FingerprintSnapshot> {
        self.previous.as_ref()
    }

    /// Returns the new current evidence; it can retain an older boundary.
    #[must_use]
    pub const fn current(&self) -> &FingerprintSnapshot {
        &self.current
    }

    /// Returns the original history position when previous bytes were superseded.
    #[must_use]
    pub const fn archived_position(&self) -> Option<u64> {
        self.archived_position
    }

    /// Returns the number of following independent marker frames.
    #[must_use]
    pub const fn marker_count(&self) -> u64 {
        self.marker_count
    }

    /// Returns the exact dirty observation cleared by a representation update.
    #[must_use]
    pub const fn cleared_marker(&self) -> Option<FingerprintRecomputation> {
        self.cleared_marker
    }

    /// Reports whether an existing dependency set changed to require extraction.
    #[must_use]
    pub const fn dependency_invalidated(&self) -> bool {
        self.dependency_invalidated
    }

    /// Returns the existing native observation for this domain.
    #[must_use]
    pub fn observation(&self) -> RevisionEventKind {
        let algorithm = self.current.algorithm().to_owned();
        let version = self.current.version();
        match self.target {
            ObjectRef::Resource(resource_id) => RevisionEventKind::ResourceFingerprintObserved {
                resource_id,
                algorithm,
                version,
            },
            ObjectRef::Representation(representation_id) => {
                RevisionEventKind::RepresentationFingerprintObserved {
                    representation_id,
                    algorithm,
                    version,
                }
            }
            _ => unreachable!("checked fingerprint target"),
        }
    }

    /// Returns the native semantic key advanced by this observation.
    #[must_use]
    pub fn conflict_key(&self) -> SemanticConflictKey {
        let algorithm = self.current.algorithm().to_owned();
        let version = self.current.version();
        match self.target {
            ObjectRef::Resource(resource_id) => SemanticConflictKey::ResourceFingerprint {
                resource_id,
                algorithm,
                version,
            },
            ObjectRef::Representation(representation_id) => {
                SemanticConflictKey::RepresentationFingerprint {
                    representation_id,
                    algorithm,
                    version,
                }
            }
            _ => unreachable!("checked fingerprint target"),
        }
    }
}
