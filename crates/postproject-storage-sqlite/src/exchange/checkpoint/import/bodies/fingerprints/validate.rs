use rusqlite::Connection;

use crate::{ExchangeResult, sqlite_error};

use super::super::super::super::invalid;

pub(super) fn boundaries(connection: &Connection, head: u64) -> ExchangeResult<()> {
    let head = i64::try_from(head).map_err(|_| invalid())?;
    for (table, owner) in [
        ("resource", "resource_id"),
        ("representation", "representation_id"),
    ] {
        let out_of_range: bool = connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table}_fingerprints WHERE observed_revision_sequence > ?1 UNION ALL SELECT 1 FROM {table}_fingerprint_history WHERE observed_revision_sequence > superseded_revision_sequence OR superseded_revision_sequence > ?1)"), [head], |row| row.get(0))
            .map_err(sqlite_error("validate checkpoint fingerprint boundaries"))?;
        let broken_history: bool = connection.query_row(&format!("WITH ordered AS (SELECT *, row_number() OVER domain AS position, lag(value) OVER domain AS previous_value, lag(superseded_revision_sequence) OVER domain AS previous_boundary FROM {table}_fingerprint_history WINDOW domain AS (PARTITION BY {owner}, algorithm, algorithm_version ORDER BY id)) SELECT EXISTS(SELECT 1 FROM ordered WHERE position > 1 AND (observed_revision_sequence IS NOT previous_boundary OR value = previous_value))"), [], |row| row.get(0))
            .map_err(sqlite_error("validate checkpoint fingerprint history continuity"))?;
        let inconsistent_current: bool = connection.query_row(&format!("WITH latest AS (SELECT *, row_number() OVER (PARTITION BY {owner}, algorithm, algorithm_version ORDER BY id DESC) AS position FROM {table}_fingerprint_history) SELECT EXISTS(SELECT 1 FROM latest h LEFT JOIN {table}_fingerprints c ON c.{owner} = h.{owner} AND c.algorithm = h.algorithm AND c.algorithm_version = h.algorithm_version WHERE h.position = 1 AND (c.{owner} IS NULL OR c.observed_revision_sequence IS NOT h.superseded_revision_sequence OR c.value = h.value))"), [], |row| row.get(0))
            .map_err(sqlite_error("validate checkpoint current fingerprint continuity"))?;
        if out_of_range || broken_history || inconsistent_current {
            return Err(invalid().into());
        }
    }
    let invalid_marker: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM representation_fingerprint_recomputations m WHERE m.marked_revision_sequence > ?1 OR NOT EXISTS(SELECT 1 FROM representation_resources r WHERE r.representation_id = m.representation_id AND r.resource_id = m.changed_resource_id))", [head], |row| row.get(0))
        .map_err(sqlite_error("validate checkpoint recomputation membership"))?;
    if invalid_marker {
        return Err(invalid().into());
    }
    Ok(())
}
