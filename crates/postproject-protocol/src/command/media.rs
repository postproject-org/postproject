//! Prepared scalar intent remains separate from authored historical effects.

use serde_json::{Value, json};

use super::Command;
use crate::{
    Document, IdentifierAttachment, ResourceHeader, Result, decode_locator, decode_root,
    encode_locator, encode_root,
    fields::{exact, malformed, object, unsupported},
};

pub(super) fn encode(command: &Command) -> Result<Value> {
    Ok(match command {
        Command::AddMediaRoot(root) => json!({"kind":"root.add", "root":encode_root(root).value}),
        Command::SetMediaRootEnabled { root_id, enabled } => {
            json!({"kind":"root.set-enabled", "id":root_id.to_string(), "enabled":enabled})
        }
        Command::RemoveMediaRoot(id) => json!({"kind":"root.remove", "id":id.to_string()}),
        Command::AddLocator(locator) => {
            json!({"kind":"locator.add", "locator":encode_locator(locator)?.value})
        }
        Command::RetireLocator(id) => json!({"kind":"locator.retire", "id":id.to_string()}),
        Command::AddIdentifier(attachment) => {
            json!({"kind":"identifier.add", "attachment":attachment.document()?.value})
        }
        Command::RemoveIdentifier(attachment) => {
            json!({"kind":"identifier.remove", "attachment":attachment.document()?.value})
        }
        Command::RecordResourceFileFacts { resource_id, facts } => {
            json!({"kind":"resource.observe-file-facts",
            "resource":ResourceHeader::from_resource(&postproject_core::Resource::new(*resource_id, Vec::new(), Some(*facts))).document().value})
        }
        _ => return Err(unsupported()),
    })
}

pub(super) fn decode(kind: &str, value: &Value) -> Result<Command> {
    Ok(match kind {
        "root.add" => {
            let fields = object(value, &["kind", "root"])?;
            Command::AddMediaRoot(decode_root(&Document {
                value: fields["root"].clone(),
            })?)
        }
        "root.set-enabled" => {
            let fields = object(value, &["kind", "id", "enabled"])?;
            Command::SetMediaRootEnabled {
                root_id: exact(&fields["id"])?,
                enabled: fields["enabled"].as_bool().ok_or_else(malformed)?,
            }
        }
        "root.remove" => {
            let fields = object(value, &["kind", "id"])?;
            Command::RemoveMediaRoot(exact(&fields["id"])?)
        }
        "locator.add" => {
            let fields = object(value, &["kind", "locator"])?;
            Command::AddLocator(decode_locator(&Document {
                value: fields["locator"].clone(),
            })?)
        }
        "locator.retire" => {
            let fields = object(value, &["kind", "id"])?;
            Command::RetireLocator(exact(&fields["id"])?)
        }
        "identifier.add" | "identifier.remove" => {
            let fields = object(value, &["kind", "attachment"])?;
            let attachment = IdentifierAttachment::from_document(&Document {
                value: fields["attachment"].clone(),
            })?;
            if kind == "identifier.add" {
                Command::AddIdentifier(attachment)
            } else {
                Command::RemoveIdentifier(attachment)
            }
        }
        "resource.observe-file-facts" => {
            let fields = object(value, &["kind", "resource"])?;
            let resource = ResourceHeader::from_document(&Document {
                value: fields["resource"].clone(),
            })?;
            Command::RecordResourceFileFacts {
                resource_id: resource.id(),
                facts: resource.file_facts().ok_or_else(malformed)?,
            }
        }
        _ => return Err(unsupported()),
    })
}
