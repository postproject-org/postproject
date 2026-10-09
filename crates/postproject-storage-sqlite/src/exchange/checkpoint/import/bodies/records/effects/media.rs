use postproject_core::{
    ObjectRef, RepresentationId, RepresentationKind, ResourceId, RevisionEventKind,
};
use postproject_protocol::{
    Document, FingerprintChangeStart, FingerprintRecomputation, IdentifierChange, MediaChange,
    RepresentationCreationStart, decode_original_creation_start,
};
use rusqlite::{Connection, OptionalExtension};

use crate::{
    ExchangeResult,
    exchange::checkpoint::import::{
        fingerprint_state, guard_state, identifier_state, locator_state, media_state,
        recomputation_state, root_state,
    },
    sqlite_error,
};

use super::{RetainedEffects, creation::CreationAudit, invalid};

pub(super) struct PendingFingerprint {
    resource: ResourceId,
    remaining: u64,
    total: u64,
    last_owner: Option<RepresentationId>,
}

impl RetainedEffects {
    pub(super) fn original_representation(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<()> {
        let header = RepresentationCreationStart::from_document(document)?;
        if self.original.take() != Some(header.representation().asset_id())
            || header.representation().kind() != RepresentationKind::Original
        {
            return Err(invalid().into());
        }
        self.creation = Some(CreationAudit::new(
            connection,
            self.revision,
            self.sequence,
            self.floor,
            self.total_events,
            header,
            &mut self.expected_events,
        )?);
        Ok(())
    }

    pub(super) fn media_effect(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<bool> {
        match document.kind()? {
            "original.creation" => {
                let asset = decode_original_creation_start(document)?;
                media_state::asset(connection, &asset)?;
                self.expect_observation(
                    connection,
                    &RevisionEventKind::AssetImported {
                        asset_id: asset.id(),
                    },
                )?;
                self.original = Some(asset.id());
            }
            "representation.creation" => {
                self.creation = Some(CreationAudit::new(
                    connection,
                    self.revision,
                    self.sequence,
                    self.floor,
                    self.total_events,
                    RepresentationCreationStart::from_document(document)?,
                    &mut self.expected_events,
                )?);
            }
            "identifier.added" | "identifier.removed" => {
                let change = IdentifierChange::from_document(document)?;
                self.require_media_target(connection, change.attachment().target())?;
                identifier_state::change(connection, &change, self.genesis)?;
                self.expect_observation(connection, &change.observation())?;
                guard_state::recorded(connection, &change.conflict_key(), self.sequence)?;
            }
            "fingerprint.change" => {
                let start = FingerprintChangeStart::from_document(document)?;
                if start.dependency_invalidated() {
                    return Err(postproject_protocol::ProtocolError::new(
                        postproject_protocol::FailureKind::Unsupported,
                        "checkpoint dependency invalidation audit is not supported yet",
                    )
                    .into());
                }
                self.require_media_target(connection, start.target())?;
                fingerprint_state::changed(connection, &start, self.sequence, self.floor)?;
                match start.target() {
                    ObjectRef::Resource(resource) => {
                        recomputation_state::resource_start(
                            connection,
                            resource,
                            start.marker_count(),
                        )?;
                        if start.marker_count() != 0 {
                            self.fingerprint = Some(PendingFingerprint {
                                resource,
                                remaining: start.marker_count(),
                                total: start.marker_count(),
                                last_owner: None,
                            });
                        }
                    }
                    ObjectRef::Representation(_) => {
                        recomputation_state::cleared(
                            connection,
                            &start,
                            self.sequence,
                            self.floor,
                        )?;
                    }
                    _ => return Err(invalid().into()),
                }
                self.expect_observation(connection, &start.observation())?;
                guard_state::recorded(connection, &start.conflict_key(), self.sequence)?;
            }
            "root.added"
            | "root.enabled"
            | "root.removed"
            | "root.removed-facts"
            | "resource.file-facts"
            | "locator.added"
            | "locator.retired" => {
                let change = MediaChange::from_document(document)?;
                self.scalar_media(connection, &change)?;
                self.expect_observation(connection, &change.observation())?;
                guard_state::recorded(connection, &change.conflict_key(), self.sequence)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn fingerprint_marker(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<()> {
        let pending = self.fingerprint.as_mut().ok_or_else(invalid)?;
        let marker = FingerprintRecomputation::from_document(document)?;
        if pending
            .last_owner
            .is_some_and(|previous| previous >= marker.representation_id())
        {
            return Err(invalid().into());
        }
        media_state::require(
            connection,
            ObjectRef::Representation(marker.representation_id()),
            self.floor,
        )?;
        pending.last_owner = Some(marker.representation_id());
        recomputation_state::marked(connection, marker, pending.resource, self.sequence)?;
        pending.remaining = pending.remaining.checked_sub(1).ok_or_else(invalid)?;
        if pending.remaining == 0 {
            recomputation_state::resource_finish(connection, pending.total)?;
            self.fingerprint = None;
        }
        Ok(())
    }

    fn scalar_media(&self, connection: &Connection, change: &MediaChange) -> ExchangeResult<()> {
        match change {
            MediaChange::ResourceFileFacts { resource_id, facts } => {
                self.require_media_target(connection, ObjectRef::Resource(*resource_id))?;
                media_state::changed_facts(connection, *resource_id, *facts, self.genesis)
            }
            MediaChange::LocatorAdded(locator) => {
                self.require_media_target(connection, ObjectRef::Resource(locator.resource_id()))?;
                if let Some(name) = locator.media_root() {
                    root_state::require_name(connection, name, self.genesis)?;
                }
                locator_state::added(connection, locator)
            }
            MediaChange::LocatorRetired {
                locator_id,
                resource_id,
            } => {
                self.require_media_target(connection, ObjectRef::Resource(*resource_id))?;
                locator_state::retired(connection, *locator_id, *resource_id, self.genesis)
            }
            MediaChange::RootRemoved(id) => {
                let name: Option<Option<String>> = connection
                    .query_row(
                        "SELECT name FROM checkpoint_root_state WHERE id = ?1",
                        [id.as_bytes().as_slice()],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(sqlite_error("read original removed root name"))?;
                if let Some(name) = name.flatten() {
                    locator_state::clear_known_root(connection, &name)?;
                }
                root_state::change(connection, change, self.genesis)
            }
            MediaChange::RootRemovedWithFacts(root) => {
                locator_state::clear_known_root(connection, root.name())?;
                root_state::change(connection, change, self.genesis)
            }
            _ => root_state::change(connection, change, self.genesis),
        }
    }

    pub(super) fn require_media_target(
        &self,
        connection: &Connection,
        target: ObjectRef,
    ) -> ExchangeResult<()> {
        if matches!(
            target,
            ObjectRef::Asset(_) | ObjectRef::Representation(_) | ObjectRef::Resource(_)
        ) {
            media_state::require(connection, target, self.floor)?;
        }
        crate::exchange::checkpoint::import::activity_state::require(
            connection, target, self.floor,
        )?;
        Ok(())
    }
}
