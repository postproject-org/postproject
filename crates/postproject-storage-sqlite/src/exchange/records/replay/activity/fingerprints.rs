use postproject_core::{FingerprintSnapshot, RepresentationId};
use postproject_protocol::{Document, decode_fingerprint_snapshot};
use rusqlite::{OptionalExtension, Transaction, params};

use super::super::effects::invalid;
use super::context::Context;
use crate::{ExchangeResult, sqlite_error, stored_u64};

pub(super) fn count(
    transaction: &Transaction<'_>,
    subject: RepresentationId,
    expected: u64,
) -> ExchangeResult<()> {
    let actual: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM representation_fingerprints WHERE representation_id = ?1",
            [subject.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("validate captured fingerprint count"))?;
    if u64::try_from(actual).ok() != Some(expected) {
        return Err(invalid().into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct Owner {
    pub(super) table: &'static str,
    pub(super) column: &'static str,
    pub(super) id: i64,
    pub(super) subject: RepresentationId,
}

pub(super) fn push(
    transaction: &Transaction<'_>,
    context: Context<'_>,
    document: &Document,
    owner: Owner,
    previous: &mut Option<(String, u16)>,
) -> ExchangeResult<()> {
    let Owner {
        table,
        column,
        id,
        subject,
    } = owner;
    let snapshot = decode_fingerprint_snapshot(document)?;
    let domain = (snapshot.algorithm().to_owned(), snapshot.version());
    if previous
        .as_ref()
        .is_some_and(|previous| previous >= &domain)
        || snapshot
            .observed_revision_sequence()
            .is_some_and(|sequence| sequence > context.sequence())
    {
        return Err(invalid().into());
    }
    if context.prefix() {
        let current = transaction.query_row("SELECT value, observed_revision_sequence FROM representation_fingerprints WHERE representation_id = ?1 AND algorithm = ?2 AND algorithm_version = ?3", params![subject.as_bytes().as_slice(), snapshot.algorithm(), snapshot.version()], |row|Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<i64>>(1)?))).optional().map_err(sqlite_error("validate original captured fingerprint"))?;
        let current = current
            .map(|(value, sequence)| -> ExchangeResult<_> {
                Ok(FingerprintSnapshot::new(
                    snapshot.algorithm(),
                    snapshot.version(),
                    value,
                    sequence
                        .map(|sequence| stored_u64(sequence, "original fingerprint boundary"))
                        .transpose()?,
                )?)
            })
            .transpose()?;
        if current.as_ref() != Some(&snapshot) {
            return Err(invalid().into());
        }
    }
    transaction.execute(&format!("INSERT INTO {table} ({column}, algorithm, algorithm_version, value, observed_revision_sequence) VALUES (?1, ?2, ?3, ?4, ?5)"), params![id, snapshot.algorithm(), snapshot.version(), snapshot.value(), snapshot.observed_revision_sequence().map(i64::try_from).transpose().map_err(|_|invalid())?]).map_err(sqlite_error("stage original fingerprint evidence"))?;
    *previous = Some(domain);
    Ok(())
}
