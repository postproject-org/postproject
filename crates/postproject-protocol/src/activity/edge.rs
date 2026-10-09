use crate::{Document, Result, fields::malformed, fingerprint::revision_sequence};
use postproject_core::{ActivityId, ActivityRole, MAX_ACTIVITY_EDGES, RepresentationId};

mod wire;

/// The closed input/output alternatives of a provenance edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityEdgeSide {
    /// An input, which may retain required dependency-path evidence.
    Input,
    /// An output, which retains its own fingerprint evidence.
    Output,
}

/// One ordered edge and exact counts for its following immutable snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityEdgeHeader {
    activity: ActivityId,
    side: ActivityEdgeSide,
    position: u64,
    representation: RepresentationId,
    role: Option<ActivityRole>,
    snapshot_sequence: Option<u64>,
    fingerprints: u64,
    dependency_snapshot: bool,
    paths: u64,
}

impl ActivityEdgeHeader {
    /// Creates an edge with absent historical snapshots.
    ///
    /// # Errors
    /// Rejects positions outside the native activity edge bound.
    pub fn new(
        activity: ActivityId,
        side: ActivityEdgeSide,
        position: u64,
        representation: RepresentationId,
        role: Option<ActivityRole>,
    ) -> Result<Self> {
        if usize::try_from(position)
            .ok()
            .is_none_or(|position| position >= MAX_ACTIVITY_EDGES)
        {
            return Err(malformed());
        }
        Ok(Self {
            activity,
            side,
            position,
            representation,
            role,
            snapshot_sequence: None,
            fingerprints: 0,
            dependency_snapshot: false,
            paths: 0,
        })
    }

    /// Adds a recorded edge snapshot, including explicitly empty evidence.
    ///
    /// # Errors
    /// Rejects invalid revisions or out-of-storage-range counts.
    pub fn with_snapshot(mut self, sequence: u64, fingerprints: u64) -> Result<Self> {
        self.snapshot_sequence = Some(revision_sequence(sequence)?);
        self.fingerprints = count(fingerprints)?;
        Ok(self)
    }

    /// Adds an input's recorded dependency marker and exact path count.
    ///
    /// # Errors
    /// Rejects output edges or out-of-storage-range counts.
    pub fn with_dependency_snapshot(mut self, paths: u64) -> Result<Self> {
        if self.side != ActivityEdgeSide::Input {
            return Err(malformed());
        }
        self.dependency_snapshot = true;
        self.paths = count(paths)?;
        Ok(self)
    }

    /// Returns original activity identity.
    #[must_use]
    pub const fn activity_id(&self) -> ActivityId {
        self.activity
    }
    /// Returns input or output scope.
    #[must_use]
    pub const fn side(&self) -> ActivityEdgeSide {
        self.side
    }
    /// Returns its zero-based position within that side.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.position
    }
    /// Returns its exact representation target.
    #[must_use]
    pub const fn representation_id(&self) -> RepresentationId {
        self.representation
    }
    /// Returns optional open-world role attribution.
    #[must_use]
    pub const fn role(&self) -> Option<&ActivityRole> {
        self.role.as_ref()
    }
    /// Returns absent legacy evidence or the original capture boundary.
    #[must_use]
    pub const fn snapshot_revision_sequence(&self) -> Option<u64> {
        self.snapshot_sequence
    }
    /// Returns the number of following fingerprint facts.
    #[must_use]
    pub const fn fingerprint_count(&self) -> u64 {
        self.fingerprints
    }
    /// Distinguishes an absent dependency marker from a complete empty snapshot.
    #[must_use]
    pub const fn has_dependency_snapshot(&self) -> bool {
        self.dependency_snapshot
    }
    /// Returns the number of following dependency paths.
    #[must_use]
    pub const fn dependency_path_count(&self) -> u64 {
        self.paths
    }
    /// Encodes an edge without collecting fingerprint or dependency bodies.
    #[must_use]
    pub fn document(&self) -> Document {
        wire::encode(self)
    }
    /// Decodes checked references, roles and internally consistent counts.
    ///
    /// # Errors
    /// Rejects unknown fields/sides and invalid position or snapshot facts.
    pub fn from_document(document: &Document) -> Result<Self> {
        wire::decode(document)
    }
}

fn count(value: u64) -> Result<u64> {
    if value > i64::MAX.unsigned_abs() {
        Err(malformed())
    } else {
        Ok(value)
    }
}
