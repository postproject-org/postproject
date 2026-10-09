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
        self.activity = Some(ActivityApply::begin_checkpoint(
            self.transaction,
            ActivityHeader::from_document(document)?,
        )?);
        Ok(true)
    }
}
