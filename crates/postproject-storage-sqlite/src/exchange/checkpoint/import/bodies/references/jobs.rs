use postproject_core::{JobId, JobState, RevisionEventType};
use rusqlite::{OptionalExtension, params};

use crate::{
    ExchangeResult,
    exchange::{job_capture, records::validate_job_completion},
    sqlite_error,
};

use super::{Bodies, invalid};

impl Bodies<'_, '_> {
    pub(super) fn validate_jobs(&self) -> ExchangeResult<()> {
        let mut statement = self
            .transaction
            .prepare("SELECT id FROM jobs ORDER BY id")
            .map_err(sqlite_error("prepare current job observations"))?;
        let mut rows = statement
            .query([])
            .map_err(sqlite_error("query current job observations"))?;
        while let Some(row) = rows
            .next()
            .map_err(sqlite_error("read current job observation"))?
        {
            let id = JobId::from_bytes(crate::id_bytes(
                row.get(0)
                    .map_err(sqlite_error("read current job identity"))?,
                "job identity",
            )?);
            self.validate_job(id)?;
        }
        Ok(())
    }

    fn validate_job(&self, job: JobId) -> ExchangeResult<()> {
        let kinds = [
            RevisionEventType::JobRequested,
            RevisionEventType::JobClaimed,
            RevisionEventType::JobClaimRenewed,
            RevisionEventType::JobClaimReleased,
            RevisionEventType::JobSucceeded,
            RevisionEventType::JobFailed,
            RevisionEventType::JobCancelled,
        ]
        .map(crate::stored_revision_event_kind)
        .into_iter()
        .collect::<postproject_core::Result<Vec<_>>>()?;
        let header = job_capture::header(self.transaction, job)?;
        let latest: Option<(i64, Vec<u8>)> = self.transaction.query_row("SELECT e.kind, e.revision_id FROM revision_events e JOIN revisions r ON r.id = e.revision_id WHERE e.primary_id = ?1 AND e.kind IN (?2, ?3, ?4, ?5, ?6, ?7, ?8) ORDER BY r.sequence DESC, e.position DESC LIMIT 1", params![job.as_bytes().as_slice(), kinds[0], kinds[1], kinds[2], kinds[3], kinds[4], kinds[5], kinds[6]], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional().map_err(sqlite_error("read latest original job observation"))?;
        let repeated: bool = self
            .transaction
            .query_row(
                "SELECT COUNT(*) > 1 FROM revision_events WHERE kind = ?1 AND primary_id = ?2",
                params![kinds[0], job.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(sqlite_error("validate immutable job request observation"))?;
        if repeated {
            return Err(invalid().into());
        }
        let Some((kind, revision)) = latest else {
            return if self.manifest.floor().sequence() == 0 {
                Err(invalid().into())
            } else {
                Ok(())
            };
        };
        let compatible = match header.state() {
            JobState::Requested => kind == kinds[0] || kind == kinds[3],
            JobState::Claimed(_) => kind == kinds[1] || kind == kinds[2],
            JobState::Succeeded(_) => kind == kinds[4],
            JobState::Failed(_) => kind == kinds[5],
            JobState::Cancelled => kind == kinds[6],
            _ => false,
        };
        if !compatible {
            return Err(invalid().into());
        }
        if let JobState::Succeeded(completion) = header.state() {
            let revision = postproject_core::RevisionId::from_bytes(crate::id_bytes(
                revision,
                "job completion revision",
            )?);
            validate_job_completion(self.transaction, revision, &header, completion)?;
        }
        Ok(())
    }
}
