use postproject_core::{RevisionEventKind, SemanticConflictKey};
use serde_json::json;

use super::IdentifierAttachment;
use crate::{
    Document, Result,
    fields::{object, unsupported},
};

/// An exact authored attachment addition or removal, separate from observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentifierChange {
    /// Addition of one exact scheme/value/qualifier on its original typed owner.
    Added(IdentifierAttachment),
    /// Removal of that exact attachment, without normalization or row identity.
    Removed(IdentifierAttachment),
}

impl IdentifierChange {
    /// Returns the exact attachment changed by this operation.
    #[must_use]
    pub const fn attachment(&self) -> &IdentifierAttachment {
        match self {
            Self::Added(attachment) | Self::Removed(attachment) => attachment,
        }
    }

    /// Encodes an authored change using the checked attachment representation.
    ///
    /// # Errors
    /// Rejects unsupported future target variants.
    pub fn document(&self) -> Result<Document> {
        let kind = match self {
            Self::Added(_) => "identifier.added",
            Self::Removed(_) => "identifier.removed",
        };
        Ok(Document {
            value: json!({"kind":kind, "attachment":self.attachment().document()?.value}),
        })
    }

    /// Decodes complete intent; storage checks existence and exact uniqueness.
    ///
    /// # Errors
    /// Rejects unsupported kinds/targets, unknown fields and malformed values.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "attachment"])?;
        let attachment = IdentifierAttachment::from_document(&Document {
            value: fields["attachment"].clone(),
        })?;
        Ok(match document.kind()? {
            "identifier.added" => Self::Added(attachment),
            "identifier.removed" => Self::Removed(attachment),
            _ => return Err(unsupported()),
        })
    }

    /// Returns the original observation projected by the native mutation.
    #[must_use]
    pub fn observation(&self) -> RevisionEventKind {
        let attachment = self.attachment();
        let target = attachment.target();
        let identifier = attachment.identifier().clone();
        match self {
            Self::Added(_) => RevisionEventKind::ExternalIdentifierAdded { target, identifier },
            Self::Removed(_) => RevisionEventKind::ExternalIdentifierRemoved { target, identifier },
        }
    }

    /// Returns the native semantic guard advanced by the exact attachment.
    #[must_use]
    pub fn conflict_key(&self) -> SemanticConflictKey {
        SemanticConflictKey::ExternalIdentifier {
            target: self.attachment().target(),
            identifier: self.attachment().identifier().clone(),
        }
    }
}
