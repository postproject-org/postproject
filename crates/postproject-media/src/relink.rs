//! Stage copied relink decisions without repeating candidate construction.

use postproject_core::{ProductionStore, ProductionStoreTransaction, RelinkSelection, Result};

/// Opens an edit at the selection's original base and stages all its locations.
///
/// Locator values are prepared before beginning the write. The returned edit
/// still requires an explicit commit; callers may compose other mutations and
/// attribution. A staging failure rolls back the owned edit. Files are not
/// reverified, and the host remains responsible for association/generation checks.
///
/// # Errors
/// Rejects wrong production scope, invalid bases or locators, and storage errors.
pub fn stage_relink<'store, Store: ProductionStore>(
    store: &'store mut Store,
    selection: &RelinkSelection,
) -> Result<Store::Transaction<'store>> {
    let locators = selection
        .locations()
        .iter()
        .map(|(resource, candidate)| {
            crate::prepare_confirmed_locator(
                *resource,
                candidate.uri(),
                candidate.media_root(),
                candidate.sequence_naming().cloned(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let mut edit = store.begin_edit(selection.base())?;
    for locator in &locators {
        if let Err(error) = edit.confirm_locator(locator) {
            edit.rollback()?;
            return Err(error);
        }
    }
    Ok(edit)
}
