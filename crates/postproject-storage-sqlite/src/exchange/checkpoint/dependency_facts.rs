use postproject_core::{DependencySetStatus, RepresentationId};
use postproject_protocol::DependencySetHeader;
use rusqlite::Connection;

use crate::{ExchangeResult, sqlite_error};

#[cfg(test)]
mod tests;

pub(super) fn header(
    connection: &Connection,
    owner: RepresentationId,
) -> ExchangeResult<DependencySetHeader> {
    let (sequence, dirty, count) = connection.query_row("SELECT recorded_revision_sequence, needs_extraction, (SELECT COUNT(*) FROM dependencies WHERE source_representation_id = ?1) FROM dependency_sets WHERE source_representation_id = ?1", [owner.as_bytes().as_slice()], crate::read_budget::bounded(|row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))))
        .map_err(sqlite_error("read checkpoint dependency header"))?;
    let status = match dirty {
        0 => DependencySetStatus::Current,
        1 => DependencySetStatus::NeedsExtraction,
        _ => return Err(super::invalid().into()),
    };
    Ok(DependencySetHeader::new(
        owner,
        crate::stored_u64(sequence, "dependency observation boundary")?,
        status,
        crate::stored_u64(count, "dependency occurrence count")?,
    )?)
}
