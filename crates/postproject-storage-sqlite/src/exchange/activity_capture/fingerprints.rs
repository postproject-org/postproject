use postproject_core::{FingerprintSnapshot, Result};
use postproject_protocol::{Document, encode_fingerprint_snapshot};
use rusqlite::Connection;

use crate::{sqlite_error, stored_domain_error, stored_u64};

pub(super) fn write(
    connection: &Connection,
    table: &'static str,
    column: &'static str,
    owner: i64,
    write: &mut impl FnMut(&Document) -> Result<()>,
) -> Result<()> {
    let sql = format!(
        "SELECT algorithm, algorithm_version, value, observed_revision_sequence FROM {table} WHERE {column} = ?1 ORDER BY algorithm, algorithm_version"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(sqlite_error("prepare authored fingerprints"))?;
    let rows = statement
        .query_map([owner], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(sqlite_error("query authored fingerprints"))?;
    for row in rows {
        let (algorithm, version, value, sequence) =
            row.map_err(sqlite_error("read authored fingerprint"))?;
        let snapshot = FingerprintSnapshot::new(
            algorithm,
            u16::try_from(version).map_err(|_| super::invalid())?,
            value,
            sequence
                .map(|sequence| stored_u64(sequence, "authored fingerprint boundary"))
                .transpose()?,
        )
        .map_err(stored_domain_error("authored fingerprint"))?;
        write(&encode_fingerprint_snapshot(&snapshot).map_err(|_| super::invalid())?)?;
    }
    Ok(())
}
