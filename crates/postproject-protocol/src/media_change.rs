//! Authored locator and root changes, separate from their observation projection.

use postproject_core::{
    Locator, LocatorId, MediaRoot, MediaRootId, ResourceId, RevisionEventKind, SemanticConflictKey,
};
use serde_json::json;

use crate::{
    Document, Result, decode_locator, decode_root, encode_locator, encode_root,
    fields::{exact, malformed, object, unsupported},
};

/// One authored root or locator mutation, without local resolver mappings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaChange {
    /// Complete original configured-root facts.
    RootAdded(MediaRoot),
    /// A changed enabled flag on an existing root.
    RootEnabled {
        /// Existing configured-root identity.
        root_id: MediaRootId,
        /// Authored state.
        enabled: bool,
    },
    /// Removal of an existing configured root.
    RootRemoved(MediaRootId),
    /// Complete original locator facts, including per-copy sequence naming.
    LocatorAdded(Locator),
    /// Retirement of one existing locator from its resource.
    LocatorRetired {
        /// Retired access-route identity.
        locator_id: LocatorId,
        /// Original owning resource.
        resource_id: ResourceId,
    },
}

impl MediaChange {
    /// Encodes complete authored facts without executing a filesystem operation.
    ///
    /// # Errors
    /// Rejects unsupported future domain alternatives.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: match self {
                Self::RootAdded(root) => {
                    json!({"kind":"root.added", "root":encode_root(root).value})
                }
                Self::RootEnabled { root_id, enabled } => {
                    json!({"kind":"root.enabled", "id":root_id.to_string(), "enabled":enabled})
                }
                Self::RootRemoved(id) => json!({"kind":"root.removed", "id":id.to_string()}),
                Self::LocatorAdded(locator) => {
                    json!({"kind":"locator.added", "locator":encode_locator(locator)?.value})
                }
                Self::LocatorRetired {
                    locator_id,
                    resource_id,
                } => {
                    json!({"kind":"locator.retired", "id":locator_id.to_string(), "resource_id":resource_id.to_string()})
                }
            },
        })
    }

    /// Decodes checked facts; a receiver also checks existing state and ownership.
    ///
    /// # Errors
    /// Rejects unknown kinds/fields and malformed domain values.
    pub fn from_document(document: &Document) -> Result<Self> {
        Ok(match document.kind()? {
            "root.added" => {
                let fields = object(&document.value, &["kind", "root"])?;
                Self::RootAdded(decode_root(&Document {
                    value: fields["root"].clone(),
                })?)
            }
            "root.enabled" => {
                let fields = object(&document.value, &["kind", "id", "enabled"])?;
                Self::RootEnabled {
                    root_id: exact(&fields["id"])?,
                    enabled: fields["enabled"].as_bool().ok_or_else(malformed)?,
                }
            }
            "root.removed" => {
                let fields = object(&document.value, &["kind", "id"])?;
                Self::RootRemoved(exact(&fields["id"])?)
            }
            "locator.added" => {
                let fields = object(&document.value, &["kind", "locator"])?;
                Self::LocatorAdded(decode_locator(&Document {
                    value: fields["locator"].clone(),
                })?)
            }
            "locator.retired" => {
                let fields = object(&document.value, &["kind", "id", "resource_id"])?;
                Self::LocatorRetired {
                    locator_id: exact(&fields["id"])?,
                    resource_id: exact(&fields["resource_id"])?,
                }
            }
            _ => return Err(unsupported()),
        })
    }

    /// Returns the original public observation implied by this effect.
    #[must_use]
    pub fn observation(&self) -> RevisionEventKind {
        match self {
            Self::RootAdded(root) => RevisionEventKind::MediaRootAdded {
                media_root_id: root.id(),
            },
            Self::RootEnabled { root_id, enabled } => RevisionEventKind::MediaRootEnabledChanged {
                media_root_id: *root_id,
                enabled: *enabled,
            },
            Self::RootRemoved(root_id) => RevisionEventKind::MediaRootRemoved {
                media_root_id: *root_id,
            },
            Self::LocatorAdded(locator) => RevisionEventKind::LocatorAdded {
                resource_id: locator.resource_id(),
                locator_id: locator.id(),
            },
            Self::LocatorRetired {
                locator_id,
                resource_id,
            } => RevisionEventKind::LocatorRetired {
                resource_id: *resource_id,
                locator_id: *locator_id,
            },
        }
    }

    /// Returns the semantic fact whose changed boundary this effect advances.
    #[must_use]
    pub const fn conflict_key(&self) -> SemanticConflictKey {
        match self {
            Self::RootAdded(root) => SemanticConflictKey::MediaRoot(root.id()),
            Self::RootEnabled { root_id, .. } | Self::RootRemoved(root_id) => {
                SemanticConflictKey::MediaRoot(*root_id)
            }
            Self::LocatorAdded(locator) => SemanticConflictKey::LocatorSet(locator.resource_id()),
            Self::LocatorRetired { resource_id, .. } => {
                SemanticConflictKey::LocatorSet(*resource_id)
            }
        }
    }
}
