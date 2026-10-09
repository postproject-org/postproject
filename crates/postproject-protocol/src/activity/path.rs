use postproject_core::RepresentationId;
mod segment;
pub use segment::ActivityPathSegment;
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, malformed, object, text, unsupported},
};

/// Historical result of capturing one required dependency path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityPathStatus {
    /// The subject's fingerprints were captured, including an empty observation.
    Recorded,
    /// The subject needed dependency extraction.
    NeedsExtraction,
    /// The last floating reference had no selected representation.
    Unresolved,
    /// Capture reached the existing 64-segment depth bound.
    DepthTruncated,
    /// Capture reached the existing visited-representation bound.
    RepresentationsTruncated,
}

impl ActivityPathStatus {
    const fn wire(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::NeedsExtraction => "needs-extraction",
            Self::Unresolved => "unresolved",
            Self::DepthTruncated => "depth-truncated",
            Self::RepresentationsTruncated => "representations-truncated",
        }
    }
}

/// Counts and subject of one immutable activity-input dependency path.
///
/// The enclosing input supplies ownership. Local SQL row identities never enter
/// the stream; segments and fingerprint frames follow this header in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityPathHeader {
    position: u64,
    status: ActivityPathStatus,
    subject: RepresentationId,
    segments: u64,
    fingerprints: u64,
}

impl ActivityPathHeader {
    /// Creates a header using the existing persisted capture limits.
    ///
    /// # Errors
    /// Rejects out-of-range counts and contradictory status/evidence facts.
    pub fn new(
        position: u64,
        status: ActivityPathStatus,
        subject: RepresentationId,
        segments: u64,
        fingerprints: u64,
    ) -> Result<Self> {
        if i64::try_from(position)
            .ok()
            .is_none_or(|position| position == i64::MAX)
            || i64::try_from(fingerprints).is_err()
            || segments > 64
            || (segments == 0 && status != ActivityPathStatus::NeedsExtraction)
            || (status == ActivityPathStatus::DepthTruncated && segments != 64)
            || (status != ActivityPathStatus::Recorded && fingerprints != 0)
        {
            return Err(malformed());
        }
        Ok(Self {
            position,
            status,
            subject,
            segments,
            fingerprints,
        })
    }

    /// Returns the original zero-based position in the input's snapshot.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.position
    }

    /// Returns the historical capture result.
    #[must_use]
    pub const fn status(&self) -> ActivityPathStatus {
        self.status
    }

    /// Returns the representation described by this path's evidence.
    #[must_use]
    pub const fn subject_representation_id(&self) -> RepresentationId {
        self.subject
    }

    /// Returns the number of following ordered path-segment frames.
    #[must_use]
    pub const fn segment_count(&self) -> u64 {
        self.segments
    }

    /// Returns the number of following subject fingerprint frames.
    #[must_use]
    pub const fn fingerprint_count(&self) -> u64 {
        self.fingerprints
    }

    /// Encodes historical evidence without consulting current dependencies.
    #[must_use]
    pub fn document(&self) -> Document {
        Document {
            value: json!({"kind":"activity.dependency-path", "position":self.position.to_string(), "status":self.status.wire(), "subject_representation_id":self.subject.to_string(), "segment_count":self.segments.to_string(), "fingerprint_count":self.fingerprints.to_string()}),
        }
    }

    /// Decodes a bounded path header with exact integer representations.
    ///
    /// # Errors
    /// Rejects unknown fields/statuses and contradictory evidence counts.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "position",
                "status",
                "subject_representation_id",
                "segment_count",
                "fingerprint_count",
            ],
        )?;
        if document.kind()? != "activity.dependency-path" {
            return Err(unsupported());
        }
        let status = match text(&fields["status"])? {
            "recorded" => ActivityPathStatus::Recorded,
            "needs-extraction" => ActivityPathStatus::NeedsExtraction,
            "unresolved" => ActivityPathStatus::Unresolved,
            "depth-truncated" => ActivityPathStatus::DepthTruncated,
            "representations-truncated" => ActivityPathStatus::RepresentationsTruncated,
            _ => return Err(unsupported()),
        };
        Self::new(
            exact(&fields["position"])?,
            status,
            exact(&fields["subject_representation_id"])?,
            exact(&fields["segment_count"])?,
            exact(&fields["fingerprint_count"])?,
        )
    }
}
