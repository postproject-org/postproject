//! Configured root facts; caller-supplied directory mappings are not wire facts.

use postproject_core::MediaRoot;
use serde_json::json;

use crate::{
    Document, Result,
    fields::{checked, exact, malformed, nullable, object, text, unsupported},
};

/// Encodes a configured logical root, including its retained migration fallback.
#[must_use]
pub fn encode_root(root: &MediaRoot) -> Document {
    Document {
        value: json!({"kind":"root.fact", "id":root.id().to_string(), "name":root.name(), "label":root.label(), "legacy_uri":root.legacy_uri(), "priority":root.priority().to_string(), "enabled":root.is_enabled()}),
    }
}

/// Decodes root configuration through the existing checked constructor.
///
/// Storage checks root uniqueness and reference semantics. This carries no local
/// resolver mapping and performs no filesystem access.
///
/// # Errors
/// Rejects unknown fields/kinds, invalid names/URIs and nonexact scalar values.
pub fn decode_root(document: &Document) -> Result<MediaRoot> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "id",
            "name",
            "label",
            "legacy_uri",
            "priority",
            "enabled",
        ],
    )?;
    if text(&fields["kind"])? != "root.fact" {
        return Err(unsupported());
    }
    checked(MediaRoot::new(
        exact(&fields["id"])?,
        text(&fields["name"])?,
        nullable(&fields["label"], |value| Ok(text(value)?.to_owned()))?,
        nullable(&fields["legacy_uri"], |value| Ok(text(value)?.to_owned()))?,
        exact(&fields["priority"])?,
        fields["enabled"].as_bool().ok_or_else(malformed)?,
    ))
}
