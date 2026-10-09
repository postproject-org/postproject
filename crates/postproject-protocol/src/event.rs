//! Original observation events carried beside authored effects.

use postproject_core::{RevisionEvent, RevisionEventKind};
use serde_json::{Value, json};

mod media;

use crate::{
    Document, Result,
    command::{decode_property, encode_property},
    fields::{decode_reference, encode_reference, exact, object, text, unsupported},
};

/// Encodes an original observation without generating a local event.
///
/// # Errors
/// Rejects observation families not yet supported by this codec.
pub fn encode_event(event: &RevisionEvent) -> Result<Document> {
    Ok(Document {
        value: json!({"kind":"observation", "revision":event.revision_id().to_string(), "position":event.position().to_string(), "event":encode_kind(event.kind())?}),
    })
}

fn encode_kind(event: &RevisionEventKind) -> Result<Value> {
    let (kind, target, property) = match event {
        RevisionEventKind::MetadataAddedOrReplaced { target, property } => {
            ("metadata_added_or_replaced", *target, property)
        }
        RevisionEventKind::MetadataRemoved { target, property } => {
            ("metadata_removed", *target, property)
        }
        _ => return media::encode(event),
    };
    Ok(
        json!({"kind":kind, "target":encode_reference(target)?, "property":encode_property(property)}),
    )
}

/// Decodes one original observation, retaining revision and position.
///
/// The replay caller must enforce the owning revision and contiguous ordering.
///
/// # Errors
/// Rejects unsupported events, invalid identities and unknown exact fields.
pub fn decode_event(document: &Document) -> Result<RevisionEvent> {
    let fields = object(&document.value, &["kind", "revision", "position", "event"])?;
    if text(&fields["kind"])? != "observation" {
        return Err(unsupported());
    }
    Ok(RevisionEvent::new(
        exact(&fields["revision"])?,
        exact(&fields["position"])?,
        decode_kind(&fields["event"])?,
    ))
}

fn decode_kind(value: &Value) -> Result<RevisionEventKind> {
    let name = text(value.get("kind").ok_or_else(crate::fields::malformed)?)?;
    if !matches!(name, "metadata_added_or_replaced" | "metadata_removed") {
        return media::decode(value);
    }
    let event = object(value, &["kind", "target", "property"])?;
    let target = decode_reference(&event["target"])?;
    let property = decode_property(&event["property"])?;
    let kind = match text(&event["kind"])? {
        "metadata_added_or_replaced" => {
            RevisionEventKind::MetadataAddedOrReplaced { target, property }
        }
        "metadata_removed" => RevisionEventKind::MetadataRemoved { target, property },
        _ => return Err(unsupported()),
    };
    Ok(kind)
}
