//! Identified local metadata proposals reuse the native staging/commit path.

use postproject_core::{CommitReceipt, Error, ErrorKind, Result};
use postproject_protocol::{Command, FailureKind, Outcome, Proposal, ProtocolError, Rejection};
use rusqlite::OptionalExtension;

use super::SqliteTransaction;
use crate::{ExchangeError, ExchangeResult, SqliteProduction, sqlite_error, stored_u64};

impl SqliteTransaction<'_> {
    pub(crate) fn submit(&mut self, proposal: &Proposal) -> ExchangeResult<Outcome> {
        self.lifecycle.ensure_open()?;
        let result = self.submit_open(proposal);
        if result.is_err() {
            self.abort_failed_commit()?;
        }
        result
    }

    fn submit_open(&mut self, proposal: &Proposal) -> ExchangeResult<Outcome> {
        // The writer lock is already held. Identity recovery precedes every
        // current-base/domain guard, including for retained terminal rejection.
        if let Some(outcome) = crate::exchange::lookup(
            self.open_transaction()?,
            proposal.scope(),
            proposal.client(),
            proposal.request(),
        )? {
            if outcome.request_digest() != proposal.digest()? {
                return Err(ProtocolError::new(
                    FailureKind::RequestIdentityMismatch,
                    "request identity has different normalized intent",
                )
                .into());
            }
            self.rollback()?;
            return Ok(outcome);
        }
        self.open_transaction()?
            .execute_batch("SAVEPOINT submission_body")
            .map_err(sqlite_error("begin submission staging"))?;
        let rejection = if self.select_submission_base(proposal)? {
            match self
                .stage_proposal(proposal)
                .and_then(|()| self.prepare_commit())
            {
                Ok(receipt) => {
                    let outcome = Outcome::accepted(proposal, receipt.clone())?;
                    self.open_transaction()?
                        .execute_batch("RELEASE submission_body")
                        .map_err(sqlite_error("finish submission staging"))?;
                    self.finish_commit(receipt, Some(&outcome))?;
                    return Ok(outcome);
                }
                Err(error) => Rejection::domain(&error).map_err(|_| ExchangeError::Store(error))?,
            }
        } else {
            Rejection::invalid_base()
        };
        self.open_transaction()?
            .execute_batch("ROLLBACK TO submission_body; RELEASE submission_body")
            .map_err(sqlite_error("discard rejected submission staging"))?;
        self.clear_pending();
        let outcome = Outcome::rejected(proposal, rejection)?;
        let receipt = CommitReceipt::new(self.production.id(), None);
        self.finish_commit(receipt, Some(&outcome))?;
        Ok(outcome)
    }

    fn select_submission_base(&mut self, proposal: &Proposal) -> Result<bool> {
        let Some(base) = proposal.base() else {
            return Ok(true);
        };
        let decision = base.decision();
        if let Some(revision) = decision.revision_id() {
            let stored = self
                .open_transaction()?
                .query_row(
                    "SELECT sequence FROM revisions WHERE id = ?1",
                    [revision.as_bytes().as_slice()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(sqlite_error("validate submission base"))?;
            let Some(stored) = stored else {
                return Ok(false);
            };
            if stored_u64(stored, "submission base sequence")? != decision.sequence() {
                return Ok(false);
            }
        }
        self.base_revision = Some((decision.revision_id(), decision.sequence()));
        Ok(true)
    }

    fn stage_proposal(&mut self, proposal: &Proposal) -> Result<()> {
        self.set_revision_context(proposal.context().clone())?;
        for command in proposal.commands() {
            match command {
                Command::AppendMetadata {
                    target,
                    property,
                    value,
                } => self.add_metadata_value(*target, property, value)?,
                Command::ReplaceMetadata {
                    target,
                    property,
                    values,
                } => self.replace_metadata_values(*target, property, values)?,
                Command::RemoveMetadata { target, property } => {
                    self.remove_metadata_property(*target, property)?;
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::Unsupported,
                        "unsupported proposal command",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl SqliteProduction {
    /// Submits identified metadata intent through the native atomic commit path.
    ///
    /// Equivalent retries return the original accepted/no-op/rejected result
    /// before checking current state. This development slice does not yet offer
    /// complete history export, replay or credential-bearing job commands.
    ///
    /// # Errors
    /// Rejects mismatched scope/request identity and passive roles; storage
    /// failures require lookup or retry with the same request identity.
    pub fn submit_proposal(&mut self, proposal: &Proposal) -> ExchangeResult<Outcome> {
        let mut edit = self.begin_transaction().map_err(|error| {
            if error.kind() == ErrorKind::Unsupported {
                ExchangeError::Protocol(ProtocolError::new(
                    FailureKind::MirrorReadOnly,
                    "passive mirror rejects proposal submission",
                ))
            } else {
                ExchangeError::Store(error)
            }
        })?;
        edit.submit(proposal)
    }
}
