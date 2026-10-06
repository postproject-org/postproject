//! Bounded root-page recipe using the domain read seam.

use postproject_core::{MediaRoot, MediaRootId, QueryPageRequest, Result};
use postproject_storage_sqlite::SqliteProduction;

// [root-pages]
fn root_pages(production: &mut SqliteProduction) -> Result<()> {
    {
        let mut transaction = production.begin_transaction()?;
        transaction.add_media_root(MediaRoot::new(
            MediaRootId::new(),
            "first",
            None,
            None,
            -1,
            true,
        )?)?;
        transaction.add_media_root(MediaRoot::new(
            MediaRootId::new(),
            "second",
            None,
            None,
            0,
            true,
        )?)?;
        transaction.commit()?;
    }
    let live = production.media_roots_page(&QueryPageRequest::new(1, None)?)?;
    assert!(live.next_cursor().is_some());
    let view = production.read_session()?;
    let first = view
        .read()
        .media_roots_page(&QueryPageRequest::new(1, None)?)?;
    let last = view
        .read()
        .media_roots_page(&QueryPageRequest::new(1, first.next_cursor().cloned())?)?;
    assert_eq!(first.items()[0].name(), "first");
    assert_eq!(last.items()[0].name(), "second");
    assert!(last.next_cursor().is_none());
    assert_eq!(view.read().media_roots()?.len(), 2);
    Ok(())
}
// [/root-pages]

#[test]
fn root_page_example_runs() -> Result<()> {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("roots.pproj"), None)?;
    root_pages(&mut production)
}
