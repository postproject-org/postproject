//! Coherent point reads and pages with deterministic intervening writes (T04).

use postproject_core::{
    DecisionBase, ErrorKind, MediaRoot, MediaRootId, ProductionReadSession, QueryPageRequest,
    RevisionEventFilter, RevisionEventType, RevisionId,
};
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
    assert!(empty.read().media_roots().unwrap().is_empty());
    assert!(empty.read().latest_revision().unwrap().is_none());
    let added = RevisionEventFilter::new([RevisionEventType::MediaRootAdded]).unwrap();
    let page = empty.read().changes_since_filtered(0, &added, 10).unwrap();
    assert!(page.revisions().is_empty());
    assert_eq!(page.through_sequence(), 0);
    let current = writer.read_session().unwrap();
    assert_eq!(current.read().media_roots().unwrap().len(), 1);
    assert_eq!(current.decision_base().sequence(), 1);
    assert_eq!(
        current.decision_base().revision_id(),
        current.read().latest_revision().unwrap().map(|r| r.id())
    );
    {
        let mut edit = writer.begin_transaction().unwrap();
        edit.add_media_root(
            MediaRoot::new(MediaRootId::new(), "proxies", None, None, 0, true).unwrap(),
        )
        .unwrap();
        edit.commit().unwrap();
    }
    let page = current
        .read()
        .changes_since_filtered(0, &added, 10)
        .unwrap();
    assert_eq!(page.revisions().len(), 1);
    assert_eq!(page.through_sequence(), 1);
    let removed = RevisionEventFilter::new([RevisionEventType::MediaRootRemoved]).unwrap();
    let unmatched = current
        .read()
        .changes_since_filtered(0, &removed, 10)
        .unwrap();
    assert!(unmatched.revisions().is_empty());
    assert_eq!(unmatched.through_sequence(), 1);
    assert_eq!(
        writer
            .changes_since_filtered(0, &added, 10)
            .unwrap()
            .through_sequence(),
        2
    );
    // The independently owned session remains usable after the writer closes.
    drop(writer);
    assert_eq!(current.read().media_roots().unwrap()[0].name(), "rushes");
}

#[test]
fn revision_event_pages_retain_order_revision_filter_and_view_scope() {
    let directory = tempfile::tempdir().unwrap();
    let media_path = directory.path().join("media.dat");
    std::fs::write(&media_path, b"media").unwrap();
    let mut writer = SqliteProduction::create(directory.path().join("events.pproj"), None).unwrap();
    let empty = writer.read_session().unwrap();
    let media = prepare_original_media(&media_path, None, None).unwrap();
    {
        let mut edit = writer.begin_transaction().unwrap();
        edit.import_original(&media).unwrap();
        edit.commit().unwrap();
    }
    let first = writer.latest_revision().unwrap().unwrap();
    let view = writer.read_session().unwrap();
    let page = view
        .read()
        .events_for_revision_page(first.id(), &QueryPageRequest::new(1, None).unwrap())
        .unwrap();
    assert_eq!(page.items().len(), 1);
    let cursor = page.next_cursor().unwrap().clone();
    assert_eq!(
        empty
            .read()
            .events_for_revision_page(first.id(), &QueryPageRequest::new(1, None).unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::NotFound
    );
    {
        let mut edit = writer.begin_transaction().unwrap();
        edit.add_media_root(
            MediaRoot::new(MediaRootId::new(), "later", None, None, 0, true).unwrap(),
        )
        .unwrap();
        edit.commit().unwrap();
    }
    let later = writer.latest_revision().unwrap().unwrap();
    let fresh = writer.read_session().unwrap();
    let continuation = QueryPageRequest::new(1000, Some(cursor)).unwrap();
    assert_eq!(
        view.read()
            .events_for_revision_page(later.id(), &continuation)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        fresh
            .read()
            .events_for_revision_page(first.id(), &continuation)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        writer
            .events_for_revision_page(first.id(), &continuation)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidArgument
    );
    let next = view
        .read()
        .events_for_revision_page(first.id(), &continuation)
        .unwrap();
    let mut events = page.into_items();
    events.extend(next.into_items());
    assert_eq!(events, writer.events_for_revision(first.id()).unwrap());
    assert_eq!(
        view.read()
            .events_for_revision_page(later.id(), &QueryPageRequest::new(1, None).unwrap())
            .unwrap_err()
            .kind(),
        ErrorKind::NotFound
    );
    drop(writer);
    assert_eq!(events[0].revision_id(), first.id());
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

#[test]
fn detached_bases_check_scope_pairing_empty_conflicts_and_additive_merge() {
    let directory = tempfile::tempdir().unwrap();
    let mut writer = SqliteProduction::create(directory.path().join("edit.pproj"), None).unwrap();
    let view = writer.read_session().unwrap();
    let empty = view.decision_base();
    let root_id = MediaRootId::new();
    {
        let mut edit = view.edit(&mut writer).unwrap();
        edit.add_media_root(MediaRoot::new(root_id, "rushes", None, None, 0, true).unwrap())
            .unwrap();
        edit.commit_with_receipt().unwrap();
    }
    drop(view);
    {
        let mut stale = writer.begin_edit(empty).unwrap();
        stale.set_media_root_enabled(root_id, false).unwrap();
        let error = stale.commit_with_receipt().unwrap_err();
        let conflict = error.transaction_conflict_detail().unwrap();
        assert_eq!(conflict.base_revision(), None);
        assert_eq!(conflict.base_sequence(), 0);
        assert_eq!(conflict.superseding_sequence(), 1);
    }
    {
        let mut additive = writer.begin_edit(empty).unwrap();
        additive
            .add_media_root(
                MediaRoot::new(MediaRootId::new(), "renders", None, None, 0, true).unwrap(),
            )
            .unwrap();
        additive.commit_with_receipt().unwrap();
    }
    let current = writer.read_session().unwrap().decision_base();
    let mut other = SqliteProduction::create(directory.path().join("other.pproj"), None).unwrap();
    assert_eq!(
        other.begin_edit(current).err().unwrap().kind(),
        ErrorKind::InvalidArgument
    );
    let forged = DecisionBase::new(
        current.production_id(),
        current.revision_id(),
        current.sequence() + 1,
    )
    .unwrap();
    assert_eq!(
        writer.begin_edit(forged).err().unwrap().kind(),
        ErrorKind::InvalidArgument
    );
    let missing = DecisionBase::new(current.production_id(), Some(RevisionId::new()), 42).unwrap();
    assert_eq!(
        writer.begin_edit(missing).err().unwrap().kind(),
        ErrorKind::NotFound
    );
    assert!(DecisionBase::new(current.production_id(), None, 1).is_err());
    assert!(DecisionBase::new(current.production_id(), current.revision_id(), 0).is_err());
    // Failed begin did not retain a write lock or poison the next valid edit.
    assert!(
        writer
            .begin_edit(current)
            .unwrap()
            .commit_with_receipt()
            .unwrap()
            .revision()
            .is_none()
    );
}

#[test]
fn cursors_cannot_escape_their_production_or_pinned_view() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("first.pproj");
    let media = directory.path().join("media.dat");
    std::fs::write(&media, b"scoped cursor fixture").unwrap();
    let mut first = SqliteProduction::create(&path, None).unwrap();
    let mut other = SqliteProduction::create(directory.path().join("other.pproj"), None).unwrap();
    for _ in 0..2 {
        let prepared = prepare_original_media(&media, None, None).unwrap();
        // Identical entity IDs/facts cannot make another production's token valid.
        for production in [&mut first, &mut other] {
            let mut edit = production.begin_transaction().unwrap();
            edit.import_original(&prepared).unwrap();
            edit.commit().unwrap();
        }
    }
    let page = first
        .assets_page(&QueryPageRequest::new(1, None).unwrap())
        .unwrap();
    let live = QueryPageRequest::new(1, page.next_cursor().cloned()).unwrap();
    assert_eq!(
        other.assets_page(&live).unwrap_err().kind(),
        ErrorKind::InvalidArgument
    );
    let reopened = SqliteProduction::open(&path).unwrap();
    assert_eq!(reopened.assets_page(&live).unwrap().items().len(), 1);

    let view = first.read_session().unwrap();
    assert_eq!(
        view.read().assets_page(&live).unwrap_err().kind(),
        ErrorKind::InvalidArgument
    );
    let page = view
        .read()
        .assets_page(&QueryPageRequest::new(1, None).unwrap())
        .unwrap();
    let scoped = QueryPageRequest::new(1, page.next_cursor().cloned()).unwrap();
    assert_eq!(view.read().assets_page(&scoped).unwrap().items().len(), 1);
    assert_eq!(
        first.assets_page(&scoped).unwrap_err().kind(),
        ErrorKind::InvalidArgument
    );
    let same_revision = first.read_session().unwrap();
    assert_eq!(same_revision.decision_base(), view.decision_base());
    assert_eq!(
        same_revision
            .read()
            .assets_page(&scoped)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidArgument
    );
    drop(view);
    assert_eq!(
        same_revision
            .read()
            .assets_page(&scoped)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidArgument
    );
}

#[test]
fn adapter_facade_stays_pinned_and_rejects_write_and_live_operations() {
    let directory = tempfile::tempdir().unwrap();
    let mut writer =
        SqliteProduction::create(directory.path().join("adapter.pproj"), None).unwrap();
    let session = writer.read_session().unwrap();
    let base = session.decision_base();
    let mut reader = session.into_read_only();
    assert!(reader.is_read_only());
    let mut edit = writer.begin_edit(base).unwrap();
    edit.add_media_root(MediaRoot::new(MediaRootId::new(), "rushes", None, None, 0, true).unwrap())
        .unwrap();
    let receipt = edit.commit_with_receipt().unwrap();
    drop(edit);
    assert!(reader.latest_revision().unwrap().is_none());
    assert!(reader.media_roots().unwrap().is_empty());
    assert_eq!(
        reader.begin_transaction().err().unwrap().kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        reader.begin_edit(base).err().unwrap().kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        reader
            .begin_transaction_at(receipt.revision().unwrap().id())
            .err()
            .unwrap()
            .kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        reader.read_session().err().unwrap().kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        reader.revision_waiter().err().unwrap().kind(),
        ErrorKind::InvalidArgument
    );
    drop(writer);
    assert!(reader.latest_revision().unwrap().is_none());
}
