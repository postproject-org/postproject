//! Logical media identity and exact import provenance, without storage layout.

use postproject_core::{Asset, Timestamp};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{exact, nullable, object, text, unsupported},
};

/// Encodes the complete scalar facts of one logical asset.
#[must_use]
pub fn encode_asset(asset: &Asset) -> Document {
    Document {
        value: json!({"kind":"asset.header", "id":asset.id().to_string(), "created_at_micros":asset.created_at().as_unix_micros().to_string(), "display_name":asset.display_name(), "import_source":asset.import_source()}),
    }
}

/// Decodes original asset facts without allocating identity or measuring media.
///
/// # Errors
/// Rejects unknown fields/kinds, noncanonical IDs and invalid exact timestamps.
pub fn decode_asset(document: &Document) -> Result<Asset> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "id",
            "created_at_micros",
            "display_name",
            "import_source",
        ],
    )?;
    if text(&fields["kind"])? != "asset.header" {
        return Err(unsupported());
    }
    Ok(Asset::new(
        exact(&fields["id"])?,
        Timestamp::from_unix_micros(exact(&fields["created_at_micros"])?),
        nullable(&fields["display_name"], |value| Ok(text(value)?.to_owned()))?,
        nullable(
            &fields["import_source"],
            |value| Ok(text(value)?.to_owned()),
        )?,
    ))
}
