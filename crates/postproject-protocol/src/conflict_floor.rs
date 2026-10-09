//! Conflict migration baseline is independent of the replay retention floor.

use postproject_core::DecisionBase;
use serde_json::json;

use crate::{
    Document, Result,
    fields::{checked, exact, nullable, object, text, unsupported},
};

/// Encodes the original semantic conflict baseline, including genesis.
#[must_use]
pub fn encode_conflict_floor(base: DecisionBase) -> Document {
    Document {
        value: json!({"kind":"conflict.floor", "production":base.production_id().to_string(), "revision":base.revision_id().map(|id| id.to_string()), "sequence":base.sequence().to_string()}),
    }
}

/// Decodes a conflict baseline; the receiver must validate retained history.
///
/// # Errors
/// Rejects unknown fields/kinds and incoherent revision/sequence pairs.
pub fn decode_conflict_floor(document: &Document) -> Result<DecisionBase> {
    let fields = object(
        &document.value,
        &["kind", "production", "revision", "sequence"],
    )?;
    if text(&fields["kind"])? != "conflict.floor" {
        return Err(unsupported());
    }
    checked(DecisionBase::new(
        exact(&fields["production"])?,
        nullable(&fields["revision"], exact)?,
        exact(&fields["sequence"])?,
    ))
}
