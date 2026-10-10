use postproject_core::RevisionEventKind;
use postproject_protocol::{Document, JobHeader, JobTransition};
use rusqlite::Connection;

use crate::{ExchangeResult, exchange::checkpoint::import::job_state};

use super::RetainedEffects;

impl RetainedEffects {
    pub(super) fn job_effect(
        &mut self,
        connection: &Connection,
        document: &Document,
    ) -> ExchangeResult<bool> {
        let observation = match document.kind()? {
            "job.header" => {
                let header = JobHeader::from_document(document)?;
                let request = job_state::Request::begin(connection, header.clone(), self.floor)?;
                if header.input_count() != 0 {
                    self.job = Some(request);
                }
                RevisionEventKind::JobRequested {
                    job_id: header.id(),
                }
            }
            "job.transition" => {
                let change = JobTransition::from_document(document)?;
                job_state::changed(
                    connection,
                    &change,
                    self.revision,
                    self.sequence,
                    self.floor,
                )?;
                change.observation()
            }
            _ => return Ok(false),
        };
        self.expect_observation(connection, &observation)?;
        Ok(true)
    }
}
