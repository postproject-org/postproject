//! Separate one authored effect's header from its ordered assertion values.

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef};
use serde_json::json;

use crate::{
    Document, MetadataChange, MetadataEffect, Result,
    command::{decode_property, encode_property},
    fields::{
        decode_reference, encode_reference, exact, malformed, nullable, object, text, unsupported,
    },
    metadata,
};

/// Authored metadata operation, distinct from client intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataOperation {
    /// Append at the original authority-assigned assertion position.
    Appended(u64),
    /// Replace with the following exact ordered values, including an empty set.
    Replaced,
    /// Remove a property that existed in the source.
    Removed,
}

/// Checked header preceding individually encoded metadata assertion values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataEffectStart {
    target: ObjectRef,
    property: MetadataProperty,
    operation: MetadataOperation,
    values: u64,
}

impl MetadataEffectStart {
    /// Decodes the exact operation and declared number of subsequent values.
    ///
    /// # Errors
    /// Rejects contradictory counts, invalid positions or unknown fields/kinds.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "target",
                "property",
                "operation",
                "position",
                "values",
            ],
        )?;
        if text(&fields["kind"])? != "metadata.effect" {
            return Err(unsupported());
        }
        let values: u64 = exact(&fields["values"])?;
        if i64::try_from(values).is_err() {
            return Err(malformed());
        }
        let position: Option<u64> = nullable(&fields["position"], exact)?;
        let operation = match text(&fields["operation"])? {
            "appended" if values == 1 => {
                let position = position.ok_or_else(malformed)?;
                if i64::try_from(position).is_err() {
                    return Err(malformed());
                }
                MetadataOperation::Appended(position)
            }
            "replaced" if position.is_none() => MetadataOperation::Replaced,
            "removed" if position.is_none() && values == 0 => MetadataOperation::Removed,
            "appended" | "replaced" | "removed" => return Err(malformed()),
            _ => return Err(unsupported()),
        };
        Ok(Self {
            target: decode_reference(&fields["target"])?,
            property: decode_property(&fields["property"])?,
            operation,
            values,
        })
    }

    /// Returns the exact source object whose assertions changed.
    #[must_use]
    pub const fn target(&self) -> ObjectRef {
        self.target
    }
    /// Returns the exact vocabulary/property identity.
    #[must_use]
    pub const fn property(&self) -> &MetadataProperty {
        &self.property
    }
    /// Returns the authored operation and any assigned assertion position.
    #[must_use]
    pub const fn operation(&self) -> MetadataOperation {
        self.operation
    }
    /// Returns the number of following ordered value documents.
    #[must_use]
    pub const fn value_count(&self) -> u64 {
        self.values
    }

    /// Decodes one subsequent assertion document using checked constructors.
    ///
    /// The caller must enforce the header's count and original item ordering.
    ///
    /// # Errors
    /// Rejects invalid framing, unsupported values or exceeded domain limits.
    pub fn decode_value(document: &Document) -> Result<MetadataValue> {
        let fields = object(&document.value, &["kind", "value"])?;
        if text(&fields["kind"])? != "metadata.value" {
            return Err(unsupported());
        }
        metadata::decode(&fields["value"], 1)
    }
}

impl MetadataEffect {
    /// Encodes one header followed by individual ordered assertion documents.
    ///
    /// Unlike `document`, this never assembles a whole replacement aggregate.
    /// Stream framing and chunks may split each resulting document into bytes.
    ///
    /// # Errors
    /// Iterator items reject unsupported kinds or unrepresentable value counts.
    pub fn frames(&self) -> impl Iterator<Item = Result<Document>> + '_ {
        let (operation, position, values) = match self.change() {
            MetadataChange::Appended { position, value } => {
                ("appended", Some(position), std::slice::from_ref(value))
            }
            MetadataChange::Replaced(values) => ("replaced", None, values),
            MetadataChange::Removed => ("removed", None, &[][..]),
        };
        let header = (|| {
            let count = i64::try_from(values.len()).map_err(|_| malformed())?;
            Ok(Document {
                value: json!({"kind":"metadata.effect", "target":encode_reference(self.target())?, "property":encode_property(self.property()), "operation":operation, "position":position.map(|position| position.to_string()), "values":count.to_string()}),
            })
        })();
        std::iter::once(header).chain(values.iter().map(|value| {
            Ok(Document {
                value: json!({"kind":"metadata.value", "value":metadata::encode(value)?}),
            })
        }))
    }
}
