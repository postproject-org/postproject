//! Coherent point reads and pages with deterministic intervening writes (T04).

use postproject_core::{MediaRoot, MediaRootId, QueryPageRequest};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;

#[test]
fn view_is_pinned_before_return_and_writer_commits_without_reader_release() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("view.pproj");
    let mut writer = SqliteProduction::create(&path, None).unwrap();
    let empty = writer.read_session().unwrap();
    assert_eq!(
        empty.decision_base().production_id(),
        writer.production().id()
    );
    assert_eq!(empty.decision_base().revision_id(), None);
    assert_eq!(empty.decision_base().sequence(), 0);
    {
        let mut edit = writer.begin_transaction().unwrap();
        edit.add_media_root(
            MediaRoot::new(MediaRootId::new(), "rushes", None, None, 0, true).unwrap(),
        )
        .unwrap();
        edit.commit().unwrap();
    }
    assert!(empty.read().production().media_roots().is_empty());
    assert!(empty.read().latest_revision().unwrap().is_none());
    let current = writer.read_session().unwrap();
    assert_eq!(current.read().production().media_roots().len(), 1);
    assert_eq!(current.decision_base().sequence(), 1);
    assert_eq!(
        current.decision_base().revision_id(),
        current.read().latest_revision().unwrap().map(|r| r.id())
    );
    // The independently owned session remains usable after the writer closes.
    drop(writer);
    assert_eq!(
        current.read().production().media_roots()[0].name(),
        "rushes"
    );
}

#[test]
fn pages_and_point_reads_use_one_view_and_drop_releases_checkpoint() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pages.pproj");
    let media_path = directory.path().join("media.dat");
    std::fs::write(&media_path, b"sample media").unwrap();
    let mut writer = SqliteProduction::create(&path, None).unwrap();
    let mut ids = Vec::new();
    for _ in 0..2 {
        let media = prepare_original_media(&media_path, None, None).unwrap();
        ids.push(media.asset().id());
        let mut edit = writer.begin_transaction().unwrap();
        edit.import_original(&media).unwrap();
        edit.commit().unwrap();
    }
    let view = writer.read_session().unwrap();
    let first = view
        .read()
        .assets_page(&QueryPageRequest::new(1, None).unwrap())
        .unwrap();
    let copied = view.read().asset(ids[0]).unwrap();
    let newer = prepare_original_media(&media_path, None, None).unwrap();
    {
        let mut edit = writer.begin_transaction().unwrap();
        edit.import_original(&newer).unwrap();
        edit.commit().unwrap();
    }
    let next = view
        .read()
        .assets_page(&QueryPageRequest::new(1, first.next_cursor().cloned()).unwrap())
        .unwrap();
    assert_eq!(first.items().len(), 1);
    assert_eq!(next.items().len(), 1);
    assert!(next.next_cursor().is_none());
    assert!(view.read().asset(newer.asset().id()).is_err());
    assert_eq!(
        view.read().latest_revision().unwrap().unwrap().sequence(),
        2
    );
    assert_eq!(writer.latest_revision().unwrap().unwrap().sequence(), 3);
    drop(view);
    assert_eq!(copied.id(), ids[0]);
    let connection = rusqlite::Connection::open(&path).unwrap();
    let (busy, _, _): (i64, i64, i64) = connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap();
    assert_eq!(busy, 0);
    let fresh = writer.read_session().unwrap();
    assert!(fresh.read().asset(newer.asset().id()).is_ok());
}
