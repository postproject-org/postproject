//! Source genesis preserves header facts while creating a distinct passive file.

use postproject_core::{ErrorKind, Production, ProductionId, RevisionId};
use postproject_protocol::{Digest, Position, StoreRole};
use postproject_storage_sqlite::SqliteProduction;

#[test]
fn source_header_and_genesis_survive_reopen_without_worker_or_writer_authority() {
    let directory = tempfile::tempdir().unwrap();
    let source = SqliteProduction::create(
        directory.path().join("source.pproj"),
        Some("Exact production name".into()),
    )
    .unwrap();
    let path = directory.path().join("mirror.pproj");
    let anchor = source.exchange_floor().unwrap();
    let mut mirror =
        SqliteProduction::create_genesis_mirror(&path, source.production(), anchor).unwrap();
    assert_eq!(mirror.production().id(), source.production().id());
    assert_eq!(
        mirror.production().created_at(),
        source.production().created_at()
    );
    assert_eq!(
        mirror.production().display_name(),
        source.production().display_name()
    );
    assert_eq!(mirror.exchange_role(), StoreRole::PassiveMirror);
    assert_eq!(mirror.exchange_head().unwrap(), anchor);
    assert_eq!(
        mirror.begin_transaction().err().unwrap().kind(),
        ErrorKind::Unsupported
    );
    drop(mirror);
    let mut mirror = SqliteProduction::open(&path).unwrap();
    assert_eq!(
        mirror.exchange_scope().unwrap(),
        source.exchange_scope().unwrap()
    );
    assert_eq!(mirror.exchange_floor().unwrap(), anchor);
    let view = mirror.read_session().unwrap();
    let base = view.decision_base();
    drop(view);
    assert_eq!(
        mirror.begin_edit(base).err().unwrap().kind(),
        ErrorKind::Unsupported
    );
    assert!(SqliteProduction::create_genesis_mirror(&path, source.production(), anchor).is_err());
}

#[test]
fn invalid_scopes_anchors_and_nonempty_bases_reject_before_reserving_a_path() {
    let directory = tempfile::tempdir().unwrap();
    let source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let path = directory.path().join("mirror.pproj");
    let anchor = source.exchange_floor().unwrap();
    let wrong_header = Production::new(
        ProductionId::new(),
        source.production().schema_version(),
        source.production().created_at(),
        None,
    );
    assert!(SqliteProduction::create_genesis_mirror(&path, &wrong_header, anchor).is_err());
    let altered = Position::new(anchor.scope(), None, 0, Digest::from_bytes([0; 32])).unwrap();
    assert!(SqliteProduction::create_genesis_mirror(&path, source.production(), altered).is_err());
    let later = Position::new(anchor.scope(), Some(RevisionId::new()), 1, anchor.digest()).unwrap();
    assert!(SqliteProduction::create_genesis_mirror(&path, source.production(), later).is_err());
    assert!(!path.exists());
}
