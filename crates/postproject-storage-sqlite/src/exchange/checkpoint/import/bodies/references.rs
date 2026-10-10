//! Check polymorphic ownership as well as SQLite's explicit foreign keys.

mod activities;
mod dependencies;

use postproject_core::{ObjectRef, RepresentationId, RevisionEventKind, SemanticConflictKey};
use rusqlite::params;

use crate::{ExchangeResult, sqlite_error, transaction::ensure_metadata_target_exists};

use super::{super::super::invalid, Bodies, media::structural};

impl Bodies<'_, '_> {
    pub(super) fn target(&self, target: ObjectRef) -> ExchangeResult<()> {
        self.target_scope(target)?;
        let (kind, id) = crate::encode_metadata_target(&target)?;
        structural(ensure_metadata_target_exists(self.transaction, kind, id))
    }

    pub(super) fn target_scope(&self, target: ObjectRef) -> ExchangeResult<()> {
        if matches!(target, ObjectRef::Production(id) if id != self.manifest.head().scope().production())
        {
            return Err(invalid().into());
        }
        Ok(())
    }

    pub(super) fn semantic_target(&self, key: &SemanticConflictKey) -> ExchangeResult<()> {
        let target = match key {
            SemanticConflictKey::MediaRoot(_) => return Ok(()),
            SemanticConflictKey::MetadataProperty { target, .. }
            | SemanticConflictKey::ExternalIdentifier { target, .. } => *target,
            SemanticConflictKey::ResourceFileFacts(id)
            | SemanticConflictKey::LocatorSet(id)
            | SemanticConflictKey::ResourceFingerprint {
                resource_id: id, ..
            } => ObjectRef::Resource(*id),
            SemanticConflictKey::DependencySet(id)
            | SemanticConflictKey::RepresentationFingerprint {
                representation_id: id,
                ..
            } => ObjectRef::Representation(*id),
            _ => return Err(invalid().into()),
        };
        self.target(target)
    }

    pub(super) fn event_targets(&self, event: &RevisionEventKind) -> ExchangeResult<()> {
        match event {
            RevisionEventKind::AssetImported { asset_id } => {
                self.target(ObjectRef::Asset(*asset_id))
            }
            RevisionEventKind::RepresentationAdded {
                asset_id,
                representation_id,
            } => {
                let belongs: bool = self.transaction.query_row("SELECT EXISTS(SELECT 1 FROM representations WHERE id = ?1 AND asset_id = ?2)", params![representation_id.as_bytes().as_slice(), asset_id.as_bytes().as_slice()], |row|row.get(0))
                    .map_err(sqlite_error("validate immutable representation ownership observation"))?;
                if !belongs {
                    return Err(invalid().into());
                }
                Ok(())
            }
            RevisionEventKind::ResourceAdded { resource_id }
            | RevisionEventKind::LocatorAdded { resource_id, .. }
            | RevisionEventKind::LocatorRetired { resource_id, .. }
            | RevisionEventKind::ResourceFingerprintObserved { resource_id, .. }
            | RevisionEventKind::ResourceFileFactsObserved { resource_id } => {
                self.target(ObjectRef::Resource(*resource_id))
            }
            RevisionEventKind::RepresentationResourceAdded {
                representation_id,
                resource_id,
                position,
            } => {
                let member: bool = self.transaction.query_row("SELECT EXISTS(SELECT 1 FROM representation_resources WHERE representation_id = ?1 AND resource_id = ?2 AND position = ?3)", params![representation_id.as_bytes().as_slice(), resource_id.as_bytes().as_slice(), i64::from(*position)], |row| row.get(0))
                    .map_err(sqlite_error("validate immutable membership observation"))?;
                if !member {
                    return Err(invalid().into());
                }
                Ok(())
            }
            RevisionEventKind::RepresentationFingerprintObserved {
                representation_id, ..
            } => self.target(ObjectRef::Representation(*representation_id)),
            RevisionEventKind::DependencySetRecorded { representation_id } => {
                self.target(ObjectRef::Representation(*representation_id))
            }
            RevisionEventKind::ExternalIdentifierAdded { target, .. }
            | RevisionEventKind::ExternalIdentifierRemoved { target, .. }
            | RevisionEventKind::MetadataAddedOrReplaced { target, .. }
            | RevisionEventKind::MetadataRemoved { target, .. } => self.target(*target),
            RevisionEventKind::MediaRootAdded { .. }
            | RevisionEventKind::MediaRootEnabledChanged { .. }
            | RevisionEventKind::MediaRootRemoved { .. } => Ok(()),
            RevisionEventKind::ActivityCreated { .. }
            | RevisionEventKind::ActivityInputAdded { .. }
            | RevisionEventKind::ActivityOutputAdded { .. } => self.activity_event(event),
            _ => Err(postproject_protocol::ProtocolError::new(
                postproject_protocol::FailureKind::Unsupported,
                "checkpoint observation family is not supported yet",
            )
            .into()),
        }
    }

    pub(super) fn validate_media(&self) -> ExchangeResult<()> {
        self.validate_activities()?;
        self.validate_dependencies()?;
        let mut targets = self.transaction.prepare("SELECT DISTINCT target_kind, target_id FROM metadata_assertions UNION SELECT DISTINCT target_kind, target_id FROM external_identifiers")
            .map_err(sqlite_error("prepare checkpoint attachment ownership"))?;
        let mut rows = targets
            .query([])
            .map_err(sqlite_error("query checkpoint attachment owners"))?;
        while let Some(row) = rows
            .next()
            .map_err(sqlite_error("read checkpoint attachment owner"))?
        {
            let target = crate::decode_metadata_target(
                row.get(0)
                    .map_err(sqlite_error("read attachment owner kind"))?,
                row.get(1)
                    .map_err(sqlite_error("read attachment owner identity"))?,
            )?;
            self.target(target)?;
        }
        let mut statement = self
            .transaction
            .prepare("SELECT id FROM representations ORDER BY id")
            .map_err(sqlite_error("prepare checkpoint structure completeness"))?;
        let mut rows = statement
            .query([])
            .map_err(sqlite_error("query checkpoint structures"))?;
        while let Some(row) = rows
            .next()
            .map_err(sqlite_error("read checkpoint structure owner"))?
        {
            let id = RepresentationId::from_bytes(crate::id_bytes(
                row.get(0)
                    .map_err(sqlite_error("read checkpoint representation identity"))?,
                "checkpoint representation",
            )?);
            super::super::super::media_facts::content(self.transaction, id)?;
        }
        let orphan: bool = self.transaction.query_row("SELECT EXISTS(SELECT 1 FROM resources r WHERE NOT EXISTS(SELECT 1 FROM representation_resources m WHERE m.resource_id = r.id))", [], |row| row.get(0))
            .map_err(sqlite_error("validate checkpoint resource membership"))?;
        if orphan {
            return Err(invalid().into());
        }
        Ok(())
    }
}
