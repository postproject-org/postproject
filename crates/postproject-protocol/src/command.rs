//! Ordered semantic intent; storage remains responsible for current-state guards.

use postproject_core::{
    FileFacts, Locator, LocatorId, MediaRoot, MediaRootId, MetadataProperty, MetadataValue,
    ObjectRef, PropertyId, ResourceId, VocabularyId,
};
use serde_json::{Value, json};

use crate::{
    Document, IdentifierAttachment, RecordFeature, Result,
    fields::{
        array, checked, decode_reference, encode_reference, malformed, object, text, unsupported,
    },
    metadata,
};

mod media;

/// A checked domain command offered by the current development codec.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Command {
    /// Adds complete logical root configuration, without a local mapping.
    AddMediaRoot(MediaRoot),
    /// Changes an existing root under its native decision guard.
    SetMediaRootEnabled {
        /// Stable configured-root identity.
        root_id: MediaRootId,
        /// Requested resolver participation.
        enabled: bool,
    },
    /// Removes an existing logical root under its native decision guard.
    RemoveMediaRoot(MediaRootId),
    /// Adds complete prepared locator/naming observations.
    AddLocator(Locator),
    /// Retires an existing access route under its native decision guard.
    RetireLocator(LocatorId),
    /// Adds one exact identifier attachment.
    AddIdentifier(IdentifierAttachment),
    /// Removes one exact attachment under its native decision guard.
    RemoveIdentifier(IdentifierAttachment),
    /// Records measured resource evidence without measuring it on submission.
    RecordResourceFileFacts {
        /// Stable resource identity.
        resource_id: ResourceId,
        /// Exact prepared size/time observations.
        facts: FileFacts,
    },
    /// Appends one value while advancing the property's destructive-edit guard.
    AppendMetadata {
        /// Production-scoped assertion target.
        target: ObjectRef,
        /// Exact vocabulary/property identity.
        property: MetadataProperty,
        /// Validated exact value.
        value: MetadataValue,
    },
    /// Replaces the ordered values, including an explicit empty set.
    ReplaceMetadata {
        /// Production-scoped assertion target.
        target: ObjectRef,
        /// Exact vocabulary/property identity.
        property: MetadataProperty,
        /// Ordered replacement values.
        values: Vec<MetadataValue>,
    },
    /// Removes one property under its current semantic guard.
    RemoveMetadata {
        /// Production-scoped assertion target.
        target: ObjectRef,
        /// Exact vocabulary/property identity.
        property: MetadataProperty,
    },
}

impl Command {
    /// Returns the domain codec required to decode this intent.
    #[must_use]
    pub const fn required_feature(&self) -> RecordFeature {
        match self {
            Self::AppendMetadata { .. }
            | Self::ReplaceMetadata { .. }
            | Self::RemoveMetadata { .. } => RecordFeature::Metadata,
            Self::AddMediaRoot(_)
            | Self::SetMediaRootEnabled { .. }
            | Self::RemoveMediaRoot(_)
            | Self::AddLocator(_)
            | Self::RetireLocator(_)
            | Self::AddIdentifier(_)
            | Self::RemoveIdentifier(_)
            | Self::RecordResourceFileFacts { .. } => RecordFeature::Media,
        }
    }
    /// Encodes checked intent without authorizing a current-state mutation.
    ///
    /// # Errors
    /// Rejects unsupported future domain kinds.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: encode(self)?,
        })
    }

    /// Decodes checked domain values; storage must still verify state and scope.
    ///
    /// # Errors
    /// Rejects unknown operations/fields, malformed values or exceeded limits.
    pub fn from_document(document: &Document) -> Result<Self> {
        decode(&document.value)
    }
}

pub(crate) fn encode_property(property: &MetadataProperty) -> Value {
    json!({"vocabulary":property.vocabulary().as_str(),"property":property.property().as_str()})
}

pub(crate) fn decode_property(value: &Value) -> Result<MetadataProperty> {
    let fields = object(value, &["vocabulary", "property"])?;
    Ok(MetadataProperty::new(
        checked(VocabularyId::new(text(&fields["vocabulary"])?))?,
        checked(PropertyId::new(text(&fields["property"])?))?,
    ))
}

pub(crate) fn encode(command: &Command) -> Result<Value> {
    Ok(match command {
        Command::AppendMetadata {
            target,
            property,
            value,
        } => {
            json!({"kind":"metadata.append","target":encode_reference(*target)?,"property":encode_property(property),"value":metadata::encode(value)?})
        }
        Command::ReplaceMetadata {
            target,
            property,
            values,
        } => {
            json!({"kind":"metadata.replace","target":encode_reference(*target)?,"property":encode_property(property),"values":values.iter().map(metadata::encode).collect::<Result<Vec<_>>>()?})
        }
        Command::RemoveMetadata { target, property } => {
            json!({"kind":"metadata.remove","target":encode_reference(*target)?,"property":encode_property(property)})
        }
        other => media::encode(other)?,
    })
}

pub(crate) fn decode(value: &Value) -> Result<Command> {
    let kind = text(value.get("kind").ok_or_else(malformed)?)?;
    let keys = match kind {
        "metadata.append" => &["kind", "target", "property", "value"][..],
        "metadata.replace" => &["kind", "target", "property", "values"][..],
        "metadata.remove" => &["kind", "target", "property"][..],
        _ => return media::decode(kind, value),
    };
    let fields = object(value, keys)?;
    let target = decode_reference(&fields["target"])?;
    let property = decode_property(&fields["property"])?;
    Ok(match kind {
        "metadata.append" => Command::AppendMetadata {
            target,
            property,
            value: metadata::decode(&fields["value"], 1)?,
        },
        "metadata.replace" => Command::ReplaceMetadata {
            target,
            property,
            values: array(&fields["values"], 1_000_000)?
                .iter()
                .map(|v| metadata::decode(v, 1))
                .collect::<Result<_>>()?,
        },
        "metadata.remove" => Command::RemoveMetadata { target, property },
        _ => return Err(unsupported()),
    })
}
