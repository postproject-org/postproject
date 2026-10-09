//! Portable production header excludes the receiver's database schema version.

use postproject_core::{Production, ProductionId, Timestamp};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, nullable, object, text, unsupported},
};

/// Source identity and header facts, independent of SQLite bookkeeping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionHeader {
    id: ProductionId,
    created_at: Timestamp,
    display_name: Option<String>,
}

impl ProductionHeader {
    /// Copies immutable source facts without its local schema version.
    #[must_use]
    pub fn from_production(production: &Production) -> Self {
        Self {
            id: production.id(),
            created_at: production.created_at(),
            display_name: production.display_name().map(str::to_owned),
        }
    }

    /// Returns the preserved source identity.
    #[must_use]
    pub const fn id(&self) -> ProductionId {
        self.id
    }

    /// Returns the source's original creation time.
    #[must_use]
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns the exact optional name, including an explicit empty name.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Encodes source facts as one canonicalizable body document.
    #[must_use]
    pub fn document(&self) -> Document {
        Document {
            value: json!({"kind":"production.header", "id":self.id.to_string(), "created_at":self.created_at.as_unix_micros().to_string(), "display_name":self.display_name}),
        }
    }

    /// Decodes exact source facts without accepting local schema/role fields.
    ///
    /// # Errors
    /// Rejects unknown fields, unsupported kind and invalid identities/times.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &["kind", "id", "created_at", "display_name"],
        )?;
        if text(&fields["kind"])? != "production.header" {
            return Err(unsupported());
        }
        Ok(Self {
            id: exact(&fields["id"])?,
            created_at: Timestamp::from_unix_micros(exact(&fields["created_at"])?),
            display_name: nullable(&fields["display_name"], |value| Ok(text(value)?.to_owned()))?,
        })
    }
}
