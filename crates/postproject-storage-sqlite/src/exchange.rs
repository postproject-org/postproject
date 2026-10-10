//! Private exchange persistence; portable history is independent of read cursors.

mod activity_capture;
mod captured;
mod checkpoint;
mod error;
mod fragments;
mod genesis;
pub(crate) mod job_capture;
mod outcomes;
mod records;
mod resynchronization;

pub(crate) use captured::{CapturedEffect, CapturedFingerprint};
pub use checkpoint::CheckpointLimits;
pub(crate) use checkpoint::export as export_checkpoint;
pub(crate) use checkpoint::import as import_checkpoint;
pub(crate) use checkpoint::recover as recover_checkpoint_import;
pub use error::{ExchangeError, ExchangeResult};
pub(crate) use fragments::persist_metadata_effects;
pub(crate) use genesis::create as create_genesis_mirror;
pub(crate) use outcomes::{lookup_for_submission, persist};
pub(crate) use records::apply;
pub(crate) use records::capture_records;
pub(crate) use records::export_range;
pub(crate) use records::position;
pub use records::{RecordReader, ReplayLimits};
pub use resynchronization::ResynchronizationLimits;
pub(crate) use resynchronization::establish_floor;

use postproject_core::{DecisionBase, Error, ErrorKind, ProductionId, Result, RevisionId};
use postproject_protocol::{Digest, HistoryId, Position, ProtocolBase, Scope, StoreRole};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{id_bytes, sqlite_error, stored_u64};

pub(crate) fn role(connection: &Connection) -> Result<StoreRole> {
    let role = connection
        .query_row(
            "SELECT role FROM exchange_history WHERE singleton = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(sqlite_error("read exchange store role"))?;
    match role {
        1 => Ok(StoreRole::Authority),
        2 => Ok(StoreRole::PassiveMirror),
        _ => Err(Error::new(
            ErrorKind::Storage,
            "invalid exchange store role",
        )),
    }
}

pub(crate) fn require_authority(connection: &Connection) -> Result<()> {
    if role(connection)? != StoreRole::Authority {
        return Err(Error::new(
            ErrorKind::Unsupported,
            "passive mirror rejects ordinary writes and worker authority",
        ));
    }
    Ok(())
}

pub(crate) fn scope(connection: &Connection, production: ProductionId) -> Result<Scope> {
    let history = connection
        .query_row(
            "SELECT history_id FROM exchange_history WHERE singleton = 1",
            [],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .map_err(sqlite_error("read exchange history"))?;
    Ok(Scope::new(
        production,
        HistoryId::from_bytes(id_bytes(history, "exchange history")?),
    ))
}

pub(crate) fn floor(connection: &Connection, production: ProductionId) -> Result<Position> {
    let scope = scope(connection, production)?;
    let (revision, sequence, digest) = connection.query_row(
        "SELECT floor_revision_id, floor_sequence, anchor_digest FROM exchange_history WHERE singleton = 1", [],
        |row| Ok((row.get::<_, Option<Vec<u8>>>(0)?, row.get::<_, i64>(1)?, row.get::<_, Vec<u8>>(2)?)),
    ).map_err(sqlite_error("read exchange floor"))?;
    let revision = revision
        .map(|id| id_bytes(id, "exchange floor revision").map(RevisionId::from_bytes))
        .transpose()?;
    let digest: [u8; 32] = digest
        .try_into()
        .map_err(|_| Error::new(ErrorKind::Storage, "invalid exchange anchor length"))?;
    let position = Position::new(
        scope,
        revision,
        stored_u64(sequence, "exchange floor sequence")?,
        Digest::from_bytes(digest),
    )
    .map_err(|_| Error::new(ErrorKind::Storage, "invalid exchange floor"))?;
    let expected = Position::anchor(position.decision_base())
        .map_err(|_| Error::new(ErrorKind::Storage, "invalid exchange anchor"))?;
    if expected != position {
        return Err(Error::new(
            ErrorKind::Storage,
            "exchange anchor digest mismatch",
        ));
    }
    if let Some(revision) = position.revision() {
        let actual = connection
            .query_row(
                "SELECT sequence FROM revisions WHERE id = ?1",
                [revision.as_bytes().as_slice()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sqlite_error("validate exchange floor revision"))?;
        if actual != Some(sequence) {
            return Err(Error::new(
                ErrorKind::Storage,
                "exchange floor revision mismatch",
            ));
        }
    }
    Ok(position)
}

/// Called inside migration/creation, before committing a usable production.
pub(crate) fn initialize_anchor(connection: &Connection) -> Result<()> {
    let production = connection
        .query_row(
            "SELECT id FROM productions WHERE singleton = 1",
            [],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(sqlite_error("read anchor production"))?;
    let Some(production) = production else {
        return Ok(());
    };
    let production = ProductionId::from_bytes(id_bytes(production, "anchor production")?);
    let scope = scope(connection, production)?;
    let (revision, sequence, digest) = connection.query_row(
        "SELECT floor_revision_id, floor_sequence, anchor_digest FROM exchange_history WHERE singleton = 1", [],
        |row| Ok((row.get::<_, Option<Vec<u8>>>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<Vec<u8>>>(2)?)),
    ).map_err(sqlite_error("read uninitialized exchange floor"))?;
    if digest.is_some() {
        return Ok(());
    }
    let revision = revision
        .map(|id| id_bytes(id, "anchor revision").map(RevisionId::from_bytes))
        .transpose()?;
    let base = ProtocolBase::new(
        scope,
        DecisionBase::new(
            production,
            revision,
            stored_u64(sequence, "anchor sequence")?,
        )?,
    )
    .map_err(|_| Error::new(ErrorKind::Storage, "invalid anchor base"))?;
    let anchor = Position::anchor(base)
        .map_err(|_| Error::new(ErrorKind::Storage, "cannot encode anchor"))?;
    connection.execute("UPDATE exchange_history SET anchor_digest = ?1 WHERE singleton = 1 AND anchor_digest IS NULL", params![anchor.digest().as_bytes().as_slice()])
        .map_err(sqlite_error("persist exchange anchor"))?;
    Ok(())
}
