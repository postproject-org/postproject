use postproject_protocol::IdentifierChange;
use rusqlite::{Transaction, params};

use super::{effects::invalid, facts::structural};
use crate::{
    ExchangeResult, encode_identifier_target,
    transaction::{ensure_metadata_target_exists, mutation_error},
};

pub(super) fn apply(
    transaction: &Transaction<'_>,
    change: &IdentifierChange,
) -> ExchangeResult<()> {
    let attachment = change.attachment();
    let target = attachment.target();
    let (kind, id) = encode_identifier_target(&target)?;
    let identifier = attachment.identifier();
    structural(ensure_metadata_target_exists(transaction, kind, id))?;
    let statement = match change {
        IdentifierChange::Added(_) => {
            "INSERT INTO external_identifiers (target_kind, target_id, scheme, value, qualifier) VALUES (?1, ?2, ?3, ?4, ?5)"
        }
        IdentifierChange::Removed(_) => {
            "DELETE FROM external_identifiers WHERE target_kind = ?1 AND target_id = ?2 AND scheme = ?3 AND value = ?4 AND qualifier IS ?5"
        }
    };
    let changed = structural(
        transaction
            .execute(
                statement,
                params![
                    kind,
                    id.as_slice(),
                    identifier.scheme().as_str(),
                    identifier.value(),
                    identifier.qualifier()
                ],
            )
            .map_err(mutation_error("stage original identifier transition")),
    )?;
    if changed != 1 {
        return Err(invalid().into());
    }
    Ok(())
}
