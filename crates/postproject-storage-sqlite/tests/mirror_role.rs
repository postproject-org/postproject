//! Synthetic persisted-role checks; these are not checkpoint acceptance.

use postproject_core::{ErrorKind, JobClaimId, JobId, QueryPageRequest};
use postproject_protocol::StoreRole;
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;

#[test]
fn persisted_passive_role_rejects_every_native_write_open_but_retains_reads() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("passive.pproj");
    let production = SqliteProduction::create(&path, None).unwrap();
    let base = production.read_session().unwrap().decision_base();
    drop(production);
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE exchange_history SET role = 2 WHERE singleton = 1",
            [],
        )
        .unwrap();
    drop(connection);
    let mut mirror = SqliteProduction::open(&path).unwrap();
    assert_eq!(mirror.exchange_role(), StoreRole::PassiveMirror);
    assert!(mirror.is_read_only());
    assert_eq!(
        mirror.begin_transaction().err().unwrap().kind(),
        ErrorKind::Unsupported
    );
    assert_eq!(
        mirror.begin_edit(base).err().unwrap().kind(),
        ErrorKind::Unsupported
    );
    let token = format!(
        "ppl1:{}:{}:{}",
        base.production_id(),
        JobId::new(),
        JobClaimId::new()
    );
    assert_eq!(
        mirror.import_job_lease(&token).unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    assert!(
        mirror
            .assets_page(&QueryPageRequest::new(10, None).unwrap())
            .unwrap()
            .items()
            .is_empty()
    );
    let scope = mirror.exchange_scope().unwrap();
    let view = mirror.read_session().unwrap().into_read_only();
    assert_eq!(view.exchange_scope().unwrap(), scope);
    assert_eq!(view.exchange_role(), StoreRole::PassiveMirror);
    drop(view);
    drop(mirror);
    assert_eq!(
        SqliteProduction::open(&path).unwrap().exchange_role(),
        StoreRole::PassiveMirror
    );
}

#[test]
fn writer_rechecks_persisted_role_under_its_writer_lock() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("role-guard.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE exchange_history SET role = 2 WHERE singleton = 1",
            [],
        )
        .unwrap();
    drop(connection);
    // This simulates external file tampering; production roles cannot be
    // changed through ordinary APIs. A stale cached facade must still reject.
    assert_eq!(
        production.begin_transaction().err().unwrap().kind(),
        ErrorKind::Unsupported
    );
}
