//! Media observations describe the original change; they are not replay effects.

use postproject_core::RevisionEventKind;
use serde_json::{Value, json};

use crate::{
    Result,
    fields::{exact, malformed, object, text, unsupported},
};

pub(super) fn encode(event: &RevisionEventKind) -> Result<Value> {
    Ok(match event {
        RevisionEventKind::AssetImported { asset_id } => {
            json!({"kind":"asset_imported", "asset_id":asset_id.to_string()})
        }
        RevisionEventKind::RepresentationAdded {
            asset_id,
            representation_id,
        } => {
            json!({"kind":"representation_added", "asset_id":asset_id.to_string(), "representation_id":representation_id.to_string()})
        }
        RevisionEventKind::ResourceAdded { resource_id } => {
            json!({"kind":"resource_added", "resource_id":resource_id.to_string()})
        }
        RevisionEventKind::RepresentationResourceAdded {
            representation_id,
            resource_id,
            position,
        } => {
            json!({"kind":"representation_resource_added", "representation_id":representation_id.to_string(), "resource_id":resource_id.to_string(), "position":position.to_string()})
        }
        RevisionEventKind::LocatorAdded {
            resource_id,
            locator_id,
        } => {
            json!({"kind":"locator_added", "resource_id":resource_id.to_string(), "locator_id":locator_id.to_string()})
        }
        RevisionEventKind::LocatorRetired {
            resource_id,
            locator_id,
        } => {
            json!({"kind":"locator_retired", "resource_id":resource_id.to_string(), "locator_id":locator_id.to_string()})
        }
        RevisionEventKind::MediaRootAdded { media_root_id } => {
            json!({"kind":"media_root_added", "media_root_id":media_root_id.to_string()})
        }
        RevisionEventKind::MediaRootEnabledChanged {
            media_root_id,
            enabled,
        } => {
            json!({"kind":"media_root_enabled_changed", "media_root_id":media_root_id.to_string(), "enabled":enabled})
        }
        RevisionEventKind::MediaRootRemoved { media_root_id } => {
            json!({"kind":"media_root_removed", "media_root_id":media_root_id.to_string()})
        }
        RevisionEventKind::ResourceFileFactsObserved { resource_id } => {
            json!({"kind":"resource_file_facts_observed", "resource_id":resource_id.to_string()})
        }
        _ => return Err(unsupported()),
    })
}

pub(super) fn decode(value: &Value) -> Result<RevisionEventKind> {
    Ok(match text(value.get("kind").ok_or_else(malformed)?)? {
        "asset_imported" => {
            let f = object(value, &["kind", "asset_id"])?;
            RevisionEventKind::AssetImported {
                asset_id: exact(&f["asset_id"])?,
            }
        }
        "representation_added" => {
            let f = object(value, &["kind", "asset_id", "representation_id"])?;
            RevisionEventKind::RepresentationAdded {
                asset_id: exact(&f["asset_id"])?,
                representation_id: exact(&f["representation_id"])?,
            }
        }
        "resource_added" => {
            let f = object(value, &["kind", "resource_id"])?;
            RevisionEventKind::ResourceAdded {
                resource_id: exact(&f["resource_id"])?,
            }
        }
        "representation_resource_added" => {
            let f = object(
                value,
                &["kind", "representation_id", "resource_id", "position"],
            )?;
            RevisionEventKind::RepresentationResourceAdded {
                representation_id: exact(&f["representation_id"])?,
                resource_id: exact(&f["resource_id"])?,
                position: exact(&f["position"])?,
            }
        }
        "locator_added" | "locator_retired" => {
            let f = object(value, &["kind", "resource_id", "locator_id"])?;
            let resource_id = exact(&f["resource_id"])?;
            let locator_id = exact(&f["locator_id"])?;
            if text(&f["kind"])? == "locator_added" {
                RevisionEventKind::LocatorAdded {
                    resource_id,
                    locator_id,
                }
            } else {
                RevisionEventKind::LocatorRetired {
                    resource_id,
                    locator_id,
                }
            }
        }
        "media_root_added" | "media_root_removed" => {
            let f = object(value, &["kind", "media_root_id"])?;
            let media_root_id = exact(&f["media_root_id"])?;
            if text(&f["kind"])? == "media_root_added" {
                RevisionEventKind::MediaRootAdded { media_root_id }
            } else {
                RevisionEventKind::MediaRootRemoved { media_root_id }
            }
        }
        "media_root_enabled_changed" => {
            let f = object(value, &["kind", "media_root_id", "enabled"])?;
            RevisionEventKind::MediaRootEnabledChanged {
                media_root_id: exact(&f["media_root_id"])?,
                enabled: f["enabled"].as_bool().ok_or_else(malformed)?,
            }
        }
        "resource_file_facts_observed" => {
            let f = object(value, &["kind", "resource_id"])?;
            RevisionEventKind::ResourceFileFactsObserved {
                resource_id: exact(&f["resource_id"])?,
            }
        }
        _ => return Err(unsupported()),
    })
}
