//! Runs the coherent read/edit recipe against a real SQLite production.

use std::path::Path;

use postproject_core::{ProductionReadSession, QueryPageRequest, Result};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;

// [coherent-reads]
fn read_then_edit(production: &mut SqliteProduction, media: &Path) -> Result<()> {
    let empty = production.read_session()?;
    let prepared = prepare_original_media(media, None, None)?;
    let asset_id = prepared.asset().id();
    let receipt = {
        let mut edit = empty.edit(production)?;
        edit.import_original(&prepared)?;
        edit.commit_with_receipt()?
    };
    assert_eq!(receipt.revision().expect("import revision").sequence(), 1);
    assert_eq!(
        empty
            .read()
            .assets_page(&QueryPageRequest::new(10, None)?)?
            .items(),
        []
    );
    drop(empty);

    let view = production.read_session()?;
    let revision_id = receipt.revision().expect("import revision").id();
    let events = view
        .read()
        .events_for_revision_page(revision_id, &QueryPageRequest::new(1, None)?)?;
    let continuation = view.read().events_for_revision_page(
        revision_id,
        &QueryPageRequest::new(1000, events.next_cursor().cloned())?,
    )?;
    assert_eq!(events.items().len(), 1);
    assert!(events.next_cursor().is_some());
    assert!(continuation.next_cursor().is_none());
    assert!(continuation.items()[0].position() > events.items()[0].position());
    assert_eq!(
        view.read().plan_regeneration(&[])?,
        [] as [postproject_core::RegenerationJobPlan; 0]
    );
    let copied = view.read().asset(asset_id)?;
    let page = view
        .read()
        .representations_page(asset_id, &QueryPageRequest::new(10, None)?)?;
    let representation = view.read().representation(page.items()[0].id())?;
    let base = view.decision_base();
    drop(view);
    let mut edit = production.begin_edit(base)?;
    assert!(edit.commit_with_receipt()?.revision().is_none());
    assert_eq!(copied.id(), asset_id);
    assert_eq!(representation.asset_id(), asset_id);
    Ok(())
}
// [/coherent-reads]

#[test]
fn coherent_read_example_runs() -> Result<()> {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("views.pproj"), None)?;
    let media =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/fixtures/sample-media.dat");
    read_then_edit(&mut production, &media)
}
