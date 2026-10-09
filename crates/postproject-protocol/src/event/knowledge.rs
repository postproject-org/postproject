use postproject_core::{ActivityKind, ActivityRole, RevisionEventKind};
use serde_json::{Value, json};

use crate::{
    IdentifierAttachment, Result,
    fields::{
        checked, decode_reference, encode_reference, exact, malformed, nullable, object, text,
        unsupported,
    },
    identifier::{decode_identifier, encode_identifier},
};

pub(super) fn encode(event: &RevisionEventKind) -> Result<Value> {
    Ok(match event {
        RevisionEventKind::ExternalIdentifierAdded { target, identifier }
        | RevisionEventKind::ExternalIdentifierRemoved { target, identifier } => {
            IdentifierAttachment::new(*target, identifier.clone())?;
            json!({"kind":event.event_type().as_str(), "target":encode_reference(*target)?, "identifier":encode_identifier(identifier)})
        }
        RevisionEventKind::ActivityCreated { activity_id, kind } => {
            json!({"kind":"activity_created", "activity_id":activity_id.to_string(), "activity_kind":kind.as_str()})
        }
        RevisionEventKind::ActivityInputAdded {
            activity_id,
            representation_id,
            role,
        }
        | RevisionEventKind::ActivityOutputAdded {
            activity_id,
            representation_id,
            role,
        } => {
            json!({"kind":event.event_type().as_str(), "activity_id":activity_id.to_string(), "representation_id":representation_id.to_string(), "role":role.as_ref().map(ActivityRole::as_str)})
        }
        RevisionEventKind::DependencySetRecorded { representation_id } => {
            json!({"kind":"dependency_set_recorded", "representation_id":representation_id.to_string()})
        }
        _ => return Err(unsupported()),
    })
}

pub(super) fn decode(value: &Value) -> Result<RevisionEventKind> {
    Ok(match text(value.get("kind").ok_or_else(malformed)?)? {
        "external_identifier_added" | "external_identifier_removed" => {
            let f = object(value, &["kind", "target", "identifier"])?;
            let attachment = IdentifierAttachment::new(
                decode_reference(&f["target"])?,
                decode_identifier(&f["identifier"])?,
            )?;
            let target = attachment.target();
            let identifier = attachment.identifier().clone();
            if text(&f["kind"])? == "external_identifier_added" {
                RevisionEventKind::ExternalIdentifierAdded { target, identifier }
            } else {
                RevisionEventKind::ExternalIdentifierRemoved { target, identifier }
            }
        }
        "activity_created" => {
            let f = object(value, &["kind", "activity_id", "activity_kind"])?;
            RevisionEventKind::ActivityCreated {
                activity_id: exact(&f["activity_id"])?,
                kind: checked(ActivityKind::new(text(&f["activity_kind"])?))?,
            }
        }
        "activity_input_added" | "activity_output_added" => {
            let f = object(value, &["kind", "activity_id", "representation_id", "role"])?;
            let activity_id = exact(&f["activity_id"])?;
            let representation_id = exact(&f["representation_id"])?;
            let role = nullable(&f["role"], |value| checked(ActivityRole::new(text(value)?)))?;
            if text(&f["kind"])? == "activity_input_added" {
                RevisionEventKind::ActivityInputAdded {
                    activity_id,
                    representation_id,
                    role,
                }
            } else {
                RevisionEventKind::ActivityOutputAdded {
                    activity_id,
                    representation_id,
                    role,
                }
            }
        }
        "dependency_set_recorded" => {
            let f = object(value, &["kind", "representation_id"])?;
            RevisionEventKind::DependencySetRecorded {
                representation_id: exact(&f["representation_id"])?,
            }
        }
        _ => return Err(unsupported()),
    })
}
