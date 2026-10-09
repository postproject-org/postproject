//! One ordered current assertion, independent of physical database row IDs.

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef};
use serde_json::json;

use crate::{
    Document, Result,
    command::{decode_property, encode_property},
    fields::{decode_reference, encode_reference, exact, malformed, object, text, unsupported},
    metadata,
};

/// Complete current assertion carried by a portable snapshot section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotAssertion {
    target: ObjectRef,
    property: MetadataProperty,
    position: u64,
    value: MetadataValue,
}

impl SnapshotAssertion {
    /// Creates one checked assertion at its original property position.
    ///
    /// # Errors
    /// Rejects a position outside the supported signed storage range.
    pub fn new(
        target: ObjectRef,
        property: MetadataProperty,
        position: u64,
        value: MetadataValue,
    ) -> Result<Self> {
        if i64::try_from(position).is_err() {
            return Err(malformed());
        }
        Ok(Self {
            target,
            property,
            position,
            value,
        })
    }

    /// Returns the source target identity.
    #[must_use]
    pub const fn target(&self) -> ObjectRef {
        self.target
    }

    /// Returns the exact vocabulary/property identity.
    #[must_use]
    pub const fn property(&self) -> &MetadataProperty {
        &self.property
    }

    /// Returns the ordered property position, never a physical row ID.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.position
    }

    /// Returns the exact typed value.
    #[must_use]
    pub const fn value(&self) -> &MetadataValue {
        &self.value
    }

    /// Encodes one item without gathering the property's repeated values.
    ///
    /// # Errors
    /// Rejects unsupported domain alternatives.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: json!({"kind":"metadata.assertion", "target":encode_reference(self.target)?, "property":encode_property(&self.property), "position":self.position.to_string(), "value":metadata::encode(&self.value)?}),
        })
    }

    /// Decodes checked fields; snapshot import validates target and ordering.
    ///
    /// # Errors
    /// Rejects unknown fields, unsupported kinds and invalid exact values.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &["kind", "target", "property", "position", "value"],
        )?;
        if text(&fields["kind"])? != "metadata.assertion" {
            return Err(unsupported());
        }
        Self::new(
            decode_reference(&fields["target"])?,
            decode_property(&fields["property"])?,
            exact(&fields["position"])?,
            metadata::decode(&fields["value"], 1)?,
        )
    }
}
