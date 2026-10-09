//! Validate the complete source body before one atomic passive publication.

mod activity;
mod creation;
mod dependency;
mod effects;
mod facts;
mod fingerprint_change;
mod identifier_change;
mod job;
mod limits;
mod media_change;

pub use limits::ReplayLimits;

use postproject_core::{Error, ErrorKind, ProductionId};
use postproject_protocol::{
    FailureKind, FrameDecoder, ProtocolError, RecordChunk, RecordManifest, StoreRole,
};
use rusqlite::{Transaction, TransactionBehavior, params};

use crate::{ExchangeResult, SqliteProduction, sqlite_error, transaction::persist_revision_header};

pub(crate) fn apply(
    production: &mut SqliteProduction,
    manifest: &RecordManifest,
    chunks: impl IntoIterator<Item = ExchangeResult<RecordChunk>>,
    limits: ReplayLimits,
) -> ExchangeResult<bool> {
    if production.read_scope.is_some() {
        return Err(Error::new(
            ErrorKind::Unsupported,
            "a pinned read view cannot apply records",
        )
        .into());
    }
    let transaction = production
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error("begin passive record apply"))?;
    let duplicate = validate_boundary(&transaction, production.production.id(), manifest)?;
    let bytes = manifest.document()?.canonical_bytes()?;
    let mut remaining = limits
        .encoded_bytes
        .checked_sub(u64::try_from(bytes.len()).map_err(|_| budget())?)
        .ok_or_else(budget)?;
    if !duplicate {
        transaction
            .execute_batch("PRAGMA defer_foreign_keys = ON")
            .map_err(sqlite_error("defer private replay references"))?;
        persist_revision_header(&transaction, manifest.revision())?;
    }
    let mut chain = manifest.chunk_chain();
    let mut chunks = chunks.into_iter();
    for _ in 0..manifest.chunks().count() {
        let chunk = chunks.next().ok_or_else(|| {
            ProtocolError::new(
                FailureKind::HistoryGap,
                "record stream ended before its advertised boundary",
            )
        })??;
        let size =
            u64::try_from(chunk.document()?.canonical_bytes()?.len()).map_err(|_| budget())?;
        remaining = remaining.checked_sub(size).ok_or_else(budget)?;
        chain.push(&chunk)?;
        if !duplicate {
            super::chunks::persist(&transaction, &chunk)?;
        }
    }
    if let Some(extra) = chunks.next() {
        extra?;
        return Err(ProtocolError::new(
            FailureKind::Integrity,
            "record stream has unadvertised chunks",
        )
        .into());
    }
    manifest.verify_chain(chain)?;
    if duplicate {
        return Ok(false);
    }
    let mut decoder = FrameDecoder::new(limits.document);
    let mut effects = effects::ApplyEffects::new(&transaction, manifest);
    for index in 0..manifest.chunks().count() {
        let chunk = super::chunks::load(&transaction, manifest.revision().id(), index)?
            .ok_or_else(super::invalid)?;
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, document) = decoder.consume(&chunk.payload()[offset..])?;
            offset += consumed;
            if let Some(document) = document {
                effects.document(&document)?;
            }
        }
    }
    decoder.finish()?;
    effects.finish()?;
    transaction
        .execute(
            "INSERT INTO exchange_records (revision_id, sequence, manifest) VALUES (?1, ?2, ?3)",
            params![
                manifest.revision().id().as_bytes().as_slice(),
                i64::try_from(manifest.revision().sequence()).map_err(|_| super::invalid())?,
                bytes
            ],
        )
        .map_err(sqlite_error("persist applied source record"))?;
    transaction
        .commit()
        .map_err(sqlite_error("commit passive record apply"))?;
    production.revision_signal.notify_commit();
    Ok(true)
}

fn validate_boundary(
    transaction: &Transaction<'_>,
    production: ProductionId,
    manifest: &RecordManifest,
) -> ExchangeResult<bool> {
    if crate::exchange::role(transaction)? != StoreRole::PassiveMirror {
        return Err(ProtocolError::new(
            FailureKind::Unsupported,
            "record apply requires a passive mirror",
        )
        .into());
    }
    let floor = crate::exchange::floor(transaction, production)?;
    if floor.scope() != manifest.predecessor().scope() {
        return Err(ProtocolError::new(
            FailureKind::ScopeMismatch,
            "record belongs to another source history",
        )
        .into());
    }
    let sequence: i64 = transaction
        .query_row(
            "SELECT coalesce(max(sequence), 0) FROM revisions",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error("read passive apply boundary"))?;
    let sequence = crate::stored_u64(sequence, "passive revision sequence")?.max(floor.sequence());
    let head = super::position(transaction, production, sequence)?.ok_or_else(|| {
        ProtocolError::new(
            FailureKind::HistoryGap,
            "mirror has no complete replay boundary",
        )
    })?;
    let duplicate = manifest.revision().sequence() <= head.sequence();
    if duplicate {
        let stored = super::manifest(transaction, production, manifest.revision().sequence())?
            .ok_or_else(|| {
                ProtocolError::new(
                    FailureKind::HistoryGap,
                    "record predates retained replay history",
                )
            })?;
        if &stored != manifest {
            return Err(ProtocolError::new(
                FailureKind::Divergence,
                "applied sequence has different source contents",
            )
            .into());
        }
    } else if manifest.predecessor() != head {
        let kind = if manifest.predecessor().sequence() == head.sequence() {
            FailureKind::Divergence
        } else {
            FailureKind::HistoryGap
        };
        return Err(ProtocolError::new(kind, "record does not follow the mirror boundary").into());
    }
    Ok(duplicate)
}

fn budget() -> ProtocolError {
    ProtocolError::new(
        FailureKind::LimitExceeded,
        "record exceeds receiver staging budget",
    )
}
