//! Root pages read fresh live facts or a retained view, with scoped cursors.

use postproject_core::{ErrorKind, MediaRoot, MediaRootId, QueryPageRequest};
use postproject_storage_sqlite::SqliteProduction;

fn root(id: u8, priority: i32) -> MediaRoot {
    MediaRoot::new(
        MediaRootId::from_bytes([id; 16]),
        format!("root-{id}"),
        None,
        None,
        priority,
        true,
    )
    .unwrap()
}

#[test]
fn open_views_and_edits_do_not_decode_unrequested_roots() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bounded.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let mut transaction = production.begin_transaction().unwrap();
    for value in [root(1, 0), root(2, 0), root(3, 0)] {
        transaction.add_media_root(value).unwrap();
    }
    transaction.commit().unwrap();
    drop(transaction);
    drop(production);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE media_roots SET name = 'invalid/name' WHERE id = ?1",
            [root(3, 0).id().as_bytes().as_slice()],
        )
        .unwrap();
    let mut production = SqliteProduction::open(&path).unwrap();
    let view = production.read_session().unwrap();
    let first = view
        .read()
        .media_roots_page(&QueryPageRequest::new(1, None).unwrap())
        .unwrap();
    assert_eq!(first.items()[0].id(), root(1, 0).id());
    let mut edit = production.begin_edit(view.decision_base()).unwrap();
    edit.set_media_root_enabled(root(1, 0).id(), false).unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert_eq!(
        view.read()
            .media_roots_page(&QueryPageRequest::new(1, first.next_cursor().cloned()).unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::Storage
    );
    assert!(
        !production
            .media_roots_page(&QueryPageRequest::new(1, None).unwrap())
            .unwrap()
            .items()[0]
            .is_enabled()
    );
}

#[test]
fn convenience_reads_fail_above_the_cap_while_pages_remain_available() {
    let directory = tempfile::tempdir().unwrap();
    let mut production =
        SqliteProduction::create(directory.path().join("many.pproj"), None).unwrap();
    let mut transaction = production.begin_transaction().unwrap();
    for index in 1_u128..=1001 {
        transaction
            .add_media_root(
                MediaRoot::new(
                    MediaRootId::from_bytes(index.to_be_bytes()),
                    format!("root-{index}"),
                    None,
                    None,
                    0,
                    true,
                )
                .unwrap(),
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(transaction);
    assert_eq!(
        production.media_roots().unwrap_err().kind(),
        ErrorKind::Unsupported
    );
    let page = production
        .media_roots_page(&QueryPageRequest::new(1000, None).unwrap())
        .unwrap();
    assert_eq!(page.items().len(), 1000);
    let last = production
        .media_roots_page(&QueryPageRequest::new(1000, page.next_cursor().cloned()).unwrap())
        .unwrap();
    assert_eq!(last.items().len(), 1);
    assert!(last.next_cursor().is_none());
}

#[test]
fn root_pages_observe_other_writers_and_retained_pages_stay_coherent() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("production.pproj");
    let mut writer = SqliteProduction::create(&path, None).unwrap();
    let reader = SqliteProduction::open(&path).unwrap();
    let empty = reader.read_session().unwrap();
    let first = QueryPageRequest::new(1, None).unwrap();
    assert_eq!(reader.media_roots_page(&first).unwrap().items(), []);
    {
        let mut transaction = writer.begin_transaction().unwrap();
        for value in [root(3, 1), root(1, -2), root(2, 1)] {
            transaction.add_media_root(value).unwrap();
        }
        transaction.commit().unwrap();
    }
    assert_eq!(empty.read().media_roots_page(&first).unwrap().items(), []);
    let live = reader.media_roots_page(&first).unwrap();
    assert_eq!(live.items()[0].id(), root(1, -2).id());
    assert!(live.next_cursor().is_some());
    let view = reader.read_session().unwrap();
    let pinned = view.read().media_roots_page(&first).unwrap();
    let continuation = QueryPageRequest::new(1, pinned.next_cursor().cloned()).unwrap();
    {
        let base = writer.read_session().unwrap().decision_base();
        let mut transaction = writer.begin_edit(base).unwrap();
        transaction
            .set_media_root_enabled(root(2, 1).id(), false)
            .unwrap();
        transaction.add_media_root(root(4, 0)).unwrap();
        transaction.commit().unwrap();
    }
    let second = view.read().media_roots_page(&continuation).unwrap();
    assert_eq!(second.items()[0].id(), root(2, 1).id());
    assert!(second.items()[0].is_enabled());
    let last = view
        .read()
        .media_roots_page(&QueryPageRequest::new(1, second.next_cursor().cloned()).unwrap())
        .unwrap();
    assert_eq!(last.items()[0].id(), root(3, 1).id());
    assert!(last.next_cursor().is_none());
    let refreshed = reader
        .media_roots_page(&QueryPageRequest::new(10, None).unwrap())
        .unwrap();
    assert_eq!(
        refreshed
            .items()
            .iter()
            .map(MediaRoot::id)
            .collect::<Vec<_>>(),
        [
            root(1, -2).id(),
            root(4, 0).id(),
            root(2, 1).id(),
            root(3, 1).id()
        ]
    );
    assert!(!refreshed.items()[2].is_enabled());
    assert_eq!(
        reader.media_roots_page(&continuation).unwrap_err().kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        view.read().assets_page(&continuation).unwrap_err().kind(),
        ErrorKind::InvalidArgument
    );
    let another = reader.read_session().unwrap();
    assert_eq!(
        another
            .read()
            .media_roots_page(&continuation)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidArgument
    );
    let foreign = SqliteProduction::create(directory.path().join("other.pproj"), None).unwrap();
    let live_continuation = QueryPageRequest::new(1, live.next_cursor().cloned()).unwrap();
    assert_eq!(
        foreign
            .media_roots_page(&live_continuation)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidArgument
    );
}
