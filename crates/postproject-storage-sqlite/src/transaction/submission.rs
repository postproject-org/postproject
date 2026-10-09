//! Identified local proposals reuse the native staging/commit path.

use postproject_core::{CommitReceipt, Error, ErrorKind, Result};
use postproject_protocol::{Command, FailureKind, Outcome, Proposal, ProtocolError, Rejection};
use rusqlite::OptionalExtension;

use super::SqliteTransaction;
use crate::{ExchangeError, ExchangeResult, SqliteProduction, sqlite_error, stored_u64};

mod capabilities;
mod jobs;
use crate::{LocalSubmissionResult, SqliteJobLease};
use capabilities::CapabilityInput;
use jobs::StagedCapabilities;

impl SqliteTransaction<'_> {
    fn submit(
        &mut self,
        proposal: &Proposal,
        input: &CapabilityInput<'_>,
    ) -> ExchangeResult<LocalSubmissionResult> {
        self.lifecycle.ensure_open()?;
        let result = self.submit_open(proposal, input);
        if result.is_err() {
            self.abort_failed_commit()?;
        }
        result
    }

    fn submit_open(
        &mut self,
        proposal: &Proposal,
        input: &CapabilityInput<'_>,
    ) -> ExchangeResult<LocalSubmissionResult> {
        // The writer lock is already held. Identity recovery precedes every
        // current-base/domain guard, including for retained terminal rejection.
        if let Some(outcome) = crate::exchange::lookup_for_submission(
            self.open_transaction()?,
            proposal.scope(),
            proposal.client(),
            proposal.request(),
            proposal.digest()?,
            input.binding(),
        )? {
            self.rollback()?;
            return Ok(LocalSubmissionResult::new(outcome, vec![]));
        }
        self.open_transaction()?
            .execute_batch("SAVEPOINT submission_body")
            .map_err(sqlite_error("begin submission staging"))?;
        let mut capabilities = StagedCapabilities::new(input);
        let rejection = if self.select_submission_base(proposal)? {
            match self
                .stage_proposal(proposal, input, &mut capabilities)
                .and_then(|()| self.prepare_commit())
            {
                Ok(receipt) => {
                    let outcome = Outcome::accepted(proposal, receipt.clone())?;
                    self.open_transaction()?
                        .execute_batch("RELEASE submission_body")
                        .map_err(sqlite_error("finish submission staging"))?;
                    self.finish_commit(receipt, Some(&outcome), input.binding())?;
                    return capabilities.finish(outcome).map_err(Into::into);
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
        self.finish_leases(false);
        if let Some(high_water) = self.lease_high_water {
            crate::job_clock::persist(self.open_transaction()?, high_water)?;
        }
        let outcome = Outcome::rejected(proposal, rejection)?;
        let receipt = CommitReceipt::new(self.production.id(), None);
        self.finish_commit(receipt, Some(&outcome), input.binding())?;
        Ok(LocalSubmissionResult::new(outcome, vec![]))
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

    fn stage_proposal(
        &mut self,
        proposal: &Proposal,
        input: &CapabilityInput<'_>,
        capabilities: &mut StagedCapabilities<'_, '_>,
    ) -> Result<()> {
        input.validate_scope(self.production.id())?;
        self.set_revision_context(proposal.context().clone())?;
        self.record_extensions = proposal.extensions().clone();
        for command in proposal.commands() {
            if jobs::stage(self, command, capabilities)? {
                continue;
            }
            match command {
                Command::ImportOriginal(import) => self.import_original(import)?,
                Command::AddRepresentation(import) => self.add_representation(import)?,
                Command::CreateActivity(activity) => self.create_activity(activity)?,
                Command::RecordDependencySet {
                    representation_id,
                    dependencies,
                } => {
                    self.record_dependency_set(*representation_id, dependencies)?;
                }
                Command::RecordResourceFingerprint {
                    resource_id,
                    fingerprint,
                } => {
                    self.record_resource_fingerprint(*resource_id, fingerprint)?;
                }
                Command::RecordRepresentationFingerprint {
                    representation_id,
                    fingerprint,
                } => {
                    self.record_representation_fingerprint(*representation_id, fingerprint)?;
                }
                Command::AddMediaRoot(root) => self.add_media_root(root.clone())?,
                Command::SetMediaRootEnabled { root_id, enabled } => {
                    self.set_media_root_enabled(*root_id, *enabled)?;
                }
                Command::RemoveMediaRoot(id) => self.remove_media_root(*id)?,
                Command::AddLocator(locator) => self.add_locator(locator)?,
                Command::RetireLocator(id) => self.retire_locator(*id)?,
                Command::AddIdentifier(attachment) => {
                    self.add_external_identifier(attachment.target(), attachment.identifier())?;
                }
                Command::RemoveIdentifier(attachment) => {
                    self.remove_external_identifier(attachment.target(), attachment.identifier())?;
                }
                Command::RecordResourceFileFacts { resource_id, facts } => {
                    self.record_resource_file_facts(*resource_id, *facts)?;
                }
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
    /// Submits identified metadata/media intent through native atomic staging.
    ///
    /// Equivalent retries return the original accepted/no-op/rejected result
    /// before checking current state. Newly claimed ownership is deliberately
    /// discarded by this public-outcome facade; use the local result method
    /// when a worker needs delivery.
    ///
    /// # Errors
    /// Rejects mismatched scope/request identity and passive roles; storage
    /// failures require lookup or retry with the same request identity.
    pub fn submit_proposal(&mut self, proposal: &Proposal) -> ExchangeResult<Outcome> {
        self.submit_proposal_with_capabilities(proposal, &[], &[])
            .map(|result| result.into_parts().0)
    }

    /// Submits trusted local intent with private borrowed leases or token input.
    ///
    /// At most 1,000 distinct job capabilities may be supplied. Neither tokens
    /// nor private bindings enter the proposal, public result or records.
    /// Equivalent retries return their original outcome with no new leases,
    /// without reevaluating current ownership. Fresh requests keep all native
    /// clock, expiry, input and publication guards.
    ///
    /// # Errors
    /// Rejects malformed/duplicate private context and mismatched identities or
    /// passive roles. Recover uncertain storage results using the same identity.
    pub fn submit_proposal_with_capabilities(
        &mut self,
        proposal: &Proposal,
        leases: &[&SqliteJobLease],
        tokens: &[&str],
    ) -> ExchangeResult<LocalSubmissionResult> {
        let input = CapabilityInput::new(leases, tokens)?;
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
        edit.submit(proposal, &input)
    }
}
