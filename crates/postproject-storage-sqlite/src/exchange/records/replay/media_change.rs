use postproject_protocol::MediaChange;
use rusqlite::{Transaction, params};

use super::{effects::invalid, facts::structural};
use crate::{
    ExchangeResult,
    transaction::{ensure_metadata_target_exists, mutation_error, persist_locator},
};

pub(super) fn apply(transaction: &Transaction<'_>, change: &MediaChange) -> ExchangeResult<()> {
    let changed = match change {
        MediaChange::ResourceFileFacts { resource_id, facts } => {
            let size = i64::try_from(facts.size_bytes()).map_err(|_| postproject_protocol::ProtocolError::new(postproject_protocol::FailureKind::Unsupported, "file size exceeds this receiver's storage range"))?;
            let modified = facts.modified_at().map(postproject_core::Timestamp::as_unix_micros);
            structural(transaction.execute(
                "UPDATE resources SET file_size_bytes = ?2, modified_at_micros = ?3 WHERE id = ?1 AND (file_size_bytes IS NOT ?2 OR modified_at_micros IS NOT ?3)",
                params![resource_id.as_bytes().as_slice(), size, modified],
            ).map_err(mutation_error("stage replayed file facts")))?
        }
        MediaChange::RootAdded(root) => structural(transaction.execute(
            "INSERT INTO media_roots (id, name, label, legacy_uri, priority, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![root.id().as_bytes().as_slice(), root.name(), root.label(), root.legacy_uri(), root.priority(), root.is_enabled()],
        ).map_err(mutation_error("stage replayed root")))?,
        MediaChange::RootEnabled { root_id, enabled } => structural(transaction.execute(
            "UPDATE media_roots SET enabled = ?1 WHERE id = ?2 AND enabled <> ?1",
            params![enabled, root_id.as_bytes().as_slice()],
        ).map_err(mutation_error("stage replayed root state")))?,
        MediaChange::RootRemoved(root_id) => structural(transaction.execute(
            "DELETE FROM media_roots WHERE id = ?1", [root_id.as_bytes().as_slice()],
        ).map_err(mutation_error("stage replayed root removal")))?,
        MediaChange::LocatorAdded(locator) => {
            structural(ensure_metadata_target_exists(transaction, 3, locator.resource_id().as_bytes()))?;
            structural(persist_locator(transaction, locator))?;
            1
        }
        MediaChange::LocatorRetired { locator_id, resource_id } => structural(transaction.execute(
            "DELETE FROM locators WHERE id = ?1 AND resource_id = ?2",
            params![locator_id.as_bytes().as_slice(), resource_id.as_bytes().as_slice()],
        ).map_err(mutation_error("stage replayed locator retirement")))?,
    };
    if changed != 1 {
        return Err(invalid().into());
    }
    Ok(())
}
