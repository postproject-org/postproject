//! Representation identity, separated from streamed structure and evidence.

use postproject_core::{AssetId, Representation, RepresentationId, RepresentationKind};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, object, text, unsupported},
};

/// Scalar ownership and role; structure and fingerprint bodies follow separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationHeader {
    id: RepresentationId,
    asset_id: AssetId,
    kind: RepresentationKind,
}

impl RepresentationHeader {
    /// Copies scalar facts without cloning resources, members or fingerprints.
    #[must_use]
    pub const fn from_representation(representation: &Representation) -> Self {
        Self {
            id: representation.id(),
            asset_id: representation.asset_id(),
            kind: representation.kind(),
        }
    }

    /// Returns the original representation identity.
    #[must_use]
    pub const fn id(self) -> RepresentationId {
        self.id
    }

    /// Returns the owning asset identity.
    #[must_use]
    pub const fn asset_id(self) -> AssetId {
        self.asset_id
    }

    /// Returns the semantic representation role.
    #[must_use]
    pub const fn kind(self) -> RepresentationKind {
        self.kind
    }

    /// Encodes identity and ownership, without embedding aggregate collections.
    ///
    /// # Errors
    /// Rejects future core kinds without a defined wire representation.
    pub fn document(self) -> Result<Document> {
        let role = match self.kind {
            RepresentationKind::Original => "original",
            RepresentationKind::Proxy => "proxy",
            RepresentationKind::Optimized => "optimized",
            RepresentationKind::Derived => "derived",
            _ => return Err(unsupported()),
        };
        Ok(Document {
            value: json!({"kind":"representation.header", "id":self.id.to_string(), "asset_id":self.asset_id.to_string(), "role":role}),
        })
    }

    /// Decodes scalar facts; storage verifies asset existence and original uniqueness.
    ///
    /// # Errors
    /// Rejects unknown fields, unsupported kinds and noncanonical typed identities.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "id", "asset_id", "role"])?;
        if text(&fields["kind"])? != "representation.header" {
            return Err(unsupported());
        }
        let kind = match text(&fields["role"])? {
            "original" => RepresentationKind::Original,
            "proxy" => RepresentationKind::Proxy,
            "optimized" => RepresentationKind::Optimized,
            "derived" => RepresentationKind::Derived,
            _ => return Err(unsupported()),
        };
        Ok(Self {
            id: exact(&fields["id"])?,
            asset_id: exact(&fields["asset_id"])?,
            kind,
        })
    }
}
