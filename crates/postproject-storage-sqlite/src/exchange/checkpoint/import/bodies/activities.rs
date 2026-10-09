use postproject_protocol::{ActivityHeader, Document};

use crate::{ExchangeResult, exchange::records::ActivityApply};

use super::Bodies;

impl Bodies<'_, '_> {
    pub(super) fn activity(&mut self, document: &Document) -> ExchangeResult<bool> {
        if let Some(activity) = self.activity.as_mut() {
            if activity.push_checkpoint(
                self.transaction,
                self.manifest.head().sequence(),
                document,
            )? {
                self.activity = None;
            }
            return Ok(false);
        }
        let header = ActivityHeader::from_document(document)?;
        let duplicate: bool = self
            .transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM activities WHERE id = ?1)",
                [header.id().as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(crate::sqlite_error("validate unique checkpoint activity"))?;
        if duplicate {
            return Err(super::super::super::invalid().into());
        }
        self.activity = Some(ActivityApply::begin_checkpoint(self.transaction, header)?);
        Ok(true)
    }
}
