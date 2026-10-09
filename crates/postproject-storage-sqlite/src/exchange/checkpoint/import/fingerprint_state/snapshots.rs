use postproject_core::{FingerprintSnapshot, ObjectRef, RepresentationId};
use rusqlite::Connection;

use crate::{ExchangeResult, sqlite_error};

use super::{invalid, lookup};

fn available(
    connection: &Connection,
    domain: &lookup::Domain<'_>,
    floor: u64,
) -> ExchangeResult<Option<FingerprintSnapshot>> {
    if let Some((snapshot, _)) = lookup::expected(connection, domain)? {
        return Ok(Some(snapshot));
    }
    if floor == 0 {
        return Ok(None);
    }
    Ok(lookup::baseline(connection, domain, floor)?.0)
}

pub(in crate::exchange::checkpoint::import) fn snapshot(
    connection: &Connection,
    owner: RepresentationId,
    snapshot: &FingerprintSnapshot,
    floor: u64,
) -> ExchangeResult<()> {
    let domain = lookup::Domain::new(ObjectRef::Representation(owner), snapshot)?;
    if available(connection, &domain, floor)?.as_ref() != Some(snapshot) {
        return Err(invalid().into());
    }
    Ok(())
}

pub(in crate::exchange::checkpoint::import) fn snapshot_count(
    connection: &Connection,
    owner: RepresentationId,
    expected: u64,
    floor: u64,
) -> ExchangeResult<()> {
    let mut statement = connection.prepare("SELECT algorithm, algorithm_version FROM representation_fingerprints WHERE representation_id = ?1 ORDER BY algorithm, algorithm_version")
        .map_err(sqlite_error("prepare authored activity fingerprint domains"))?;
    let mut rows = statement
        .query([owner.as_bytes().as_slice()])
        .map_err(sqlite_error("query authored activity fingerprint domains"))?;
    let mut count = 0_u64;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read authored activity fingerprint domain"))?
    {
        let algorithm: String = row
            .get(0)
            .map_err(sqlite_error("read activity fingerprint algorithm"))?;
        let version: i64 = row
            .get(1)
            .map_err(sqlite_error("read activity fingerprint version"))?;
        let domain = lookup::Domain {
            kind: 2,
            owner: owner.into_bytes(),
            algorithm: &algorithm,
            version: u16::try_from(version).map_err(|_| invalid())?,
        };
        if available(connection, &domain, floor)?.is_some() {
            count = count.checked_add(1).ok_or_else(invalid)?;
        }
    }
    if count != expected {
        return Err(invalid().into());
    }
    Ok(())
}
