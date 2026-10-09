use postproject_core::{FingerprintSnapshot, ObjectRef};
use serde_json::{Value, json};

use super::{decode_snapshot, encode_snapshot, revision_sequence};
use crate::{
    Document, Result,
    fields::{decode_reference, encode_reference, exact, malformed, object, text, unsupported},
};

/// Whether evidence is current or retained at its original supersession boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FingerprintState {
    /// Current evidence in the owner's algorithm/version domain.
    Current,
    /// One historical value; same-revision intermediate observations are retained.
    Superseded {
        /// Zero-based history order within the owner's algorithm/version domain.
        position: u64,
        /// Original revision at which the value stopped being current.
        revision_sequence: u64,
    },
}

/// A typed owner's current or historical fingerprint fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FingerprintObservation {
    target: ObjectRef,
    snapshot: FingerprintSnapshot,
    state: FingerprintState,
}

impl FingerprintObservation {
    /// Creates evidence for a resource or representation with checked boundaries.
    ///
    /// # Errors
    /// Rejects other target kinds, zero/overflowed revisions, oversized positions
    /// and supersession before observation. Equal revision boundaries are valid.
    pub fn new(
        target: ObjectRef,
        snapshot: FingerprintSnapshot,
        state: FingerprintState,
    ) -> Result<Self> {
        if !matches!(
            target,
            ObjectRef::Resource(_) | ObjectRef::Representation(_)
        ) {
            return Err(unsupported());
        }
        if let Some(sequence) = snapshot.observed_revision_sequence() {
            revision_sequence(sequence)?;
        }
        if let FingerprintState::Superseded {
            position,
            revision_sequence: superseded,
        } = state
        {
            revision_sequence(superseded)?;
            if position > i64::MAX.unsigned_abs()
                || snapshot
                    .observed_revision_sequence()
                    .is_some_and(|observed| observed > superseded)
            {
                return Err(malformed());
            }
        }
        Ok(Self {
            target,
            snapshot,
            state,
        })
    }

    /// Returns the resource or representation owner, preserving its domain.
    #[must_use]
    pub const fn target(&self) -> ObjectRef {
        self.target
    }

    /// Returns the original bytes and observation boundary.
    #[must_use]
    pub const fn snapshot(&self) -> &FingerprintSnapshot {
        &self.snapshot
    }

    /// Returns currentness or the exact historical boundary and order.
    #[must_use]
    pub const fn state(&self) -> FingerprintState {
        self.state
    }

    /// Encodes one independent fact without collecting its history siblings.
    ///
    /// # Errors
    /// Rejects unsupported future target variants or invalid snapshot boundaries.
    pub fn document(&self) -> Result<Document> {
        let state = match self.state {
            FingerprintState::Current => json!({"kind":"current"}),
            FingerprintState::Superseded {
                position,
                revision_sequence,
            } => {
                json!({"kind":"superseded", "position":position.to_string(), "revision_sequence":revision_sequence.to_string()})
            }
        };
        Ok(Document {
            value: json!({"kind":"fingerprint.observation", "target":encode_reference(self.target)?, "fingerprint":encode_snapshot(&self.snapshot)?, "state":state}),
        })
    }

    /// Decodes original evidence without deriving it from current media facts.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds and invalid typed evidence or boundaries.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "target", "fingerprint", "state"])?;
        if text(&fields["kind"])? != "fingerprint.observation" {
            return Err(unsupported());
        }
        Self::new(
            decode_reference(&fields["target"])?,
            decode_snapshot(&fields["fingerprint"])?,
            decode_state(&fields["state"])?,
        )
    }
}

fn decode_state(value: &Value) -> Result<FingerprintState> {
    Ok(match text(value.get("kind").ok_or_else(malformed)?)? {
        "current" => {
            object(value, &["kind"])?;
            FingerprintState::Current
        }
        "superseded" => {
            let fields = object(value, &["kind", "position", "revision_sequence"])?;
            FingerprintState::Superseded {
                position: exact(&fields["position"])?,
                revision_sequence: exact(&fields["revision_sequence"])?,
            }
        }
        _ => return Err(unsupported()),
    })
}
