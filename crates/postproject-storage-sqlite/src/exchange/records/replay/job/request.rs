use postproject_protocol::JobHeader;
use rusqlite::{Transaction, params};

use crate::{
    ExchangeResult, sqlite_error,
    transaction::{asset_exists, encode_representation_kind},
};

use super::invalid;

pub(super) fn insert(
    transaction: &Transaction<'_>,
    header: &JobHeader,
    require_root: bool,
) -> ExchangeResult<()> {
    let output = header.requested_output();
    let duplicate: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM jobs WHERE id = ?1)",
            [header.id().as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check unique original job"))?;
    if duplicate || !asset_exists(transaction, output.asset_id())? {
        return Err(invalid().into());
    }
    if let Some(root) = output.target_root().filter(|_| require_root) {
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_roots WHERE name = ?1)",
                [root],
                |row| row.get(0),
            )
            .map_err(sqlite_error("validate original job target root"))?;
        if !exists {
            return Err(invalid().into());
        }
    }
    transaction.execute("INSERT INTO jobs (id, kind, output_asset_id, output_representation_kind, target_root, state) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        params![header.id().as_bytes().as_slice(), header.kind().as_str(), output.asset_id().as_bytes().as_slice(), encode_representation_kind(output.representation_kind())?, output.target_root()])
        .map_err(sqlite_error("stage original job request"))?;
    Ok(())
}
