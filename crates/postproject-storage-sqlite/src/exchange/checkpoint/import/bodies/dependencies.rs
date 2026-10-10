use postproject_protocol::{DependencySetHeader, Document};

use crate::{ExchangeResult, exchange::records::DependencyApply};

use super::{super::super::invalid, Bodies};

impl Bodies<'_, '_> {
    pub(super) fn dependency(&mut self, document: &Document) -> ExchangeResult<bool> {
        if let Some(dependency) = self.dependency.as_mut() {
            if dependency.push(self.transaction, document)? {
                self.dependency = None;
            }
            return Ok(false);
        }
        let header = DependencySetHeader::from_document(document)?;
        let duplicate: bool = self
            .transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM dependency_sets WHERE source_representation_id = ?1)",
                [header.source_representation_id().as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(crate::sqlite_error(
                "check unique checkpoint dependency set",
            ))?;
        if duplicate {
            return Err(invalid().into());
        }
        let state = DependencyApply::begin_checkpoint(
            self.transaction,
            self.manifest.head().sequence(),
            header,
        )?;
        if header.occurrence_count() != 0 {
            self.dependency = Some(state);
        }
        Ok(true)
    }
}
