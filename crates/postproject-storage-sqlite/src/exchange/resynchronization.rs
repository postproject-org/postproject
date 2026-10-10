use postproject_core::{DecisionBase, Error, ErrorKind};
use postproject_protocol::{FailureKind, Position, ProtocolBase, ProtocolError};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

/// Explicit budgets for checking retained history before recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResynchronizationLimits {
    records: u64,
    encoded_bytes: u64,
}

impl ResynchronizationLimits {
    /// Creates positive record-count and encoded-evidence budgets.
    ///
    /// # Errors
    /// Rejects zero budgets. Limits do not constrain native production writes.
    pub fn new(records: u64, encoded_bytes: u64) -> postproject_protocol::Result<Self> {
        if records == 0 || encoded_bytes == 0 {
            return Err(budget());
        }
        Ok(Self {
            records,
            encoded_bytes,
        })
    }
}

impl Default for ResynchronizationLimits {
    fn default() -> Self {
        Self {
            records: 10_000_000,
            encoded_bytes: 1024 * 1024 * 1024,
        }
    }
}

pub(crate) fn establish_floor(
    production: &mut SqliteProduction,
    base: ProtocolBase,
    limits: ResynchronizationLimits,
) -> ExchangeResult<Position> {
    if production.read_scope.is_some() {
        return Err(Error::new(
            ErrorKind::Unsupported,
            "pinned views cannot resynchronize history",
        )
        .into());
    }
    let transaction = production
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error("begin explicit history resynchronization"))?;
    super::require_authority(&transaction)?;
    super::outcomes::validate_scope(&transaction, base.scope())?;
    let revision = transaction.query_row("SELECT id, sequence, transaction_id, committed_at_micros, origin_name, origin_version, origin_uri, message FROM revisions ORDER BY sequence DESC LIMIT 1", [], crate::stored_revision_row)
        .optional().map_err(sqlite_error("read resynchronization head"))?
        .map(crate::decode_revision).transpose()?;
    let current = DecisionBase::new(
        production.production.id(),
        revision.as_ref().map(postproject_core::Revision::id),
        revision
            .as_ref()
            .map_or(0, postproject_core::Revision::sequence),
    )?;
    if base.decision() != current {
        return Err(ProtocolError::new(
            FailureKind::InvalidBase,
            "resynchronization requires the current scoped head",
        )
        .into());
    }
    let floor = super::floor(&transaction, production.production.id())?;
    if !has_gap(&transaction, floor, current.sequence(), limits)? {
        return Ok(floor);
    }
    let anchor = Position::anchor(base)?;
    transaction.execute("INSERT INTO exchange_prior_anchors (floor_sequence, floor_revision_id, anchor_digest) VALUES (?1, ?2, ?3)", params![i64::try_from(floor.sequence()).map_err(|_| invalid())?, floor.revision().map(|id| id.as_bytes().to_vec()), floor.digest().as_bytes().as_slice()])
        .map_err(sqlite_error("retain prior history anchor"))?;
    transaction.execute("UPDATE exchange_history SET floor_sequence = ?1, floor_revision_id = ?2, anchor_digest = ?3 WHERE singleton = 1", params![i64::try_from(anchor.sequence()).map_err(|_| invalid())?, anchor.revision().map(|id| id.as_bytes().to_vec()), anchor.digest().as_bytes().as_slice()])
        .map_err(sqlite_error("establish explicit resynchronization floor"))?;
    transaction
        .commit()
        .map_err(sqlite_error("commit explicit history resynchronization"))?;
    Ok(anchor)
}

fn has_gap(
    connection: &Connection,
    floor: Position,
    head: u64,
    limits: ResynchronizationLimits,
) -> ExchangeResult<bool> {
    if head - floor.sequence() > limits.records {
        return Err(budget().into());
    }
    let mut remaining = limits.encoded_bytes;
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM exchange_records WHERE sequence > ?1 AND sequence <= ?2",
            params![
                i64::try_from(floor.sequence()).map_err(|_| invalid())?,
                i64::try_from(head).map_err(|_| invalid())?
            ],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check retained record coverage"))?;
    if u64::try_from(count).ok() != Some(head - floor.sequence()) {
        return Ok(true);
    }
    let mut previous = floor;
    for sequence in floor.sequence() + 1..=head {
        let manifest = super::records::manifest(connection, floor.scope().production(), sequence)?
            .ok_or_else(invalid)?;
        remaining = remaining
            .checked_sub(
                u64::try_from(manifest.document()?.canonical_bytes()?.len())
                    .map_err(|_| budget())?,
            )
            .ok_or_else(budget)?;
        if manifest.predecessor() != previous {
            return Ok(true);
        }
        let count: i64 = connection.query_row("SELECT COUNT(DISTINCT position) FROM exchange_record_chunks WHERE revision_id = ?1", [manifest.revision().id().as_bytes().as_slice()], |row| row.get(0))
            .map_err(sqlite_error("check retained chunk coverage"))?;
        if u64::try_from(count).ok() != Some(manifest.chunks().count()) {
            return Ok(true);
        }
        let mut chain = manifest.chunk_chain();
        for index in 0..manifest.chunks().count() {
            let Some(chunk) =
                super::records::chunks::load(connection, manifest.revision().id(), index)?
            else {
                return Ok(true);
            };
            remaining = remaining
                .checked_sub(
                    u64::try_from(chunk.document()?.canonical_bytes()?.len())
                        .map_err(|_| budget())?,
                )
                .ok_or_else(budget)?;
            chain.push(&chunk)?;
        }
        manifest.verify_chain(chain)?;
        previous = manifest.head()?;
    }
    Ok(false)
}

fn invalid() -> Error {
    Error::new(ErrorKind::Storage, "inconsistent retained history evidence")
}

fn budget() -> ProtocolError {
    ProtocolError::new(
        FailureKind::LimitExceeded,
        "history validation exceeds resynchronization budget",
    )
}
