//! Exact recorded locations and per-copy sequence naming, without resolution.

use postproject_core::{Locator, LocatorAvailability, SequenceNaming, Timestamp};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{checked, exact, nullable, object, text, unsupported},
};

/// Encodes a recorded location without checking the filesystem.
///
/// # Errors
/// Rejects future availability kinds without a defined wire representation.
pub fn encode_locator(locator: &Locator) -> Result<Document> {
    let availability = match locator.availability() {
        LocatorAvailability::Unknown => "unknown",
        LocatorAvailability::Online => "online",
        LocatorAvailability::Offline => "offline",
        _ => return Err(unsupported()),
    };
    let naming = locator.sequence_naming().map(|naming| json!({"prefix":naming.prefix(), "suffix":naming.suffix(), "padding":naming.padding().to_string()}));
    Ok(Document {
        value: json!({"kind":"locator.fact", "id":locator.id().to_string(), "resource_id":locator.resource_id().to_string(), "uri":locator.uri(), "last_seen_micros":locator.last_seen().map(|time| time.as_unix_micros().to_string()), "availability":availability, "media_root":locator.media_root(), "sequence_naming":naming}),
    })
}

/// Decodes checked location facts and naming, retaining original identity/time.
///
/// Storage checks resource/root existence, locator uniqueness and whether naming
/// agrees with the resource's content structure. Decode performs no media I/O.
///
/// # Errors
/// Rejects unknown fields/kinds, invalid URIs/root names and invalid sequence naming.
pub fn decode_locator(document: &Document) -> Result<Locator> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "id",
            "resource_id",
            "uri",
            "last_seen_micros",
            "availability",
            "media_root",
            "sequence_naming",
        ],
    )?;
    if text(&fields["kind"])? != "locator.fact" {
        return Err(unsupported());
    }
    let availability = match text(&fields["availability"])? {
        "unknown" => LocatorAvailability::Unknown,
        "online" => LocatorAvailability::Online,
        "offline" => LocatorAvailability::Offline,
        _ => return Err(unsupported()),
    };
    let last_seen = nullable(&fields["last_seen_micros"], |value| {
        Ok(Timestamp::from_unix_micros(exact(value)?))
    })?;
    let mut locator = checked(Locator::new(
        exact(&fields["id"])?,
        exact(&fields["resource_id"])?,
        text(&fields["uri"])?,
        last_seen,
        availability,
    ))?;
    if let Some(root) = nullable(&fields["media_root"], |value| Ok(text(value)?.to_owned()))? {
        locator = checked(locator.with_media_root(root))?;
    }
    if let Some(naming) = nullable(&fields["sequence_naming"], |value| {
        let fields = object(value, &["prefix", "suffix", "padding"])?;
        checked(SequenceNaming::new(
            text(&fields["prefix"])?,
            text(&fields["suffix"])?,
            exact(&fields["padding"])?,
        ))
    })? {
        locator = locator.with_sequence_naming(naming);
    }
    Ok(locator)
}
