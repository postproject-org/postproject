//! Exact external attachments, independent of local assertion row identities.

use postproject_core::{ExternalIdentifier, IdentifierScheme, ObjectRef};
use serde_json::{Value, json};

mod change;
pub use change::IdentifierChange;

use crate::{
    Document, Result,
    fields::{checked, decode_reference, encode_reference, nullable, object, text, unsupported},
};

/// One exact supported external identifier attachment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentifierAttachment {
    target: ObjectRef,
    identifier: ExternalIdentifier,
}

impl IdentifierAttachment {
    /// Binds an identifier to an asset, representation, resource or activity.
    ///
    /// # Errors
    /// Rejects target kinds not supported by the native attachment operations.
    pub fn new(target: ObjectRef, identifier: ExternalIdentifier) -> Result<Self> {
        if !matches!(
            target,
            ObjectRef::Asset(_)
                | ObjectRef::Representation(_)
                | ObjectRef::Resource(_)
                | ObjectRef::Activity(_)
        ) {
            return Err(unsupported());
        }
        Ok(Self { target, identifier })
    }

    /// Returns the typed attachment target.
    #[must_use]
    pub const fn target(&self) -> ObjectRef {
        self.target
    }

    /// Returns the exact scheme, value and optional qualifier.
    #[must_use]
    pub const fn identifier(&self) -> &ExternalIdentifier {
        &self.identifier
    }

    /// Encodes an attachment without SQLite row identity or normalization.
    ///
    /// # Errors
    /// Rejects future target variants lacking a defined representation.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: json!({"kind":"identifier.attachment", "target":encode_reference(self.target)?, "identifier":encode_identifier(&self.identifier)}),
        })
    }

    /// Decodes an exact attachment through the checked domain constructors.
    ///
    /// # Errors
    /// Rejects unsupported targets/kinds, unknown fields and invalid values.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "target", "identifier"])?;
        if text(&fields["kind"])? != "identifier.attachment" {
            return Err(unsupported());
        }
        Self::new(
            decode_reference(&fields["target"])?,
            decode_identifier(&fields["identifier"])?,
        )
    }
}

pub(crate) fn encode_identifier(identifier: &ExternalIdentifier) -> Value {
    json!({"scheme":identifier.scheme().as_str(), "value":identifier.value(), "qualifier":identifier.qualifier()})
}

pub(crate) fn decode_identifier(value: &Value) -> Result<ExternalIdentifier> {
    let fields = object(value, &["scheme", "value", "qualifier"])?;
    checked(ExternalIdentifier::new(
        checked(IdentifierScheme::new(text(&fields["scheme"])?))?,
        text(&fields["value"])?,
        nullable(&fields["qualifier"], |value| Ok(text(value)?.to_owned()))?,
    ))
}
