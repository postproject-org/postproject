use postproject_core::{ObjectRef, RepresentationKind, ResourceId, RevisionEventKind};
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
                    self.total_events,
                    RepresentationCreationStart::from_document(document)?,
                    &mut self.expected_events,
                )?);
            }
            "identifier.added" | "identifier.removed" => {
                let change = IdentifierChange::from_document(document)?;
                identifier_state::change(connection, &change, self.genesis)?;
                self.expect_observation(connection, &change.observation())?;
                guard_state::recorded(connection, &change.conflict_key(), self.sequence)?;
            }
            "fingerprint.change" => {
                let start = FingerprintChangeStart::from_document(document)?;
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
        recomputation_state::marked(
            connection,
            FingerprintRecomputation::from_document(document)?,
            pending.resource,
            self.sequence,
        )?;
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
                media_state::changed_facts(connection, *resource_id, *facts, self.genesis)
            }
            MediaChange::LocatorAdded(locator) => locator_state::added(connection, locator),
            MediaChange::LocatorRetired {
                locator_id,
                resource_id,
            } => locator_state::retired(connection, *locator_id, *resource_id, self.genesis),
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
            _ => root_state::change(connection, change, self.genesis),
        }
    }
}
