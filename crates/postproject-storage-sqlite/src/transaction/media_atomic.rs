use postproject_core::Result;

use super::SqliteTransaction;
use crate::sqlite_error;

impl SqliteTransaction<'_> {
    // Aggregate staging either succeeds completely or leaves no partial domain
    // rows or authored observations, even when a caller handles the error and
    // commits unrelated operations in this same transaction.
    pub(super) fn stage_media_atomically<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let events = self.pending_events.len();
        let effects = self.pending_effects.len();
        let conflict_keys = self.pending_conflict_keys.checkpoint();
        let changed_keys = self.pending_changed_keys.checkpoint();
        self.open_transaction()?
            .execute_batch("SAVEPOINT postproject_media_aggregate")
            .map_err(sqlite_error("start media aggregate savepoint"))?;
        let result = operation(self);
        if result.is_err() {
            self.pending_events.truncate(events);
            self.pending_effects.truncate(effects);
            self.pending_conflict_keys.rollback_to(conflict_keys);
            self.pending_changed_keys.rollback_to(changed_keys);
            if let Err(error) = self.open_transaction()?.execute_batch(
                "ROLLBACK TO postproject_media_aggregate; RELEASE postproject_media_aggregate",
            ) {
                self.abort_failed_commit()?;
                return Err(sqlite_error("roll back media aggregate")(error));
            }
            return result;
        }
        if let Err(error) = self
            .open_transaction()?
            .execute_batch("RELEASE postproject_media_aggregate")
        {
            self.abort_failed_commit()?;
            return Err(sqlite_error("release media aggregate savepoint")(error));
        }
        result
    }
}
