use postproject_core::RevisionEventType;
use rusqlite::params;

use crate::{ExchangeResult, sqlite_error};

use super::{Bodies, invalid};

impl Bodies<'_, '_> {
    pub(super) fn validate_dependencies(&self) -> ExchangeResult<()> {
        let kind = crate::stored_revision_event_kind(RevisionEventType::DependencySetRecorded)?;
        let mismatch: bool = self.transaction.query_row("SELECT EXISTS(SELECT 1 FROM dependency_sets d WHERE d.recorded_revision_sequence IS NOT (SELECT MAX(r.sequence) FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.kind = ?1 AND e.primary_id = d.source_representation_id) UNION ALL SELECT 1 FROM revision_events e WHERE e.kind = ?1 AND NOT EXISTS(SELECT 1 FROM dependency_sets d WHERE d.source_representation_id = e.primary_id))", params![kind], |row| row.get(0))
            .map_err(sqlite_error("validate original dependency observation boundaries"))?;
        if mismatch {
            return Err(invalid().into());
        }
        Ok(())
    }
}
