//! Authoritative facts are distinct from client command intent.

mod stream;

pub use stream::{MetadataEffectStart, MetadataOperation};

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef};
use serde_json::{Value, json};

use crate::{
    Document, Result,
    command::{decode_property, encode_property},
    fields::{array, decode_reference, encode_reference, exact, malformed, object, unsupported},
    metadata,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Change {
    Appended(u64, MetadataValue),
    Replaced(Vec<MetadataValue>),
    Removed,
}

/// A complete metadata fact authored by the committing authority.
///
/// Replay validates structural state; constructing a value proves no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataEffect {
    target: ObjectRef,
    property: MetadataProperty,
    change: Change,
}

/// A borrowed authoritative alternative, retaining its exact ordered payload.
#[derive(Clone, Copy, Debug)]
pub enum MetadataChange<'a> {
    /// One assertion at its assigned property position.
    Appended {
        /// The authority-assigned position, independent of SQLite row IDs.
        position: u64,
        /// The exact assertion value.
        value: &'a MetadataValue,
    },
    /// The complete ordered replacement.
    Replaced(&'a [MetadataValue]),
    /// The property was removed.
    Removed,
}

impl MetadataEffect {
    /// Records a validated assertion and its authority-assigned storage position.
    ///
    /// # Errors
    /// Rejects positions outside the current signed storage range.
    pub fn appended(
        target: ObjectRef,
        property: MetadataProperty,
        position: u64,
        value: MetadataValue,
    ) -> Result<Self> {
        if position > i64::MAX as u64 {
            return Err(malformed());
        }
        Ok(Self {
            target,
            property,
            change: Change::Appended(position, value),
        })
    }

    /// Records a complete ordered replacement, including an empty set.
    #[must_use]
    pub const fn replaced(
        target: ObjectRef,
        property: MetadataProperty,
        values: Vec<MetadataValue>,
    ) -> Self {
        Self {
            target,
            property,
            change: Change::Replaced(values),
        }
    }

    /// Records an actual removal; absence/no-change is not a new effect.
    #[must_use]
    pub const fn removed(target: ObjectRef, property: MetadataProperty) -> Self {
        Self {
            target,
            property,
            change: Change::Removed,
        }
    }

    /// Returns the original production-scoped target.
    #[must_use]
    pub const fn target(&self) -> ObjectRef {
        self.target
    }
    /// Borrows the exact vocabulary/property identity.
    #[must_use]
    pub const fn property(&self) -> &MetadataProperty {
        &self.property
    }
    /// Borrows the complete authoritative alternative.
    #[must_use]
    pub fn change(&self) -> MetadataChange<'_> {
        match &self.change {
            Change::Appended(position, value) => MetadataChange::Appended {
                position: *position,
                value,
            },
            Change::Replaced(values) => MetadataChange::Replaced(values),
            Change::Removed => MetadataChange::Removed,
        }
    }

    /// Encodes a fact without substituting intent or allocating a revision.
    ///
    /// # Errors
    /// Rejects future unsupported nested value/reference kinds.
    pub fn document(&self) -> Result<Document> {
        let target = encode_reference(self.target)?;
        let property = encode_property(&self.property);
        let value = match &self.change {
            Change::Appended(position, value) => {
                json!({"kind":"metadata.appended","target":target,"property":property,"position":position.to_string(),"value":metadata::encode(value)?})
            }
            Change::Replaced(values) => {
                json!({"kind":"metadata.replaced","target":target,"property":property,"values":values.iter().map(metadata::encode).collect::<Result<Vec<_>>>()?})
            }
            Change::Removed => {
                json!({"kind":"metadata.removed","target":target,"property":property})
            }
        };
        Ok(Document { value })
    }

    /// Decodes complete facts; a replay store must still validate source/state.
    ///
    /// # Errors
    /// Rejects invalid exact positions, unsupported facts or malformed payloads.
    pub fn from_document(document: &Document) -> Result<Self> {
        let value = &document.value;
        let kind = value
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(malformed)?;
        let keys = match kind {
            "metadata.appended" => &["kind", "target", "property", "position", "value"][..],
            "metadata.replaced" => &["kind", "target", "property", "values"][..],
            "metadata.removed" => &["kind", "target", "property"][..],
            _ => return Err(unsupported()),
        };
        let fields = object(value, keys)?;
        let target = decode_reference(&fields["target"])?;
        let property = decode_property(&fields["property"])?;
        match kind {
            "metadata.appended" => Self::appended(
                target,
                property,
                exact(&fields["position"])?,
                metadata::decode(&fields["value"], 1)?,
            ),
            "metadata.replaced" => Ok(Self::replaced(
                target,
                property,
                array(&fields["values"], 1_000_000)?
                    .iter()
                    .map(|v| metadata::decode(v, 1))
                    .collect::<Result<_>>()?,
            )),
            "metadata.removed" => Ok(Self::removed(target, property)),
            _ => Err(unsupported()),
        }
    }
}
