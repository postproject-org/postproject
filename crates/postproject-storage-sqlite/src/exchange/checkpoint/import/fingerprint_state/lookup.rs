use postproject_core::{FingerprintSnapshot, ObjectRef};
use rusqlite::{Connection, OptionalExtension, params};

use super::invalid;
use crate::{ExchangeResult, sqlite_error};

pub(super) struct Domain<'a> {
    pub(super) kind: i64,
    pub(super) owner: [u8; 16],
    pub(super) algorithm: &'a str,
    pub(super) version: u16,
}

impl<'a> Domain<'a> {
    pub(super) fn new(
        target: ObjectRef,
        snapshot: &'a FingerprintSnapshot,
    ) -> ExchangeResult<Self> {
        let (kind, owner) = match target {
            ObjectRef::Representation(id) => (2, id.into_bytes()),
            ObjectRef::Resource(id) => (3, id.into_bytes()),
            _ => return Err(invalid().into()),
        };
        Ok(Self {
            kind,
            owner,
            algorithm: snapshot.algorithm(),
            version: snapshot.version(),
        })
    }
}

fn snapshot(
    domain: &Domain<'_>,
    value: Vec<u8>,
    observed: Option<i64>,
) -> ExchangeResult<FingerprintSnapshot> {
    Ok(FingerprintSnapshot::new(
        domain.algorithm,
        domain.version,
        value,
        observed
            .map(|sequence| crate::stored_u64(sequence, "original fingerprint observation"))
            .transpose()?,
    )?)
}

pub(super) fn current(
    connection: &Connection,
    domain: &Domain<'_>,
) -> ExchangeResult<Option<FingerprintSnapshot>> {
    let (table, owner) = if domain.kind == 2 {
        ("representation_fingerprints", "representation_id")
    } else {
        ("resource_fingerprints", "resource_id")
    };
    let result = connection.query_row(&format!("SELECT value, observed_revision_sequence FROM {table} WHERE {owner} = ?1 AND algorithm = ?2 AND algorithm_version = ?3"), params![domain.owner.as_slice(), domain.algorithm, domain.version], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional().map_err(sqlite_error("read current checkpoint fingerprint evidence"))?;
    result
        .map(|(value, observed)| snapshot(domain, value, observed))
        .transpose()
}

pub(super) fn history(
    connection: &Connection,
    domain: &Domain<'_>,
    position: u64,
) -> ExchangeResult<Option<(FingerprintSnapshot, u64)>> {
    let result = connection.query_row("SELECT value, observed, superseded FROM checkpoint_fingerprint_history WHERE kind = ?1 AND owner = ?2 AND algorithm = ?3 AND version = ?4 AND position = ?5", params![domain.kind, domain.owner.as_slice(), domain.algorithm, domain.version, i64::try_from(position).map_err(|_| invalid())?], |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)?)))
        .optional().map_err(sqlite_error("read original checkpoint archive position"))?;
    result
        .map(|(value, observed, superseded)| {
            Ok((
                snapshot(domain, value, observed)?,
                crate::stored_u64(superseded, "original fingerprint supersession")?,
            ))
        })
        .transpose()
}

pub(super) fn expected(
    connection: &Connection,
    domain: &Domain<'_>,
) -> ExchangeResult<Option<(FingerprintSnapshot, u64)>> {
    let result = connection.query_row("SELECT value, observed, next_archive FROM checkpoint_fingerprint_state WHERE kind = ?1 AND owner = ?2 AND algorithm = ?3 AND version = ?4", params![domain.kind, domain.owner.as_slice(), domain.algorithm, domain.version], |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)?)))
        .optional().map_err(sqlite_error("read authored checkpoint fingerprint boundary"))?;
    result
        .map(|(value, observed, next)| {
            Ok((
                snapshot(domain, value, observed)?,
                crate::stored_u64(next, "next authored archive position")?,
            ))
        })
        .transpose()
}

pub(super) fn baseline(
    connection: &Connection,
    domain: &Domain<'_>,
    floor: u64,
) -> ExchangeResult<(Option<FingerprintSnapshot>, u64)> {
    let first: Option<i64> = connection.query_row("SELECT position FROM checkpoint_fingerprint_history WHERE kind = ?1 AND owner = ?2 AND algorithm = ?3 AND version = ?4 AND superseded > ?5 ORDER BY superseded, position LIMIT 1", params![domain.kind, domain.owner.as_slice(), domain.algorithm, domain.version, i64::try_from(floor).map_err(|_| invalid())?], |row| row.get(0))
        .optional().map_err(sqlite_error("read migration fingerprint boundary"))?;
    let (value, next) = if let Some(first) = first {
        let position = crate::stored_u64(first, "first post-floor archive")?;
        let (value, _) = history(connection, domain, position)?.ok_or_else(invalid)?;
        (Some(value), position)
    } else {
        let count: i64 = connection.query_row("SELECT count(*) FROM checkpoint_fingerprint_history WHERE kind = ?1 AND owner = ?2 AND algorithm = ?3 AND version = ?4", params![domain.kind, domain.owner.as_slice(), domain.algorithm, domain.version], |row| row.get(0))
            .map_err(sqlite_error("read retained baseline archive count"))?;
        (
            current(connection, domain)?,
            crate::stored_u64(count, "retained baseline archive count")?,
        )
    };
    if value
        .as_ref()
        .and_then(FingerprintSnapshot::observed_revision_sequence)
        .is_some_and(|sequence| sequence > floor)
    {
        if next != 0 {
            return Err(invalid().into());
        }
        return Ok((None, 0));
    }
    Ok((value, next))
}
