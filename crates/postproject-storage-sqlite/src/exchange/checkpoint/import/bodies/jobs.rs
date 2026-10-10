use postproject_protocol::{Document, JobHeader, JobInput};

use crate::{ExchangeResult, exchange::records::JobApply};

use super::Bodies;

impl Bodies<'_, '_> {
    pub(super) fn job(&mut self, document: &Document) -> ExchangeResult<bool> {
        if let Some(job) = self.job.as_mut() {
            if job.input(self.transaction, JobInput::from_document(document)?)? {
                self.job = None;
            }
            return Ok(false);
        }
        self.job =
            JobApply::begin_checkpoint(self.transaction, JobHeader::from_document(document)?)?;
        Ok(true)
    }
}
