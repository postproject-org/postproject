//! Empty passive initialization, before applying the source's first record.

use std::path::Path;

use postproject_core::Production;
use postproject_protocol::{FailureKind, Position, ProtocolError};
use rusqlite::params;

use crate::{
    CURRENT_SCHEMA_VERSION, ExchangeResult, SqliteProduction, migrations, open_connection,
    reserve_new_file, sqlite_error,
};

pub(crate) fn create(
    path: &Path,
    source: &Production,
    anchor: Position,
) -> ExchangeResult<SqliteProduction> {
    if source.id() != anchor.scope().production() {
        return Err(ProtocolError::new(
            FailureKind::ScopeMismatch,
            "mirror header and source scope differ",
        )
        .into());
    }
    if anchor.sequence() != 0 || anchor.revision().is_some() {
        return Err(ProtocolError::new(
            FailureKind::InvalidBase,
            "empty mirror initialization requires genesis",
        )
        .into());
    }
    if Position::anchor(anchor.decision_base())? != anchor {
        return Err(
            ProtocolError::new(FailureKind::Integrity, "mirror genesis anchor mismatch").into(),
        );
    }
    reserve_new_file(path)?;
    let mut connection = open_connection(path)?;
    migrations::migrate(&mut connection)?;
    let header = Production::new(
        source.id(),
        CURRENT_SCHEMA_VERSION,
        source.created_at(),
        source.display_name().map(str::to_owned),
    );
    let transaction = connection
        .transaction()
        .map_err(sqlite_error("begin empty passive initialization"))?;
    transaction.execute("INSERT INTO productions (singleton, id, schema_version, created_at_micros, display_name) VALUES (1, ?1, ?2, ?3, ?4)", params![header.id().as_bytes().as_slice(), CURRENT_SCHEMA_VERSION, header.created_at().as_unix_micros(), header.display_name()])
        .map_err(sqlite_error("persist source production header"))?;
    transaction.execute("UPDATE exchange_history SET history_id = ?1, role = 2, floor_revision_id = NULL, floor_sequence = 0, anchor_digest = ?2 WHERE singleton = 1", params![anchor.scope().history().as_bytes().as_slice(), anchor.digest().as_bytes().as_slice()])
        .map_err(sqlite_error("persist passive source genesis"))?;
    transaction
        .commit()
        .map_err(sqlite_error("commit empty passive initialization"))?;
    Ok(SqliteProduction::from_parts(
        path.to_path_buf(),
        connection,
        header,
    )?)
}
