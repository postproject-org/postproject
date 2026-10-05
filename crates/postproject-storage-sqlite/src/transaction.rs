//! Explicit SQLite-backed domain transactions.

use std::collections::BTreeMap;

use postproject_core::{
    Activity, AgentIdentity, CommitReceipt, ContentStructure, ContentStructureKind, DecisionBase,
    Dependency, DependencySetStatus, DependencyTarget, Error, ErrorKind, ExternalIdentifier,
    FileFacts, Job, JobClaim, JobClaimId, JobFailure, JobId, JobState, Locator,
    LocatorAvailability, LocatorId, MAX_DEPENDENCIES_PER_SET, MediaRoot, MediaRootId,
    MetadataProperty, MetadataValue, ObjectRef, OriginalMediaImport, Production,
    ProductionStoreTransaction, Representation, RepresentationFingerprint, RepresentationId,
    RepresentationImport, RepresentationKind, Resource, ResourceFingerprint, ResourceId, Result,
    Revision, RevisionContext, RevisionEventKind, RevisionId, SemanticConflictKey, SequenceNaming,
    Timestamp, ToolIdentity, TransactionConflict, TransactionId, TransactionLifecycle,
    TransactionState,
};
use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params,
};

use crate::{
    dependency_snapshot::persist_dependency_snapshot, encode_identifier_target,
    encode_metadata_target, load_dependency_set, metadata_codec, revision_wait::RevisionSignal,
    sqlite_error,
};

/// An explicit production mutation transaction.
///
/// Dropping an open value rolls its SQLite transaction back. Call [`Self::commit`]
/// to make all staged mutations durable or [`Self::rollback`] to discard them
/// explicitly.
pub struct SqliteTransaction<'production> {
    transaction: Option<Transaction<'production>>,
    lifecycle: TransactionLifecycle,
    production: &'production mut Production,
    pending_roots: Vec<MediaRoot>,
    revision_context: RevisionContext,
    pending_events: Vec<RevisionEventKind>,
    base_revision: Option<(Option<RevisionId>, u64)>,
    pending_conflict_keys: BTreeMap<Vec<u8>, SemanticConflictKey>,
    pending_changed_keys: BTreeMap<Vec<u8>, SemanticConflictKey>,
    revision_signal: &'production RevisionSignal,
}

impl<'production> SqliteTransaction<'production> {
    pub(crate) fn begin(
        connection: &'production mut Connection,
        production: &'production mut Production,
        revision_signal: &'production RevisionSignal,
    ) -> Result<Self> {
        Self::begin_with_base(connection, production, revision_signal, None)
    }

    pub(crate) fn begin_at(
        connection: &'production mut Connection,
        production: &'production mut Production,
        revision_signal: &'production RevisionSignal,
        base_revision: RevisionId,
    ) -> Result<Self> {
        Self::begin_with_base(connection, production, revision_signal, Some(base_revision))
    }

    pub(crate) fn begin_decision(
        connection: &'production mut Connection,
        production: &'production mut Production,
        revision_signal: &'production RevisionSignal,
        base: DecisionBase,
    ) -> Result<Self> {
        if base.production_id() != production.id() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "decision base belongs to another production",
            ));
        }
        let mut edit =
            Self::begin_with_base(connection, production, revision_signal, base.revision_id())?;
        let sequence = edit.base_revision.map_or(0, |(_, sequence)| sequence);
        if sequence != base.sequence() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "decision revision and sequence do not match",
            ));
        }
        // A protected empty base differs from the legacy unbased write path.
        edit.base_revision = Some((base.revision_id(), base.sequence()));
        Ok(edit)
    }

    fn begin_with_base(
        connection: &'production mut Connection,
        production: &'production mut Production,
        revision_signal: &'production RevisionSignal,
        base_revision: Option<RevisionId>,
    ) -> Result<Self> {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error("begin domain transaction"))?;
        let base_revision = base_revision
            .map(|revision_id| {
                transaction
                    .query_row(
                        "SELECT sequence FROM revisions WHERE id = ?1",
                        [revision_id.as_bytes().as_slice()],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()
                    .map_err(sqlite_error("load transaction base revision"))?
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::NotFound,
                            "transaction base revision does not exist",
                        )
                    })
                    .and_then(|sequence| {
                        crate::stored_u64(sequence, "transaction base revision sequence")
                    })
                    .map(|sequence| (Some(revision_id), sequence))
            })
            .transpose()?;
        let current = crate::load_production(&transaction)?;
        if current.id() != production.id() {
            return Err(Error::new(
                ErrorKind::Conflict,
                "production identity changed",
            ));
        }
        let pending_roots = current.media_roots().to_vec();
        Ok(Self {
            transaction: Some(transaction),
            lifecycle: TransactionLifecycle::new(),
            production,
            pending_roots,
            revision_context: RevisionContext::default(),
            pending_events: Vec::new(),
            base_revision,
            pending_conflict_keys: BTreeMap::new(),
            pending_changed_keys: BTreeMap::new(),
            revision_signal,
        })
    }

    /// Returns the stable transaction identity.
    #[must_use]
    pub const fn id(&self) -> TransactionId {
        self.lifecycle.id()
    }

    /// Returns the transaction lifecycle state.
    #[must_use]
    pub const fn state(&self) -> TransactionState {
        self.lifecycle.state()
    }

    /// Sets the origin and message for the revision created on commit.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Conflict`] if the transaction is closed.
    pub fn set_revision_context(&mut self, context: RevisionContext) -> Result<()> {
        self.lifecycle.ensure_open()?;
        self.revision_context = context;
        Ok(())
    }

    /// Stages a prepared original-media import atomically.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Conflict`] if the transaction is closed,
    /// [`ErrorKind::Unsupported`] if the file size exceeds SQLite's signed
    /// integer range, or a storage/conflict error if persistence fails.
    pub fn import_original(&mut self, import: &OriginalMediaImport) -> Result<()> {
        let transaction = self.open_transaction()?;
        let asset = import.asset();

        transaction
            .execute(
                "INSERT INTO assets (id, created_at_micros, display_name, import_source)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    asset.id().as_bytes().as_slice(),
                    asset.created_at().as_unix_micros(),
                    asset.display_name(),
                    asset.import_source(),
                ],
            )
            .map_err(mutation_error("persist imported asset"))?;
        self.pending_events.push(RevisionEventKind::AssetImported {
            asset_id: asset.id(),
        });
        self.persist_representation(
            import.representation(),
            import.resources(),
            import.locators(),
        )
    }

    /// Stages a representation and its resources on an existing asset.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the owning asset is absent, or a
    /// storage-domain error when persistence rejects the aggregate.
    pub fn add_representation(&mut self, import: &RepresentationImport) -> Result<()> {
        let transaction = self.open_transaction()?;
        if !asset_exists(transaction, import.representation().asset_id())? {
            return Err(Error::new(
                ErrorKind::NotFound,
                "representation asset does not exist",
            ));
        }
        self.persist_representation(
            import.representation(),
            import.resources(),
            import.locators(),
        )
    }

    /// Stages one requested job with its canonical inputs.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the supplied job is not in
    /// the requested state, [`ErrorKind::NotFound`] when an asset, input, or
    /// target root is absent, or a transaction/storage error.
    pub fn request_job(&mut self, job: &Job) -> Result<()> {
        self.lifecycle.ensure_open()?;
        if !matches!(job.state(), JobState::Requested) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "only a requested job can be persisted as a new request",
            ));
        }
        if let Some(target_root) = job.requested_output().target_root() {
            if !self
                .pending_roots
                .iter()
                .any(|root| root.name() == target_root)
            {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "job target root does not exist",
                ));
            }
        }
        let output_kind = encode_representation_kind(job.requested_output().representation_kind())?;
        let transaction = self.open_transaction()?;
        if !asset_exists(transaction, job.requested_output().asset_id())? {
            return Err(Error::new(
                ErrorKind::NotFound,
                "job output asset does not exist",
            ));
        }
        for input in job.inputs() {
            if !representation_exists(transaction, *input)? {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "job input representation does not exist",
                ));
            }
        }
        transaction
            .execute(
                "INSERT INTO jobs (
                    id, kind, output_asset_id, output_representation_kind,
                    target_root, state
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                params![
                    job.id().as_bytes().as_slice(),
                    job.kind().as_str(),
                    job.requested_output().asset_id().as_bytes().as_slice(),
                    output_kind,
                    job.requested_output().target_root(),
                ],
            )
            .map_err(mutation_error("persist job request"))?;
        for (position, input) in job.inputs().iter().enumerate() {
            let position = i64::try_from(position).map_err(|error| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("job input position cannot be stored: {error}"),
                )
            })?;
            transaction
                .execute(
                    "INSERT INTO job_inputs (job_id, position, representation_id)
                     VALUES (?1, ?2, ?3)",
                    params![
                        job.id().as_bytes().as_slice(),
                        position,
                        input.as_bytes().as_slice(),
                    ],
                )
                .map_err(mutation_error("persist job input"))?;
        }
        self.pending_events
            .push(RevisionEventKind::JobRequested { job_id: job.id() });
        Ok(())
    }

    /// Atomically claims a requested or expired job with a new random token.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for a non-future expiry,
    /// [`ErrorKind::NotFound`] for an absent job, [`ErrorKind::Conflict`] when
    /// the job is not claimable, or a transaction/storage error.
    pub fn claim_job(
        &mut self,
        job_id: JobId,
        tool: &ToolIdentity,
        agent: Option<&AgentIdentity>,
        now: Timestamp,
        expires_at: Timestamp,
    ) -> Result<JobClaim> {
        self.claim_job_with_id(job_id, JobClaimId::new(), tool, agent, now, expires_at)
    }

    /// Claims a job using a library-generated capability supplied by an
    /// adapter that must return the token before transaction commit.
    #[doc(hidden)]
    pub fn claim_job_with_id(
        &mut self,
        job_id: JobId,
        claim_id: JobClaimId,
        tool: &ToolIdentity,
        agent: Option<&AgentIdentity>,
        now: Timestamp,
        expires_at: Timestamp,
    ) -> Result<JobClaim> {
        validate_future_job_expiry(now, expires_at)?;
        let agent_name = agent.and_then(AgentIdentity::name);
        let agent_identifier = agent.and_then(AgentIdentity::identifier);
        let transaction = self.open_transaction()?;
        let changed = transaction
            .execute(
                "UPDATE jobs SET
                    state = 2, claim_id = ?1, claim_tool_name = ?2,
                    claim_tool_version = ?3, claim_tool_uri = ?4,
                    claim_agent_name = ?5, claim_agent_scheme = ?6,
                    claim_agent_value = ?7, claim_agent_qualifier = ?8,
                    claim_expires_at_micros = ?9
                 WHERE id = ?10
                   AND (state = 1 OR (state = 2 AND claim_expires_at_micros <= ?11))",
                params![
                    claim_id.as_bytes().as_slice(),
                    tool.name(),
                    tool.version(),
                    tool.uri(),
                    agent_name,
                    agent_identifier.map(|identifier| identifier.scheme().as_str()),
                    agent_identifier.map(ExternalIdentifier::value),
                    agent_identifier.and_then(ExternalIdentifier::qualifier),
                    expires_at.as_unix_micros(),
                    job_id.as_bytes().as_slice(),
                    now.as_unix_micros(),
                ],
            )
            .map_err(mutation_error("claim job"))?;
        if changed == 0 {
            return Err(job_transition_error(
                transaction,
                job_id,
                "job is not claimable",
            )?);
        }
        self.pending_events
            .push(RevisionEventKind::JobClaimed { job_id });
        Ok(JobClaim::new(
            claim_id,
            tool.clone(),
            agent.cloned(),
            expires_at,
        ))
    }

    /// Extends the current unexpired claim to a later expiry.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for a non-future expiry,
    /// [`ErrorKind::NotFound`] for an absent job, [`ErrorKind::Conflict`] for a
    /// stale token, expired lease, or non-extending expiry, or a storage error.
    pub fn renew_job_claim(
        &mut self,
        job_id: JobId,
        claim_id: JobClaimId,
        now: Timestamp,
        expires_at: Timestamp,
    ) -> Result<()> {
        validate_future_job_expiry(now, expires_at)?;
        let transaction = self.open_transaction()?;
        let changed = transaction
            .execute(
                "UPDATE jobs SET claim_expires_at_micros = ?1
                 WHERE id = ?2 AND state = 2 AND claim_id = ?3
                   AND claim_expires_at_micros > ?4
                   AND claim_expires_at_micros < ?1",
                params![
                    expires_at.as_unix_micros(),
                    job_id.as_bytes().as_slice(),
                    claim_id.as_bytes().as_slice(),
                    now.as_unix_micros(),
                ],
            )
            .map_err(mutation_error("renew job claim"))?;
        if changed == 0 {
            return Err(job_transition_error(
                transaction,
                job_id,
                "job claim cannot be renewed",
            )?);
        }
        self.pending_events
            .push(RevisionEventKind::JobClaimRenewed { job_id });
        Ok(())
    }

    /// Releases the current claim and returns the job to requested state.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] for an absent job,
    /// [`ErrorKind::Conflict`] for a stale token or non-claimed job, or a
    /// transaction/storage error.
    pub fn release_job_claim(&mut self, job_id: JobId, claim_id: JobClaimId) -> Result<()> {
        let transaction = self.open_transaction()?;
        let changed = transaction
            .execute(
                "UPDATE jobs SET
                    state = 1, claim_id = NULL, claim_tool_name = NULL,
                    claim_tool_version = NULL, claim_tool_uri = NULL,
                    claim_agent_name = NULL, claim_agent_scheme = NULL,
                    claim_agent_value = NULL, claim_agent_qualifier = NULL,
                    claim_expires_at_micros = NULL
                 WHERE id = ?1 AND state = 2 AND claim_id = ?2",
                params![job_id.as_bytes().as_slice(), claim_id.as_bytes().as_slice(),],
            )
            .map_err(mutation_error("release job claim"))?;
        if changed == 0 {
            return Err(job_transition_error(
                transaction,
                job_id,
                "job claim cannot be released",
            )?);
        }
        self.pending_events
            .push(RevisionEventKind::JobClaimReleased { job_id });
        Ok(())
    }

    /// Fails an actively claimed job without creating output or activity facts.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] for an absent job,
    /// [`ErrorKind::Conflict`] for a stale token, expired lease, or non-claimed
    /// job, or a transaction/storage error.
    pub fn fail_job(
        &mut self,
        job_id: JobId,
        claim_id: JobClaimId,
        now: Timestamp,
        failure: &JobFailure,
    ) -> Result<()> {
        let transaction = self.open_transaction()?;
        let changed = transaction
            .execute(
                "UPDATE jobs SET
                    state = 4, claim_id = NULL, claim_tool_name = NULL,
                    claim_tool_version = NULL, claim_tool_uri = NULL,
                    claim_agent_name = NULL, claim_agent_scheme = NULL,
                    claim_agent_value = NULL, claim_agent_qualifier = NULL,
                    claim_expires_at_micros = NULL, failure_diagnostic = ?1
                 WHERE id = ?2 AND state = 2 AND claim_id = ?3
                   AND claim_expires_at_micros > ?4",
                params![
                    failure.diagnostic(),
                    job_id.as_bytes().as_slice(),
                    claim_id.as_bytes().as_slice(),
                    now.as_unix_micros(),
                ],
            )
            .map_err(mutation_error("fail job"))?;
        if changed == 0 {
            return Err(job_transition_error(
                transaction,
                job_id,
                "job cannot be failed by this claim",
            )?);
        }
        self.pending_events
            .push(RevisionEventKind::JobFailed { job_id });
        Ok(())
    }

    /// Atomically completes a claimed job with its output and activity fact.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] for an absent job,
    /// [`ErrorKind::Conflict`] for a stale token or expired lease,
    /// [`ErrorKind::InvalidArgument`] when output or activity facts do not
    /// match the request, or a transaction/storage error. Any failure rolls
    /// back all completion-specific changes.
    pub fn complete_job(
        &mut self,
        job_id: JobId,
        claim_id: JobClaimId,
        now: Timestamp,
        output: &RepresentationImport,
        activity: &Activity,
    ) -> Result<()> {
        const SAVEPOINT: &str = "postproject_complete_job";
        let pending_event_count = self.pending_events.len();
        self.open_transaction()?
            .execute_batch("SAVEPOINT postproject_complete_job")
            .map_err(sqlite_error("start job completion savepoint"))?;
        let result = self.complete_job_inner(job_id, claim_id, now, output, activity);
        if let Err(error) = result {
            self.pending_events.truncate(pending_event_count);
            self.open_transaction()?
                .execute_batch(&format!("ROLLBACK TO {SAVEPOINT}; RELEASE {SAVEPOINT}"))
                .map_err(sqlite_error("roll back job completion"))?;
            return Err(error);
        }
        if let Err(error) = self
            .open_transaction()?
            .execute_batch(&format!("RELEASE {SAVEPOINT}"))
        {
            self.pending_events.truncate(pending_event_count);
            self.open_transaction()?
                .execute_batch(&format!("ROLLBACK TO {SAVEPOINT}; RELEASE {SAVEPOINT}"))
                .map_err(sqlite_error("roll back unreleased job completion"))?;
            return Err(sqlite_error("release job completion savepoint")(error));
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "claim, output, provenance, and terminal-state validation form one atomic invariant"
    )]
    fn complete_job_inner(
        &mut self,
        job_id: JobId,
        claim_id: JobClaimId,
        now: Timestamp,
        output: &RepresentationImport,
        activity: &Activity,
    ) -> Result<()> {
        let transaction = self.open_transaction()?;
        let stored = transaction
            .query_row(
                "SELECT state, claim_id, claim_expires_at_micros,
                        output_asset_id, output_representation_kind
                 FROM jobs WHERE id = ?1",
                [job_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(mutation_error("load job completion request"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "job does not exist"))?;
        if stored.0 != 2 {
            return Err(Error::new(ErrorKind::Conflict, "job is not claimed"));
        }
        let stored_claim = JobClaimId::from_bytes(crate::id_bytes(
            stored
                .1
                .ok_or_else(|| stored_job_invariant("claimed job has no claim token"))?,
            "job claim",
        )?);
        let stored_expiry = stored
            .2
            .ok_or_else(|| stored_job_invariant("claimed job has no lease expiry"))?;
        if stored_claim != claim_id || stored_expiry <= now.as_unix_micros() {
            return Err(Error::new(
                ErrorKind::Conflict,
                "job cannot be completed by this claim",
            ));
        }
        let requested_asset =
            postproject_core::AssetId::from_bytes(crate::id_bytes(stored.3, "job output asset")?);
        let representation = output.representation();
        if representation.asset_id() != requested_asset
            || encode_representation_kind(representation.kind())? != stored.4
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "job output does not match the requested asset and representation kind",
            ));
        }
        let expected_inputs = load_job_input_ids(transaction, job_id)?;
        let mut activity_inputs = activity
            .inputs()
            .iter()
            .map(postproject_core::ActivityInput::representation_id)
            .collect::<Vec<_>>();
        activity_inputs.sort_unstable();
        if activity_inputs != expected_inputs {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "completion activity inputs do not match the job inputs",
            ));
        }
        let [activity_output] = activity.outputs() else {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "completion activity must have exactly one output",
            ));
        };
        if activity_output.representation_id() != representation.id() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "completion activity output does not match the job output",
            ));
        }

        self.add_representation(output)?;
        self.create_activity(activity)?;
        let changed = self
            .open_transaction()?
            .execute(
                "UPDATE jobs SET
                    state = 3, claim_id = NULL, claim_tool_name = NULL,
                    claim_tool_version = NULL, claim_tool_uri = NULL,
                    claim_agent_name = NULL, claim_agent_scheme = NULL,
                    claim_agent_value = NULL, claim_agent_qualifier = NULL,
                    claim_expires_at_micros = NULL, completion_activity_id = ?1,
                    completion_representation_id = ?2
                 WHERE id = ?3 AND state = 2 AND claim_id = ?4
                   AND claim_expires_at_micros > ?5",
                params![
                    activity.id().as_bytes().as_slice(),
                    representation.id().as_bytes().as_slice(),
                    job_id.as_bytes().as_slice(),
                    claim_id.as_bytes().as_slice(),
                    now.as_unix_micros(),
                ],
            )
            .map_err(mutation_error("complete job"))?;
        if changed != 1 {
            return Err(Error::new(
                ErrorKind::Conflict,
                "job claim changed during completion",
            ));
        }
        self.pending_events
            .push(RevisionEventKind::JobSucceeded { job_id });
        Ok(())
    }

    /// Cancels a requested or claimed job administratively.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] for an absent job,
    /// [`ErrorKind::Conflict`] for a terminal job, or a transaction/storage
    /// error.
    pub fn cancel_job(&mut self, job_id: JobId) -> Result<()> {
        let transaction = self.open_transaction()?;
        let changed = transaction
            .execute(
                "UPDATE jobs SET
                    state = 5, claim_id = NULL, claim_tool_name = NULL,
                    claim_tool_version = NULL, claim_tool_uri = NULL,
                    claim_agent_name = NULL, claim_agent_scheme = NULL,
                    claim_agent_value = NULL, claim_agent_qualifier = NULL,
                    claim_expires_at_micros = NULL
                 WHERE id = ?1 AND state IN (1, 2)",
                [job_id.as_bytes().as_slice()],
            )
            .map_err(mutation_error("cancel job"))?;
        if changed == 0 {
            return Err(job_transition_error(
                transaction,
                job_id,
                "job cannot be cancelled",
            )?);
        }
        self.pending_events
            .push(RevisionEventKind::JobCancelled { job_id });
        Ok(())
    }

    fn persist_representation(
        &mut self,
        representation: &Representation,
        resources: &[Resource],
        locators: &[Locator],
    ) -> Result<()> {
        let transaction = self.open_transaction()?;
        let observation_sequence = next_revision_sequence(transaction)?;
        transaction
            .execute(
                "INSERT INTO representations (
                    id, asset_id, kind, structure_kind
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    representation.id().as_bytes().as_slice(),
                    representation.asset_id().as_bytes().as_slice(),
                    encode_representation_kind(representation.kind())?,
                    encode_structure_kind(representation.content_structure().kind())?,
                ],
            )
            .map_err(mutation_error("persist representation"))?;
        for fingerprint in representation.fingerprints() {
            transaction
                .execute(
                    "INSERT INTO representation_fingerprints (
                        representation_id, algorithm, algorithm_version, value,
                        observed_revision_sequence
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        representation.id().as_bytes().as_slice(),
                        fingerprint.algorithm(),
                        fingerprint.version(),
                        fingerprint.value(),
                        observation_sequence,
                    ],
                )
                .map_err(mutation_error("persist representation fingerprint"))?;
        }
        for resource in resources {
            persist_resource(transaction, resource, observation_sequence)?;
        }
        persist_content_structure(
            transaction,
            representation.id(),
            representation.content_structure(),
        )?;
        for locator in locators {
            persist_locator(transaction, locator)?;
        }
        self.pending_events
            .push(RevisionEventKind::RepresentationAdded {
                asset_id: representation.asset_id(),
                representation_id: representation.id(),
            });
        self.pending_events.extend(resources.iter().map(|resource| {
            RevisionEventKind::ResourceAdded {
                resource_id: resource.id(),
            }
        }));
        for (position, resource_id) in representation
            .content_structure()
            .resource_ids()
            .into_iter()
            .enumerate()
        {
            let position = u32::try_from(position).map_err(|error| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("content member position cannot be journaled: {error}"),
                )
            })?;
            self.pending_events
                .push(RevisionEventKind::RepresentationResourceAdded {
                    representation_id: representation.id(),
                    resource_id,
                    position,
                });
        }
        self.pending_events.extend(locators.iter().map(|locator| {
            RevisionEventKind::LocatorAdded {
                resource_id: locator.resource_id(),
                locator_id: locator.id(),
            }
        }));
        Ok(())
    }

    /// Stages an additional confirmed locator for a resource.
    ///
    /// A locator of an image-sequence resource carries the naming of its files
    /// and no other locator does. A resource may hold one URI under several
    /// namings.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Conflict`] if the transaction is closed,
    /// [`ErrorKind::InvalidArgument`] when the sequence naming is present for
    /// a resource that is not an image sequence or absent for one that is,
    /// [`ErrorKind::AlreadyExists`] for a locator with the same URI and naming
    /// or a missing owning resource, or [`ErrorKind::Storage`] for other
    /// persistence failures.
    pub fn add_locator(&mut self, locator: &Locator) -> Result<()> {
        persist_locator(self.open_transaction()?, locator)?;
        self.record_conflict_key(SemanticConflictKey::LocatorSet(locator.resource_id()))?;
        self.pending_events.push(RevisionEventKind::LocatorAdded {
            resource_id: locator.resource_id(),
            locator_id: locator.id(),
        });
        Ok(())
    }

    /// Stages retirement of a superseded locator.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when `locator_id` is absent, or a
    /// transaction/storage error.
    pub fn retire_locator(&mut self, locator_id: LocatorId) -> Result<()> {
        let transaction = self.open_transaction()?;
        let resource_id = transaction
            .query_row(
                "SELECT resource_id FROM locators WHERE id = ?1",
                params![locator_id.as_bytes().as_slice()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(sqlite_error("load resource locator"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "resource locator does not exist"))?;
        let resource_id = postproject_core::ResourceId::from_bytes(crate::id_bytes(
            resource_id,
            "locator resource",
        )?);
        transaction
            .execute(
                "DELETE FROM locators WHERE id = ?1",
                params![locator_id.as_bytes().as_slice()],
            )
            .map_err(mutation_error("retire resource locator"))?;
        self.record_conflict_key(SemanticConflictKey::LocatorSet(resource_id))?;
        self.pending_events.push(RevisionEventKind::LocatorRetired {
            resource_id,
            locator_id,
        });
        Ok(())
    }

    /// Stages a configured media root.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Conflict`] if the transaction is closed,
    /// [`ErrorKind::AlreadyExists`] for a duplicate identity or name, or
    /// [`ErrorKind::Storage`] for other persistence failures.
    pub fn add_media_root(&mut self, root: MediaRoot) -> Result<()> {
        let media_root_id = root.id();
        let transaction = self.open_transaction()?;
        transaction
            .execute(
                "INSERT INTO media_roots (id, name, label, legacy_uri, priority, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    root.id().as_bytes().as_slice(),
                    root.name(),
                    root.label(),
                    root.legacy_uri(),
                    root.priority(),
                    root.is_enabled(),
                ],
            )
            .map_err(mutation_error("persist media root"))?;
        self.pending_roots.push(root);
        self.pending_roots
            .sort_by_key(|item| (item.priority(), item.id()));
        self.pending_events
            .push(RevisionEventKind::MediaRootAdded { media_root_id });
        // New identities still merge independently; recording their initial
        // version protects later updates made from an older, possibly empty view.
        self.record_conflict_key(SemanticConflictKey::MediaRoot(media_root_id))?;
        Ok(())
    }

    /// Enables or disables a configured media root.
    ///
    /// Requires a decision base; early rejection leaves the transaction open.
    /// Reapplying the current state succeeds without creating a semantic event.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when `root_id` is absent, or a
    /// transaction/storage error.
    pub fn set_media_root_enabled(&mut self, root_id: MediaRootId, enabled: bool) -> Result<()> {
        self.require_decision_base()?;
        let Some(index) = self
            .pending_roots
            .iter()
            .position(|root| root.id() == root_id)
        else {
            return Err(Error::new(ErrorKind::NotFound, "media root does not exist"));
        };
        if self.pending_roots[index].is_enabled() == enabled {
            return Ok(());
        }
        let current = &self.pending_roots[index];
        let replacement = MediaRoot::new(
            current.id(),
            current.name(),
            current.label().map(str::to_owned),
            current.legacy_uri().map(str::to_owned),
            current.priority(),
            enabled,
        )?;
        self.open_transaction()?
            .execute(
                "UPDATE media_roots SET enabled = ?1 WHERE id = ?2",
                params![enabled, root_id.as_bytes().as_slice()],
            )
            .map_err(mutation_error("update media root"))?;
        self.pending_roots[index] = replacement;
        self.record_conflict_key(SemanticConflictKey::MediaRoot(root_id))?;
        self.pending_events
            .push(RevisionEventKind::MediaRootEnabledChanged {
                media_root_id: root_id,
                enabled,
            });
        Ok(())
    }

    /// Removes a configured media root.
    /// Requires a decision base; early rejection leaves the transaction open.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when `root_id` is absent, or a
    /// transaction/storage error.
    pub fn remove_media_root(&mut self, root_id: MediaRootId) -> Result<()> {
        self.require_decision_base()?;
        if !self.pending_roots.iter().any(|root| root.id() == root_id) {
            return Err(Error::new(ErrorKind::NotFound, "media root does not exist"));
        }
        self.open_transaction()?
            .execute(
                "DELETE FROM media_roots WHERE id = ?1",
                params![root_id.as_bytes().as_slice()],
            )
            .map_err(mutation_error("remove media root"))?;
        self.pending_roots.retain(|root| root.id() != root_id);
        self.record_conflict_key(SemanticConflictKey::MediaRoot(root_id))?;
        self.pending_events
            .push(RevisionEventKind::MediaRootRemoved {
                media_root_id: root_id,
            });
        Ok(())
    }

    /// Stages an external identifier attachment.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the target does not exist,
    /// [`ErrorKind::AlreadyExists`] for an identical attachment,
    /// [`ErrorKind::Unsupported`] for a target kind not yet persisted, or a
    /// transaction/storage error.
    pub fn add_external_identifier(
        &mut self,
        target: ObjectRef,
        identifier: &ExternalIdentifier,
    ) -> Result<()> {
        let (target_kind, target_id) = encode_identifier_target(&target)?;
        let transaction = self.open_transaction()?;
        if !identifier_target_exists(transaction, target_kind, target_id)? {
            return Err(Error::new(
                ErrorKind::NotFound,
                "external identifier target does not exist",
            ));
        }
        transaction
            .execute(
                "INSERT INTO external_identifiers (
                    target_kind, target_id, scheme, value, qualifier
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    target_kind,
                    target_id.as_slice(),
                    identifier.scheme().as_str(),
                    identifier.value(),
                    identifier.qualifier(),
                ],
            )
            .map_err(mutation_error("persist external identifier"))?;
        self.record_conflict_key(SemanticConflictKey::ExternalIdentifier {
            target,
            identifier: identifier.clone(),
        })?;
        self.pending_events
            .push(RevisionEventKind::ExternalIdentifierAdded {
                target,
                identifier: identifier.clone(),
            });
        Ok(())
    }

    /// Stages removal of an exact external identifier attachment.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] if the attachment does not exist,
    /// [`ErrorKind::Unsupported`] for a target kind not yet persisted, or a
    /// transaction/storage error.
    pub fn remove_external_identifier(
        &mut self,
        target: ObjectRef,
        identifier: &ExternalIdentifier,
    ) -> Result<()> {
        let (target_kind, target_id) = encode_identifier_target(&target)?;
        let changed = self
            .open_transaction()?
            .execute(
                "DELETE FROM external_identifiers
                 WHERE target_kind = ?1 AND target_id = ?2
                   AND scheme = ?3 AND value = ?4 AND qualifier IS ?5",
                params![
                    target_kind,
                    target_id.as_slice(),
                    identifier.scheme().as_str(),
                    identifier.value(),
                    identifier.qualifier(),
                ],
            )
            .map_err(mutation_error("remove external identifier"))?;
        if changed == 0 {
            return Err(Error::new(
                ErrorKind::NotFound,
                "external identifier attachment does not exist",
            ));
        }
        self.record_conflict_key(SemanticConflictKey::ExternalIdentifier {
            target,
            identifier: identifier.clone(),
        })?;
        self.pending_events
            .push(RevisionEventKind::ExternalIdentifierRemoved {
                target,
                identifier: identifier.clone(),
            });
        Ok(())
    }

    /// Appends one ordered value to a metadata property.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the target does not exist,
    /// or a transaction/encoding/storage error. Appends merge independently,
    /// while advancing the version checked by destructive edits.
    pub fn add_metadata_value(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
        value: &MetadataValue,
    ) -> Result<()> {
        let encoded = metadata_codec::encode(value)?;
        let (target_kind, target_id) = encode_metadata_target(&target)?;
        let transaction = self.open_transaction()?;
        ensure_metadata_target_exists(transaction, target_kind, target_id)?;
        let position = next_metadata_position(transaction, target_kind, target_id, property)?;
        insert_metadata_value(
            transaction,
            target_kind,
            target_id,
            property,
            position,
            &encoded,
        )?;
        // An append may merge, but a later stale removal or replacement must
        // still notice that this property's contents changed.
        self.record_changed_key(SemanticConflictKey::MetadataProperty {
            target,
            property: property.clone(),
        })?;
        self.pending_events
            .push(RevisionEventKind::MetadataAddedOrReplaced {
                target,
                property: property.clone(),
            });
        Ok(())
    }

    /// Replaces all ordered values of a metadata property atomically.
    ///
    /// An empty slice removes all values without treating absence as an error.
    /// Requires an edit with a decision base; unbased transactions may append.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] without a decision base,
    /// [`ErrorKind::NotFound`] when the target does not exist,
    /// or a transaction/encoding/storage error.
    pub fn replace_metadata_values(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
        values: &[MetadataValue],
    ) -> Result<()> {
        self.require_decision_base()?;
        let encoded = values
            .iter()
            .map(metadata_codec::encode)
            .collect::<Result<Vec<_>>>()?;
        let (target_kind, target_id) = encode_metadata_target(&target)?;
        let transaction = self.open_transaction()?;
        ensure_metadata_target_exists(transaction, target_kind, target_id)?;
        let removed = delete_metadata_property(transaction, target_kind, target_id, property)?;
        for (position, value) in encoded.iter().enumerate() {
            let position = i64::try_from(position).map_err(|error| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("metadata value position cannot be stored: {error}"),
                )
            })?;
            insert_metadata_value(
                transaction,
                target_kind,
                target_id,
                property,
                position,
                value,
            )?;
        }
        if values.is_empty() {
            if removed > 0 {
                self.record_conflict_key(SemanticConflictKey::MetadataProperty {
                    target,
                    property: property.clone(),
                })?;
                self.pending_events
                    .push(RevisionEventKind::MetadataRemoved {
                        target,
                        property: property.clone(),
                    });
            }
        } else {
            self.record_conflict_key(SemanticConflictKey::MetadataProperty {
                target,
                property: property.clone(),
            })?;
            self.pending_events
                .push(RevisionEventKind::MetadataAddedOrReplaced {
                    target,
                    property: property.clone(),
                });
        }
        Ok(())
    }

    /// Removes all values of one metadata property.
    /// Requires an edit with a decision base.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] without a decision base,
    /// [`ErrorKind::NotFound`] when the property is absent,
    /// [`ErrorKind::Unsupported`] for an unsupported target kind, or a
    /// transaction/storage error.
    pub fn remove_metadata_property(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
    ) -> Result<()> {
        self.require_decision_base()?;
        let (target_kind, target_id) = encode_metadata_target(&target)?;
        let changed =
            delete_metadata_property(self.open_transaction()?, target_kind, target_id, property)?;
        if changed == 0 {
            return Err(Error::new(
                ErrorKind::NotFound,
                "metadata property does not exist on target",
            ));
        }
        self.record_conflict_key(SemanticConflictKey::MetadataProperty {
            target,
            property: property.clone(),
        })?;
        self.pending_events
            .push(RevisionEventKind::MetadataRemoved {
                target,
                property: property.clone(),
            });
        Ok(())
    }

    /// Records a resource's current size and modification time as part of a
    /// content observation.
    ///
    /// Facts are cheap discovery filters, not identity evidence, so they have
    /// no history and emit no event of their own; the accompanying
    /// fingerprint observation carries the revision.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] for an absent resource or a transaction/
    /// storage error. Identical facts are a successful no-op.
    pub fn record_resource_file_facts(
        &mut self,
        resource_id: ResourceId,
        facts: FileFacts,
    ) -> Result<bool> {
        self.lifecycle.ensure_open()?;
        let transaction = self.open_transaction()?;
        let size = i64::try_from(facts.size_bytes())
            .map_err(|_| Error::new(ErrorKind::InvalidArgument, "file size is too large"))?;
        let modified = facts.modified_at().map(Timestamp::as_unix_micros);
        let changed = transaction
            .execute(
                "UPDATE resources SET file_size_bytes = ?2, modified_at_micros = ?3
                 WHERE id = ?1
                   AND (file_size_bytes IS NOT ?2 OR modified_at_micros IS NOT ?3)",
                params![resource_id.as_bytes().as_slice(), size, modified],
            )
            .map_err(mutation_error("update resource file facts"))?;
        if changed == 0 && !resource_exists(transaction, resource_id)? {
            return Err(Error::new(ErrorKind::NotFound, "resource does not exist"));
        }
        Ok(changed != 0)
    }

    /// Records a resource fingerprint observation and marks aggregate owners dirty.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] for an absent resource or a transaction/
    /// storage error. An identical current value is a successful no-op.
    pub fn record_resource_fingerprint(
        &mut self,
        resource_id: ResourceId,
        fingerprint: &ResourceFingerprint,
    ) -> Result<bool> {
        let transaction = self.open_transaction()?;
        let current = transaction
            .query_row(
                "SELECT value, observed_revision_sequence
                 FROM resource_fingerprints
                 WHERE resource_id = ?1 AND algorithm = ?2 AND algorithm_version = ?3",
                params![
                    resource_id.as_bytes().as_slice(),
                    fingerprint.algorithm(),
                    fingerprint.version(),
                ],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()
            .map_err(mutation_error("read current resource fingerprint"))?;
        if current
            .as_ref()
            .is_some_and(|(value, _)| value == fingerprint.value())
        {
            return Ok(false);
        }
        if current.is_none() && !resource_exists(transaction, resource_id)? {
            return Err(Error::new(ErrorKind::NotFound, "resource does not exist"));
        }
        let sequence = next_revision_sequence(transaction)?;
        if let Some((value, observed_sequence)) = current {
            transaction
                .execute(
                    "INSERT INTO resource_fingerprint_history (
                        resource_id, algorithm, algorithm_version, value,
                        observed_revision_sequence, superseded_revision_sequence
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        resource_id.as_bytes().as_slice(),
                        fingerprint.algorithm(),
                        fingerprint.version(),
                        value,
                        observed_sequence,
                        sequence,
                    ],
                )
                .map_err(mutation_error("archive resource fingerprint"))?;
            transaction
                .execute(
                    "UPDATE resource_fingerprints
                     SET value = ?4, observed_revision_sequence = ?5
                     WHERE resource_id = ?1 AND algorithm = ?2 AND algorithm_version = ?3",
                    params![
                        resource_id.as_bytes().as_slice(),
                        fingerprint.algorithm(),
                        fingerprint.version(),
                        fingerprint.value(),
                        sequence,
                    ],
                )
                .map_err(mutation_error("update resource fingerprint"))?;
        } else {
            transaction
                .execute(
                    "INSERT INTO resource_fingerprints (
                        resource_id, algorithm, algorithm_version, value,
                        observed_revision_sequence
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        resource_id.as_bytes().as_slice(),
                        fingerprint.algorithm(),
                        fingerprint.version(),
                        fingerprint.value(),
                        sequence,
                    ],
                )
                .map_err(mutation_error("insert resource fingerprint"))?;
        }
        transaction
            .execute(
                "INSERT INTO representation_fingerprint_recomputations (
                    representation_id, changed_resource_id, marked_revision_sequence
                 )
                 SELECT representation_id, ?1, ?2
                 FROM representation_resources WHERE resource_id = ?1
                 ON CONFLICT(representation_id) DO UPDATE SET
                    changed_resource_id = excluded.changed_resource_id,
                    marked_revision_sequence = excluded.marked_revision_sequence",
                params![resource_id.as_bytes().as_slice(), sequence],
            )
            .map_err(mutation_error(
                "mark representation fingerprints for recomputation",
            ))?;
        self.pending_events
            .push(RevisionEventKind::ResourceFingerprintObserved {
                resource_id,
                algorithm: fingerprint.algorithm().to_owned(),
                version: fingerprint.version(),
            });
        self.record_conflict_key(SemanticConflictKey::ResourceFingerprint {
            resource_id,
            algorithm: fingerprint.algorithm().to_owned(),
            version: fingerprint.version(),
        })?;
        Ok(true)
    }

    /// Records a representation fingerprint and clears its dirty marker.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] for an absent representation or a
    /// transaction/storage error.
    #[allow(
        clippy::too_many_lines,
        reason = "the archive, current-row update, dirty-marker clear, and event are one auditable mutation"
    )]
    pub fn record_representation_fingerprint(
        &mut self,
        representation_id: RepresentationId,
        fingerprint: &RepresentationFingerprint,
    ) -> Result<bool> {
        let transaction = self.open_transaction()?;
        let current = transaction
            .query_row(
                "SELECT value, observed_revision_sequence
                 FROM representation_fingerprints
                 WHERE representation_id = ?1 AND algorithm = ?2 AND algorithm_version = ?3",
                params![
                    representation_id.as_bytes().as_slice(),
                    fingerprint.algorithm(),
                    fingerprint.version(),
                ],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()
            .map_err(mutation_error("read current representation fingerprint"))?;
        let dirty = transaction
            .query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM representation_fingerprint_recomputations
                    WHERE representation_id = ?1
                 )",
                [representation_id.as_bytes().as_slice()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(mutation_error("read representation recomputation marker"))?;
        if current
            .as_ref()
            .is_some_and(|(value, _)| value == fingerprint.value())
            && !dirty
        {
            return Ok(false);
        }
        if current.is_none() && !representation_exists(transaction, representation_id)? {
            return Err(Error::new(
                ErrorKind::NotFound,
                "representation does not exist",
            ));
        }
        let sequence = next_revision_sequence(transaction)?;
        if let Some((value, observed_sequence)) = current {
            if value != fingerprint.value() {
                transaction
                    .execute(
                        "INSERT INTO representation_fingerprint_history (
                            representation_id, algorithm, algorithm_version, value,
                            observed_revision_sequence, superseded_revision_sequence
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            representation_id.as_bytes().as_slice(),
                            fingerprint.algorithm(),
                            fingerprint.version(),
                            value,
                            observed_sequence,
                            sequence,
                        ],
                    )
                    .map_err(mutation_error("archive representation fingerprint"))?;
                transaction
                    .execute(
                        "UPDATE representation_fingerprints
                         SET value = ?4, observed_revision_sequence = ?5
                         WHERE representation_id = ?1
                           AND algorithm = ?2 AND algorithm_version = ?3",
                        params![
                            representation_id.as_bytes().as_slice(),
                            fingerprint.algorithm(),
                            fingerprint.version(),
                            fingerprint.value(),
                            sequence,
                        ],
                    )
                    .map_err(mutation_error("update representation fingerprint"))?;
            }
        } else {
            transaction
                .execute(
                    "INSERT INTO representation_fingerprints (
                        representation_id, algorithm, algorithm_version, value,
                        observed_revision_sequence
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        representation_id.as_bytes().as_slice(),
                        fingerprint.algorithm(),
                        fingerprint.version(),
                        fingerprint.value(),
                        sequence,
                    ],
                )
                .map_err(mutation_error("insert representation fingerprint"))?;
        }
        transaction
            .execute(
                "DELETE FROM representation_fingerprint_recomputations
                 WHERE representation_id = ?1",
                [representation_id.as_bytes().as_slice()],
            )
            .map_err(mutation_error("clear representation recomputation marker"))?;
        transaction
            .execute(
                "UPDATE dependency_sets SET needs_extraction = 1
                 WHERE source_representation_id = ?1 AND needs_extraction = 0",
                [representation_id.as_bytes().as_slice()],
            )
            .map_err(mutation_error("mark dependency set for extraction"))?;
        self.pending_events
            .push(RevisionEventKind::RepresentationFingerprintObserved {
                representation_id,
                algorithm: fingerprint.algorithm().to_owned(),
                version: fingerprint.version(),
            });
        self.record_conflict_key(SemanticConflictKey::RepresentationFingerprint {
            representation_id,
            algorithm: fingerprint.algorithm().to_owned(),
            version: fingerprint.version(),
        })?;
        Ok(true)
    }

    /// Stages a complete production activity and its provenance edges.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when an edge references an absent
    /// representation, [`ErrorKind::AlreadyExists`] for a duplicate activity,
    /// [`ErrorKind::Conflict`] when the new edges create a cycle, or a
    /// transaction/storage error.
    #[allow(
        clippy::too_many_lines,
        reason = "activity, storage-captured edge snapshots, cycle validation, and events are one atomic mutation"
    )]
    pub fn create_activity(&mut self, activity: &Activity) -> Result<()> {
        let transaction = self.open_transaction()?;
        let snapshot_sequence = next_revision_sequence(transaction)?;
        let tool = activity.tool();
        let agent = activity.agent();
        let agent_identifier = agent.and_then(postproject_core::AgentIdentity::identifier);
        transaction
            .execute(
                "INSERT INTO activities (
                    id, kind, started_at_micros, finished_at_micros,
                    tool_name, tool_version, tool_uri, agent_name,
                    agent_identifier_scheme, agent_identifier_value,
                    agent_identifier_qualifier
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    activity.id().as_bytes().as_slice(),
                    activity.kind().as_str(),
                    activity
                        .started_at()
                        .map(postproject_core::Timestamp::as_unix_micros),
                    activity
                        .finished_at()
                        .map(postproject_core::Timestamp::as_unix_micros),
                    tool.map(postproject_core::ToolIdentity::name),
                    tool.and_then(postproject_core::ToolIdentity::version),
                    tool.and_then(postproject_core::ToolIdentity::uri),
                    agent.and_then(postproject_core::AgentIdentity::name),
                    agent_identifier.map(|identifier| identifier.scheme().as_str()),
                    agent_identifier.map(postproject_core::ExternalIdentifier::value),
                    agent_identifier.and_then(postproject_core::ExternalIdentifier::qualifier),
                ],
            )
            .map_err(mutation_error("persist activity"))?;

        let edge_result = (|| {
            for input in activity.inputs() {
                transaction
                    .execute(
                        "INSERT INTO activity_inputs (
                            activity_id, representation_id, role, snapshot_revision_sequence
                         ) VALUES (?1, ?2, ?3, ?4)",
                        params![
                            activity.id().as_bytes().as_slice(),
                            input.representation_id().as_bytes().as_slice(),
                            input.role().map(postproject_core::ActivityRole::as_str),
                            snapshot_sequence,
                        ],
                    )
                    .map_err(activity_edge_error("persist activity input"))?;
                let edge_id = transaction.last_insert_rowid();
                persist_edge_fingerprint_snapshot(
                    transaction,
                    "activity_input_fingerprint_snapshots",
                    "activity_input_id",
                    edge_id,
                    input.representation_id(),
                )?;
                persist_dependency_snapshot(transaction, edge_id, input.representation_id())?;
            }
            for output in activity.outputs() {
                transaction
                    .execute(
                        "INSERT INTO activity_outputs (
                            activity_id, representation_id, role, snapshot_revision_sequence
                         ) VALUES (?1, ?2, ?3, ?4)",
                        params![
                            activity.id().as_bytes().as_slice(),
                            output.representation_id().as_bytes().as_slice(),
                            output.role().map(postproject_core::ActivityRole::as_str),
                            snapshot_sequence,
                        ],
                    )
                    .map_err(activity_edge_error("persist activity output"))?;
                let edge_id = transaction.last_insert_rowid();
                persist_edge_fingerprint_snapshot(
                    transaction,
                    "activity_output_fingerprint_snapshots",
                    "activity_output_id",
                    edge_id,
                    output.representation_id(),
                )?;
            }
            Ok(())
        })();
        if let Err(error) = edge_result {
            delete_activity(transaction, activity)?;
            return Err(error);
        }
        if activity_creates_cycle(transaction, activity)? {
            delete_activity(transaction, activity)?;
            return Err(Error::new(
                ErrorKind::Conflict,
                "activity would create a provenance cycle",
            ));
        }
        self.pending_events
            .push(RevisionEventKind::ActivityCreated {
                activity_id: activity.id(),
                kind: activity.kind().clone(),
            });
        self.pending_events
            .extend(
                activity
                    .inputs()
                    .iter()
                    .map(|input| RevisionEventKind::ActivityInputAdded {
                        activity_id: activity.id(),
                        representation_id: input.representation_id(),
                        role: input.role().cloned(),
                    }),
            );
        self.pending_events
            .extend(activity.outputs().iter().map(|output| {
                RevisionEventKind::ActivityOutputAdded {
                    activity_id: activity.id(),
                    representation_id: output.representation_id(),
                    role: output.role().cloned(),
                }
            }));
        Ok(())
    }

    /// Replaces one representation's complete dependency observation.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] for an absent source or target, an
    /// invalid-argument error for inconsistent source membership or resolution,
    /// or a transaction/storage error. An identical current set is a no-op.
    pub fn record_dependency_set(
        &mut self,
        representation_id: RepresentationId,
        dependencies: &[Dependency],
    ) -> Result<bool> {
        if dependencies.len() > MAX_DEPENDENCIES_PER_SET {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("dependency set must contain at most {MAX_DEPENDENCIES_PER_SET} edges"),
            ));
        }
        let transaction = self.open_transaction()?;
        if !representation_exists(transaction, representation_id)? {
            return Err(Error::new(
                ErrorKind::NotFound,
                "dependency source representation does not exist",
            ));
        }
        if load_dependency_set(transaction, representation_id)?.is_some_and(|set| {
            set.status() == DependencySetStatus::Current && set.dependencies() == dependencies
        }) {
            return Ok(false);
        }
        for dependency in dependencies {
            validate_dependency_references(transaction, representation_id, dependency)?;
        }

        transaction
            .execute_batch("SAVEPOINT record_dependency_set")
            .map_err(mutation_error("begin dependency-set replacement"))?;
        let result = persist_dependency_set(transaction, representation_id, dependencies);
        if let Err(error) = result {
            transaction
                .execute_batch("ROLLBACK TO record_dependency_set; RELEASE record_dependency_set")
                .map_err(mutation_error("roll back dependency-set replacement"))?;
            return Err(error);
        }
        transaction
            .execute_batch("RELEASE record_dependency_set")
            .map_err(mutation_error("finish dependency-set replacement"))?;
        self.pending_events
            .push(RevisionEventKind::DependencySetRecorded { representation_id });
        self.record_conflict_key(SemanticConflictKey::DependencySet(representation_id))?;
        Ok(true)
    }

    /// Atomically commits all staged mutations.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Conflict`] if already closed, or
    /// [`ErrorKind::Storage`] if SQLite cannot commit.
    pub fn commit(&mut self) -> Result<()> {
        self.commit_with_receipt().map(|_| ())
    }

    /// Commits atomically and returns the revision produced by this transaction.
    ///
    /// An absent revision means no revision was created, even in a nonempty
    /// production. Every commit attempt is terminal; retry uses a new transaction.
    ///
    /// # Errors
    ///
    /// Returns structured semantic conflicts, closed-state or storage errors.
    pub fn commit_with_receipt(&mut self) -> Result<CommitReceipt> {
        self.lifecycle.ensure_open()?;
        let result = self.commit_open();
        if result.is_err() {
            // Dropping an outstanding SQLite transaction rolls it back even
            // when journal preparation failed before the commit was attempted.
            self.transaction.take();
            if self.lifecycle.state() == TransactionState::Open {
                self.lifecycle.mark_rolled_back()?;
            }
            self.pending_roots.clear();
            self.pending_events.clear();
            self.pending_conflict_keys.clear();
            self.pending_changed_keys.clear();
        }
        result
    }

    fn commit_open(&mut self) -> Result<CommitReceipt> {
        let conflict_keys = self.pending_conflict_keys.clone();
        let changed_keys = self.pending_changed_keys.clone();
        if let Some(base_revision) = self.base_revision {
            let conflict =
                find_transaction_conflict(self.open_transaction()?, base_revision, &conflict_keys)?;
            if let Some(conflict) = conflict {
                let kind = conflict.key().kind().as_str();
                return Err(Error::transaction_conflict(
                    conflict,
                    format!("semantic fact changed after transaction base: {kind}"),
                ));
            }
        }
        let mut revision = None;
        if !self.pending_events.is_empty() {
            let revision_id = RevisionId::new();
            let transaction_id = self.id();
            let committed_at = Timestamp::now()?;
            let context = self.revision_context.clone();
            let events = self.pending_events.clone();
            let revision_sequence = persist_revision(
                self.open_transaction()?,
                revision_id,
                transaction_id,
                committed_at,
                &context,
                &events,
            )?;
            persist_conflict_versions(
                self.open_transaction()?,
                &changed_keys,
                revision_id,
                revision_sequence,
            )?;
            revision = Some(Revision::new(
                revision_id,
                revision_sequence,
                transaction_id,
                committed_at,
                context.origin().cloned(),
                context.message().map(str::to_owned),
            )?);
        } else if !changed_keys.is_empty() {
            return Err(Error::new(
                ErrorKind::Internal,
                "semantic conflict keys require a revision event",
            ));
        }
        let transaction = self.take_transaction()?;
        if let Err(error) = transaction.commit() {
            self.lifecycle.mark_rolled_back()?;
            return Err(sqlite_error("commit domain transaction")(error));
        }
        self.lifecycle.mark_committed()?;
        self.production
            .set_media_roots(std::mem::take(&mut self.pending_roots));
        if !self.pending_events.is_empty() {
            self.revision_signal.notify_commit();
        }
        self.pending_events.clear();
        self.pending_conflict_keys.clear();
        self.pending_changed_keys.clear();
        Ok(CommitReceipt::new(self.production.id(), revision))
    }

    /// Explicitly discards all staged mutations.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Conflict`] if already closed, or
    /// [`ErrorKind::Storage`] if SQLite cannot roll back.
    pub fn rollback(&mut self) -> Result<()> {
        self.lifecycle.ensure_open()?;
        let transaction = self.take_transaction()?;
        transaction
            .rollback()
            .map_err(sqlite_error("roll back domain transaction"))?;
        self.lifecycle.mark_rolled_back()?;
        self.pending_roots.clear();
        self.pending_events.clear();
        self.pending_conflict_keys.clear();
        self.pending_changed_keys.clear();
        Ok(())
    }

    fn record_conflict_key(&mut self, key: SemanticConflictKey) -> Result<()> {
        let encoded = encode_conflict_key(&key)?;
        self.pending_changed_keys
            .insert(encoded.clone(), key.clone());
        self.pending_conflict_keys.insert(encoded, key);
        Ok(())
    }

    fn record_changed_key(&mut self, key: SemanticConflictKey) -> Result<()> {
        let encoded = encode_conflict_key(&key)?;
        self.pending_changed_keys.insert(encoded, key);
        Ok(())
    }

    fn require_decision_base(&self) -> Result<()> {
        self.lifecycle.ensure_open()?;
        if self.base_revision.is_none() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "non-additive mutation requires a decision base",
            ));
        }
        Ok(())
    }

    fn open_transaction(&mut self) -> Result<&Transaction<'production>> {
        self.lifecycle.ensure_open()?;
        self.transaction.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::Internal,
                "open transaction has no SQLite transaction",
            )
        })
    }

    fn take_transaction(&mut self) -> Result<Transaction<'production>> {
        self.transaction.take().ok_or_else(|| {
            Error::new(
                ErrorKind::Internal,
                "open transaction has no SQLite transaction",
            )
        })
    }
}

fn find_transaction_conflict(
    transaction: &Transaction<'_>,
    (base_revision, base_sequence): (Option<RevisionId>, u64),
    keys: &BTreeMap<Vec<u8>, SemanticConflictKey>,
) -> Result<Option<TransactionConflict>> {
    let baseline = transaction
        .query_row(
            "SELECT revision_id, revision_sequence
             FROM conflict_migration_baseline WHERE singleton = 1",
            [],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(mutation_error("load conflict migration baseline"))?;
    for (encoded, key) in keys {
        let changed = transaction
            .query_row(
                "SELECT last_changed_revision_id, last_changed_revision_sequence
                 FROM conflict_versions WHERE conflict_key = ?1",
                [encoded],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(mutation_error("load semantic conflict version"))?;
        let superseding = changed
            .as_ref()
            .or(baseline.as_ref().filter(|_| changed.is_none()));
        let Some((revision_id, sequence)) = superseding else {
            continue;
        };
        let sequence = crate::stored_u64(*sequence, "semantic conflict revision sequence")?;
        if sequence <= base_sequence {
            continue;
        }
        let revision_id = RevisionId::from_bytes(crate::id_bytes(
            revision_id.clone(),
            "semantic conflict revision",
        )?);
        return Ok(Some(TransactionConflict::new(
            key.clone(),
            base_revision,
            base_sequence,
            revision_id,
            sequence,
        )));
    }
    Ok(None)
}

fn persist_conflict_versions(
    transaction: &Transaction<'_>,
    keys: &BTreeMap<Vec<u8>, SemanticConflictKey>,
    revision_id: RevisionId,
    revision_sequence: u64,
) -> Result<()> {
    let revision_sequence = i64::try_from(revision_sequence).map_err(|error| {
        Error::new(
            ErrorKind::Unsupported,
            format!("conflict revision sequence cannot be stored: {error}"),
        )
    })?;
    for encoded in keys.keys() {
        transaction
            .execute(
                "INSERT INTO conflict_versions (
                    conflict_key, last_changed_revision_id,
                    last_changed_revision_sequence
                 ) VALUES (?1, ?2, ?3)
                 ON CONFLICT(conflict_key) DO UPDATE SET
                    last_changed_revision_id = excluded.last_changed_revision_id,
                    last_changed_revision_sequence = excluded.last_changed_revision_sequence",
                params![
                    encoded,
                    revision_id.as_bytes().as_slice(),
                    revision_sequence,
                ],
            )
            .map_err(mutation_error("persist semantic conflict version"))?;
    }
    Ok(())
}

fn encode_conflict_key(key: &SemanticConflictKey) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    match key {
        SemanticConflictKey::LocatorSet(resource_id) => {
            encoded.push(1);
            encoded.extend_from_slice(resource_id.as_bytes());
        }
        SemanticConflictKey::MetadataProperty { target, property } => {
            encoded.push(2);
            append_object_ref(&mut encoded, target)?;
            append_text(&mut encoded, property.vocabulary().as_str())?;
            append_text(&mut encoded, property.property().as_str())?;
        }
        SemanticConflictKey::DependencySet(representation_id) => {
            encoded.push(3);
            encoded.extend_from_slice(representation_id.as_bytes());
        }
        SemanticConflictKey::MediaRoot(root_id) => {
            encoded.push(4);
            encoded.extend_from_slice(root_id.as_bytes());
        }
        SemanticConflictKey::ExternalIdentifier { target, identifier } => {
            encoded.push(5);
            append_object_ref(&mut encoded, target)?;
            append_text(&mut encoded, identifier.scheme().as_str())?;
            append_text(&mut encoded, identifier.value())?;
            if let Some(qualifier) = identifier.qualifier() {
                encoded.push(1);
                append_text(&mut encoded, qualifier)?;
            } else {
                encoded.push(0);
            }
        }
        SemanticConflictKey::ResourceFingerprint {
            resource_id,
            algorithm,
            version,
        } => {
            encoded.push(6);
            encoded.extend_from_slice(resource_id.as_bytes());
            append_text(&mut encoded, algorithm)?;
            encoded.extend_from_slice(&version.to_be_bytes());
        }
        SemanticConflictKey::RepresentationFingerprint {
            representation_id,
            algorithm,
            version,
        } => {
            encoded.push(7);
            encoded.extend_from_slice(representation_id.as_bytes());
            append_text(&mut encoded, algorithm)?;
            encoded.extend_from_slice(&version.to_be_bytes());
        }
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "semantic conflict key is not supported by this storage version",
            ));
        }
    }
    Ok(encoded)
}

fn append_object_ref(encoded: &mut Vec<u8>, target: &ObjectRef) -> Result<()> {
    let (kind, id) = encode_metadata_target(target)?;
    encoded.extend_from_slice(&kind.to_be_bytes());
    encoded.extend_from_slice(id);
    Ok(())
}

fn append_text(encoded: &mut Vec<u8>, value: &str) -> Result<()> {
    let length = u32::try_from(value.len()).map_err(|error| {
        Error::new(
            ErrorKind::Unsupported,
            format!("semantic conflict key text is too long: {error}"),
        )
    })?;
    encoded.extend_from_slice(&length.to_be_bytes());
    encoded.extend_from_slice(value.as_bytes());
    Ok(())
}

struct StoredEvent<'event> {
    kind: i64,
    target_kind: Option<i64>,
    primary_id: Option<Vec<u8>>,
    secondary_id: Option<Vec<u8>>,
    structural_position: Option<i64>,
    vocabulary: Option<&'event str>,
    property: Option<&'event str>,
    identifier_scheme: Option<&'event str>,
    identifier_value: Option<&'event str>,
    identifier_qualifier: Option<&'event str>,
    activity_kind: Option<&'event str>,
    role: Option<&'event str>,
    fingerprint_algorithm: Option<&'event str>,
    fingerprint_version: Option<i64>,
}

fn persist_revision(
    transaction: &Transaction<'_>,
    revision_id: RevisionId,
    transaction_id: TransactionId,
    committed_at: Timestamp,
    context: &RevisionContext,
    events: &[RevisionEventKind],
) -> Result<u64> {
    let sequence: i64 = transaction
        .query_row(
            "SELECT coalesce(max(sequence), 0) + 1 FROM revisions",
            [],
            |row| row.get(0),
        )
        .map_err(mutation_error("allocate revision sequence"))?;
    let origin = context.origin();
    transaction
        .execute(
            "INSERT INTO revisions (
                id, sequence, transaction_id, committed_at_micros,
                origin_name, origin_version, origin_uri, message
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                revision_id.as_bytes().as_slice(),
                sequence,
                transaction_id.as_bytes().as_slice(),
                committed_at.as_unix_micros(),
                origin.map(postproject_core::OriginIdentity::name),
                origin.and_then(postproject_core::OriginIdentity::version),
                origin.and_then(postproject_core::OriginIdentity::uri),
                context.message(),
            ],
        )
        .map_err(mutation_error("persist revision"))?;

    let event_result = events.iter().enumerate().try_for_each(|(position, event)| {
        let position = i64::try_from(position).map_err(|error| {
            Error::new(
                ErrorKind::Unsupported,
                format!("revision event position cannot be stored: {error}"),
            )
        })?;
        let event = stored_event(event)?;
        transaction
            .execute(
                "INSERT INTO revision_events (
                    revision_id, position, kind, target_kind, primary_id,
                    secondary_id, structural_position, vocabulary, property,
                    identifier_scheme, identifier_value, identifier_qualifier,
                    activity_kind, role, fingerprint_algorithm, fingerprint_version
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                    ?13, ?14, ?15, ?16
                 )",
                params![
                    revision_id.as_bytes().as_slice(),
                    position,
                    event.kind,
                    event.target_kind,
                    event.primary_id,
                    event.secondary_id,
                    event.structural_position,
                    event.vocabulary,
                    event.property,
                    event.identifier_scheme,
                    event.identifier_value,
                    event.identifier_qualifier,
                    event.activity_kind,
                    event.role,
                    event.fingerprint_algorithm,
                    event.fingerprint_version,
                ],
            )
            .map(|_| ())
            .map_err(mutation_error("persist revision event"))
    });
    if let Err(error) = event_result {
        transaction
            .execute(
                "DELETE FROM revisions WHERE id = ?1",
                params![revision_id.as_bytes().as_slice()],
            )
            .map_err(mutation_error("clean failed revision"))?;
        return Err(error);
    }
    crate::stored_u64(sequence, "persisted revision sequence")
}

#[allow(
    clippy::too_many_lines,
    reason = "keeping the exhaustive semantic-event schema mapping together is auditable"
)]
fn stored_event(event: &RevisionEventKind) -> Result<StoredEvent<'_>> {
    let mut stored = StoredEvent {
        kind: 0,
        target_kind: None,
        primary_id: None,
        secondary_id: None,
        structural_position: None,
        vocabulary: None,
        property: None,
        identifier_scheme: None,
        identifier_value: None,
        identifier_qualifier: None,
        activity_kind: None,
        role: None,
        fingerprint_algorithm: None,
        fingerprint_version: None,
    };
    match event {
        RevisionEventKind::AssetImported { asset_id } => {
            stored.kind = 1;
            stored.primary_id = Some(asset_id.into_bytes().to_vec());
        }
        RevisionEventKind::RepresentationAdded {
            asset_id,
            representation_id,
        } => {
            stored.kind = 2;
            stored.primary_id = Some(representation_id.into_bytes().to_vec());
            stored.secondary_id = Some(asset_id.into_bytes().to_vec());
        }
        RevisionEventKind::ResourceAdded { resource_id } => {
            stored.kind = 3;
            stored.primary_id = Some(resource_id.into_bytes().to_vec());
        }
        RevisionEventKind::RepresentationResourceAdded {
            representation_id,
            resource_id,
            position,
        } => {
            stored.kind = 4;
            stored.primary_id = Some(representation_id.into_bytes().to_vec());
            stored.secondary_id = Some(resource_id.into_bytes().to_vec());
            stored.structural_position = Some(i64::from(*position));
        }
        RevisionEventKind::LocatorAdded {
            resource_id,
            locator_id,
        } => {
            stored.kind = 5;
            stored.primary_id = Some(locator_id.into_bytes().to_vec());
            stored.secondary_id = Some(resource_id.into_bytes().to_vec());
        }
        RevisionEventKind::MediaRootAdded { media_root_id } => {
            stored.kind = 6;
            stored.primary_id = Some(media_root_id.into_bytes().to_vec());
        }
        RevisionEventKind::LocatorRetired {
            resource_id,
            locator_id,
        } => {
            stored.kind = 14;
            stored.primary_id = Some(locator_id.into_bytes().to_vec());
            stored.secondary_id = Some(resource_id.into_bytes().to_vec());
        }
        RevisionEventKind::MediaRootEnabledChanged {
            media_root_id,
            enabled,
        } => {
            stored.kind = 15;
            stored.primary_id = Some(media_root_id.into_bytes().to_vec());
            stored.structural_position = Some(i64::from(*enabled));
        }
        RevisionEventKind::MediaRootRemoved { media_root_id } => {
            stored.kind = 16;
            stored.primary_id = Some(media_root_id.into_bytes().to_vec());
        }
        RevisionEventKind::ExternalIdentifierAdded { target, identifier }
        | RevisionEventKind::ExternalIdentifierRemoved { target, identifier } => {
            stored.kind = i64::from(matches!(
                event,
                RevisionEventKind::ExternalIdentifierRemoved { .. }
            )) + 7;
            let (target_kind, target_id) = encode_metadata_target(target)?;
            stored.target_kind = Some(target_kind);
            stored.primary_id = Some(target_id.to_vec());
            stored.identifier_scheme = Some(identifier.scheme().as_str());
            stored.identifier_value = Some(identifier.value());
            stored.identifier_qualifier = identifier.qualifier();
        }
        RevisionEventKind::MetadataAddedOrReplaced { target, property }
        | RevisionEventKind::MetadataRemoved { target, property } => {
            stored.kind = i64::from(matches!(event, RevisionEventKind::MetadataRemoved { .. })) + 9;
            let (target_kind, target_id) = encode_metadata_target(target)?;
            stored.target_kind = Some(target_kind);
            stored.primary_id = Some(target_id.to_vec());
            stored.vocabulary = Some(property.vocabulary().as_str());
            stored.property = Some(property.property().as_str());
        }
        RevisionEventKind::ActivityCreated { activity_id, kind } => {
            stored.kind = 11;
            stored.primary_id = Some(activity_id.into_bytes().to_vec());
            stored.activity_kind = Some(kind.as_str());
        }
        RevisionEventKind::ActivityInputAdded {
            activity_id,
            representation_id,
            role,
        }
        | RevisionEventKind::ActivityOutputAdded {
            activity_id,
            representation_id,
            role,
        } => {
            stored.kind = i64::from(matches!(
                event,
                RevisionEventKind::ActivityOutputAdded { .. }
            )) + 12;
            stored.primary_id = Some(activity_id.into_bytes().to_vec());
            stored.secondary_id = Some(representation_id.into_bytes().to_vec());
            stored.role = role.as_ref().map(postproject_core::ActivityRole::as_str);
        }
        RevisionEventKind::ResourceFingerprintObserved {
            resource_id,
            algorithm,
            version,
        } => {
            stored.kind = 17;
            stored.primary_id = Some(resource_id.into_bytes().to_vec());
            stored.fingerprint_algorithm = Some(algorithm);
            stored.fingerprint_version = Some(i64::from(*version));
        }
        RevisionEventKind::RepresentationFingerprintObserved {
            representation_id,
            algorithm,
            version,
        } => {
            stored.kind = 18;
            stored.primary_id = Some(representation_id.into_bytes().to_vec());
            stored.fingerprint_algorithm = Some(algorithm);
            stored.fingerprint_version = Some(i64::from(*version));
        }
        RevisionEventKind::DependencySetRecorded { representation_id } => {
            stored.kind = 19;
            stored.primary_id = Some(representation_id.into_bytes().to_vec());
        }
        RevisionEventKind::JobRequested { job_id }
        | RevisionEventKind::JobClaimed { job_id }
        | RevisionEventKind::JobClaimRenewed { job_id }
        | RevisionEventKind::JobClaimReleased { job_id }
        | RevisionEventKind::JobSucceeded { job_id }
        | RevisionEventKind::JobFailed { job_id }
        | RevisionEventKind::JobCancelled { job_id } => {
            stored.kind = match event {
                RevisionEventKind::JobRequested { .. } => 20,
                RevisionEventKind::JobClaimed { .. } => 21,
                RevisionEventKind::JobClaimRenewed { .. } => 22,
                RevisionEventKind::JobClaimReleased { .. } => 23,
                RevisionEventKind::JobSucceeded { .. } => 24,
                RevisionEventKind::JobFailed { .. } => 25,
                RevisionEventKind::JobCancelled { .. } => 26,
                _ => unreachable!("job event arm only contains job events"),
            };
            stored.primary_id = Some(job_id.into_bytes().to_vec());
        }
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "revision event kind is not supported by this schema",
            ));
        }
    }
    Ok(stored)
}

impl ProductionStoreTransaction for SqliteTransaction<'_> {
    fn id(&self) -> TransactionId {
        SqliteTransaction::id(self)
    }

    fn state(&self) -> TransactionState {
        SqliteTransaction::state(self)
    }

    fn set_revision_context(&mut self, context: RevisionContext) -> Result<()> {
        SqliteTransaction::set_revision_context(self, context)
    }

    fn import_original(&mut self, import: &OriginalMediaImport) -> Result<()> {
        SqliteTransaction::import_original(self, import)
    }

    fn add_representation(&mut self, import: &RepresentationImport) -> Result<()> {
        SqliteTransaction::add_representation(self, import)
    }

    fn add_locator(&mut self, locator: &Locator) -> Result<()> {
        SqliteTransaction::add_locator(self, locator)
    }

    fn retire_locator(&mut self, locator_id: LocatorId) -> Result<()> {
        SqliteTransaction::retire_locator(self, locator_id)
    }

    fn add_media_root(&mut self, root: MediaRoot) -> Result<()> {
        SqliteTransaction::add_media_root(self, root)
    }

    fn set_media_root_enabled(&mut self, root_id: MediaRootId, enabled: bool) -> Result<()> {
        SqliteTransaction::set_media_root_enabled(self, root_id, enabled)
    }

    fn remove_media_root(&mut self, root_id: MediaRootId) -> Result<()> {
        SqliteTransaction::remove_media_root(self, root_id)
    }

    fn add_external_identifier(
        &mut self,
        target: ObjectRef,
        identifier: &ExternalIdentifier,
    ) -> Result<()> {
        SqliteTransaction::add_external_identifier(self, target, identifier)
    }

    fn remove_external_identifier(
        &mut self,
        target: ObjectRef,
        identifier: &ExternalIdentifier,
    ) -> Result<()> {
        SqliteTransaction::remove_external_identifier(self, target, identifier)
    }

    fn add_metadata_value(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
        value: &MetadataValue,
    ) -> Result<()> {
        SqliteTransaction::add_metadata_value(self, target, property, value)
    }

    fn replace_metadata_values(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
        values: &[MetadataValue],
    ) -> Result<()> {
        SqliteTransaction::replace_metadata_values(self, target, property, values)
    }

    fn remove_metadata_property(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
    ) -> Result<()> {
        SqliteTransaction::remove_metadata_property(self, target, property)
    }

    fn create_activity(&mut self, activity: &Activity) -> Result<()> {
        SqliteTransaction::create_activity(self, activity)
    }

    fn record_dependency_set(
        &mut self,
        representation_id: RepresentationId,
        dependencies: &[Dependency],
    ) -> Result<bool> {
        SqliteTransaction::record_dependency_set(self, representation_id, dependencies)
    }

    fn record_resource_fingerprint(
        &mut self,
        resource_id: ResourceId,
        fingerprint: &ResourceFingerprint,
    ) -> Result<bool> {
        SqliteTransaction::record_resource_fingerprint(self, resource_id, fingerprint)
    }

    fn record_resource_file_facts(
        &mut self,
        resource_id: ResourceId,
        facts: FileFacts,
    ) -> Result<bool> {
        SqliteTransaction::record_resource_file_facts(self, resource_id, facts)
    }

    fn record_representation_fingerprint(
        &mut self,
        representation_id: RepresentationId,
        fingerprint: &RepresentationFingerprint,
    ) -> Result<bool> {
        SqliteTransaction::record_representation_fingerprint(self, representation_id, fingerprint)
    }

    fn request_job(&mut self, job: &Job) -> Result<()> {
        SqliteTransaction::request_job(self, job)
    }

    fn claim_job(
        &mut self,
        job_id: JobId,
        tool: &ToolIdentity,
        agent: Option<&AgentIdentity>,
        now: Timestamp,
        expires_at: Timestamp,
    ) -> Result<JobClaim> {
        SqliteTransaction::claim_job(self, job_id, tool, agent, now, expires_at)
    }

    fn renew_job_claim(
        &mut self,
        job_id: JobId,
        claim_id: JobClaimId,
        now: Timestamp,
        expires_at: Timestamp,
    ) -> Result<()> {
        SqliteTransaction::renew_job_claim(self, job_id, claim_id, now, expires_at)
    }

    fn release_job_claim(&mut self, job_id: JobId, claim_id: JobClaimId) -> Result<()> {
        SqliteTransaction::release_job_claim(self, job_id, claim_id)
    }

    fn fail_job(
        &mut self,
        job_id: JobId,
        claim_id: JobClaimId,
        now: Timestamp,
        failure: &JobFailure,
    ) -> Result<()> {
        SqliteTransaction::fail_job(self, job_id, claim_id, now, failure)
    }

    fn complete_job(
        &mut self,
        job_id: JobId,
        claim_id: JobClaimId,
        now: Timestamp,
        output: &RepresentationImport,
        activity: &Activity,
    ) -> Result<()> {
        SqliteTransaction::complete_job(self, job_id, claim_id, now, output, activity)
    }

    fn cancel_job(&mut self, job_id: JobId) -> Result<()> {
        SqliteTransaction::cancel_job(self, job_id)
    }

    fn commit(&mut self) -> Result<()> {
        SqliteTransaction::commit(self)
    }

    fn commit_with_receipt(&mut self) -> Result<CommitReceipt> {
        SqliteTransaction::commit_with_receipt(self)
    }

    fn rollback(&mut self) -> Result<()> {
        SqliteTransaction::rollback(self)
    }
}

fn encode_structure_kind(value: ContentStructureKind) -> Result<i64> {
    match value {
        ContentStructureKind::SingleResource => Ok(0),
        ContentStructureKind::ImageSequence => Ok(1),
        ContentStructureKind::OrderedParts => Ok(2),
        ContentStructureKind::Package => Ok(3),
        _ => Err(Error::new(
            ErrorKind::Unsupported,
            "content structure is not supported by this schema",
        )),
    }
}

fn encode_representation_kind(value: RepresentationKind) -> Result<i64> {
    match value {
        RepresentationKind::Original => Ok(0),
        RepresentationKind::Proxy => Ok(1),
        RepresentationKind::Optimized => Ok(2),
        RepresentationKind::Derived => Ok(3),
        _ => Err(Error::new(
            ErrorKind::Unsupported,
            "representation kind is not supported by this schema",
        )),
    }
}

fn asset_exists(
    transaction: &Transaction<'_>,
    asset_id: postproject_core::AssetId,
) -> Result<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1)",
            [asset_id.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check representation asset"))
}

fn resource_exists(transaction: &Transaction<'_>, resource_id: ResourceId) -> Result<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM resources WHERE id = ?1)",
            [resource_id.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check fingerprint resource"))
}

fn representation_exists(
    transaction: &Transaction<'_>,
    representation_id: RepresentationId,
) -> Result<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM representations WHERE id = ?1)",
            [representation_id.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check fingerprint representation"))
}

fn validate_future_job_expiry(now: Timestamp, expires_at: Timestamp) -> Result<()> {
    if expires_at <= now {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "job claim expiry must be after the caller-supplied current time",
        ));
    }
    Ok(())
}

fn job_transition_error(
    transaction: &Transaction<'_>,
    job_id: JobId,
    conflict_message: &'static str,
) -> Result<Error> {
    let exists = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM jobs WHERE id = ?1)",
            [job_id.as_bytes().as_slice()],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sqlite_error("check job transition target"))?;
    Ok(if exists {
        Error::new(ErrorKind::Conflict, conflict_message)
    } else {
        Error::new(ErrorKind::NotFound, "job does not exist")
    })
}

fn load_job_input_ids(
    transaction: &Transaction<'_>,
    job_id: JobId,
) -> Result<Vec<RepresentationId>> {
    let mut statement = transaction
        .prepare(
            "SELECT representation_id FROM job_inputs
             WHERE job_id = ?1 ORDER BY representation_id",
        )
        .map_err(sqlite_error("prepare job input validation"))?;
    statement
        .query_map([job_id.as_bytes().as_slice()], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .map_err(sqlite_error("query job input validation"))?
        .map(|row| {
            crate::id_bytes(
                row.map_err(sqlite_error("read job input validation"))?,
                "job input",
            )
            .map(RepresentationId::from_bytes)
        })
        .collect()
}

fn stored_job_invariant(message: &'static str) -> Error {
    Error::new(ErrorKind::Storage, message)
}

fn validate_dependency_references(
    transaction: &Transaction<'_>,
    source_representation_id: RepresentationId,
    dependency: &Dependency,
) -> Result<()> {
    if let Some(resource_id) = dependency.source_resource_id() {
        let is_member = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM representation_resources
                    WHERE representation_id = ?1 AND resource_id = ?2
                 )",
                params![
                    source_representation_id.as_bytes().as_slice(),
                    resource_id.as_bytes().as_slice(),
                ],
                |row| row.get::<_, bool>(0),
            )
            .map_err(mutation_error("validate dependency source resource"))?;
        if !is_member {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "dependency source resource is not a member of its representation",
            ));
        }
    }
    match dependency.target() {
        DependencyTarget::Asset(asset_id) => {
            if !asset_exists(transaction, asset_id)? {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "dependency target asset does not exist",
                ));
            }
            if let Some(resolved_id) = dependency.resolved_representation_id() {
                let belongs = transaction
                    .query_row(
                        "SELECT EXISTS(
                            SELECT 1 FROM representations
                            WHERE id = ?1 AND asset_id = ?2
                         )",
                        params![
                            resolved_id.as_bytes().as_slice(),
                            asset_id.as_bytes().as_slice(),
                        ],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(mutation_error(
                        "validate resolved dependency representation",
                    ))?;
                if !belongs {
                    return Err(Error::new(
                        ErrorKind::InvalidArgument,
                        "resolved dependency representation does not belong to target asset",
                    ));
                }
            }
        }
        DependencyTarget::Representation(target_id) => {
            if !representation_exists(transaction, target_id)? {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "dependency target representation does not exist",
                ));
            }
        }
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "dependency target kind is not supported by this schema",
            ));
        }
    }
    Ok(())
}

fn persist_dependency_set(
    transaction: &Transaction<'_>,
    representation_id: RepresentationId,
    dependencies: &[Dependency],
) -> Result<()> {
    let sequence = next_revision_sequence(transaction)?;
    transaction
        .execute(
            "INSERT INTO dependency_sets (
                source_representation_id, recorded_revision_sequence, needs_extraction
             ) VALUES (?1, ?2, 0)
             ON CONFLICT(source_representation_id) DO UPDATE SET
                recorded_revision_sequence = excluded.recorded_revision_sequence,
                needs_extraction = 0",
            params![representation_id.as_bytes().as_slice(), sequence],
        )
        .map_err(mutation_error("persist dependency-set observation"))?;
    transaction
        .execute(
            "DELETE FROM dependencies WHERE source_representation_id = ?1",
            [representation_id.as_bytes().as_slice()],
        )
        .map_err(mutation_error("replace dependency edges"))?;
    for (position, dependency) in dependencies.iter().enumerate() {
        let position = i64::try_from(position).map_err(|error| {
            Error::new(
                ErrorKind::Unsupported,
                format!("dependency position cannot be stored: {error}"),
            )
        })?;
        let (target_kind, target_id) = match dependency.target() {
            DependencyTarget::Asset(id) => (1_i64, id.into_bytes()),
            DependencyTarget::Representation(id) => (2_i64, id.into_bytes()),
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "dependency target kind is not supported by this schema",
                ));
            }
        };
        transaction
            .execute(
                "INSERT INTO dependencies (
                    source_representation_id, position, source_resource_id, kind,
                    target_kind, target_id, resolved_representation_id, required,
                    authored_reference
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    representation_id.as_bytes().as_slice(),
                    position,
                    dependency
                        .source_resource_id()
                        .map(|id| id.into_bytes().to_vec()),
                    dependency.kind().as_str(),
                    target_kind,
                    target_id.as_slice(),
                    dependency
                        .resolved_representation_id()
                        .map(|id| id.into_bytes().to_vec()),
                    dependency.is_required(),
                    dependency.authored_reference(),
                ],
            )
            .map_err(mutation_error("persist dependency edge"))?;
    }
    Ok(())
}

fn next_revision_sequence(transaction: &Transaction<'_>) -> Result<i64> {
    transaction
        .query_row(
            "SELECT coalesce(max(sequence), 0) + 1 FROM revisions",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error("allocate observation revision sequence"))
}

fn persist_edge_fingerprint_snapshot(
    transaction: &Transaction<'_>,
    snapshot_table: &'static str,
    edge_column: &'static str,
    edge_id: i64,
    representation_id: RepresentationId,
) -> Result<()> {
    let statement = format!(
        "INSERT INTO {snapshot_table} (
            {edge_column}, algorithm, algorithm_version, value,
            observed_revision_sequence
         )
         SELECT ?1, algorithm, algorithm_version, value, observed_revision_sequence
         FROM representation_fingerprints WHERE representation_id = ?2"
    );
    transaction
        .execute(
            &statement,
            params![edge_id, representation_id.as_bytes().as_slice()],
        )
        .map(|_| ())
        .map_err(mutation_error("capture activity-edge fingerprint snapshot"))
}

fn encode_availability(value: LocatorAvailability) -> Result<i64> {
    match value {
        LocatorAvailability::Unknown => Ok(0),
        LocatorAvailability::Online => Ok(1),
        LocatorAvailability::Offline => Ok(2),
        _ => Err(Error::new(
            ErrorKind::Unsupported,
            "locator availability is not supported by this schema",
        )),
    }
}

fn persist_resource(
    transaction: &Transaction<'_>,
    resource: &Resource,
    observation_sequence: i64,
) -> Result<()> {
    let size = resource
        .file_facts()
        .map(|facts| {
            i64::try_from(facts.size_bytes()).map_err(|error| {
                Error::new(
                    ErrorKind::Unsupported,
                    format!("media resource is too large for SQLite storage: {error}"),
                )
            })
        })
        .transpose()?;
    let modified_at = resource
        .file_facts()
        .and_then(postproject_core::FileFacts::modified_at)
        .map(postproject_core::Timestamp::as_unix_micros);
    transaction
        .execute(
            "INSERT INTO resources (id, file_size_bytes, modified_at_micros)
             VALUES (?1, ?2, ?3)",
            params![resource.id().as_bytes().as_slice(), size, modified_at],
        )
        .map_err(mutation_error("persist resource"))?;
    for fingerprint in resource.fingerprints() {
        transaction
            .execute(
                "INSERT INTO resource_fingerprints (
                    resource_id, algorithm, algorithm_version, value,
                    observed_revision_sequence
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    resource.id().as_bytes().as_slice(),
                    fingerprint.algorithm(),
                    fingerprint.version(),
                    fingerprint.value(),
                    observation_sequence,
                ],
            )
            .map_err(mutation_error("persist resource fingerprint"))?;
    }
    Ok(())
}

fn persist_content_structure(
    transaction: &Transaction<'_>,
    representation_id: postproject_core::RepresentationId,
    structure: &ContentStructure,
) -> Result<()> {
    let resource_ids = structure.resource_ids();
    for (position, resource_id) in resource_ids.iter().enumerate() {
        let member = structure
            .members()
            .and_then(|members| members.get(position));
        transaction
            .execute(
                "INSERT INTO representation_resources (
                    representation_id, resource_id, position, role, required
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    representation_id.as_bytes().as_slice(),
                    resource_id.as_bytes().as_slice(),
                    i64::try_from(position).map_err(|error| Error::new(
                        ErrorKind::Unsupported,
                        format!("content position is too large for SQLite: {error}"),
                    ))?,
                    member.map(|item| item.role().as_str()),
                    member.is_none_or(postproject_core::ResourceMember::is_required),
                ],
            )
            .map_err(mutation_error("persist representation resource"))?;
    }

    if let Some(sequence) = structure.image_sequence_descriptor() {
        let frames = sequence.frames();
        let rate = sequence.rate();
        transaction
            .execute(
                "INSERT INTO image_sequences (
                    representation_id, resource_id,
                    start_frame, end_frame, frame_step, rate_numerator, rate_denominator
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    representation_id.as_bytes().as_slice(),
                    sequence.resource_id().as_bytes().as_slice(),
                    frames.start(),
                    frames.end(),
                    frames.step(),
                    rate.numerator(),
                    rate.denominator(),
                ],
            )
            .map_err(mutation_error("persist image sequence"))?;
        for frame in sequence.known_missing_frames() {
            transaction
                .execute(
                    "INSERT INTO image_sequence_missing_frames (representation_id, frame)
                     VALUES (?1, ?2)",
                    params![representation_id.as_bytes().as_slice(), frame],
                )
                .map_err(mutation_error("persist missing sequence frame"))?;
        }
    }
    Ok(())
}

/// Persists a locator and its sequence naming.
///
/// A locator of an image-sequence resource must carry a naming and no other
/// locator may. A resource's locators are unique by URI and naming, so one
/// directory may be recorded under two namings.
fn persist_locator(transaction: &Transaction<'_>, locator: &Locator) -> Result<()> {
    let resource_id = *locator.resource_id().as_bytes();
    let sequence_resource: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM image_sequences WHERE resource_id = ?1)",
            [resource_id.as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check image-sequence resource"))?;
    let naming = locator.sequence_naming();
    if sequence_resource != naming.is_some() {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            if sequence_resource {
                "a locator of an image-sequence resource needs a sequence naming"
            } else {
                "only a locator of an image-sequence resource has a sequence naming"
            },
        ));
    }
    let duplicate: bool = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM locators l
                LEFT JOIN locator_sequence_namings n ON n.locator_id = l.id
                WHERE l.resource_id = ?1 AND l.uri = ?2
                  AND n.prefix IS ?3 AND n.suffix IS ?4 AND n.padding IS ?5
             )",
            params![
                resource_id.as_slice(),
                locator.uri(),
                naming.map(SequenceNaming::prefix),
                naming.map(SequenceNaming::suffix),
                naming.map(SequenceNaming::padding),
            ],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check duplicate resource locator"))?;
    if duplicate {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            "persist resource locator: the resource already has this locator",
        ));
    }
    let availability = encode_availability(locator.availability())?;
    transaction
        .execute(
            "INSERT INTO locators (
                id, resource_id, uri, last_seen_micros, availability, media_root_name
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                locator.id().as_bytes().as_slice(),
                locator.resource_id().as_bytes().as_slice(),
                locator.uri(),
                locator
                    .last_seen()
                    .map(postproject_core::Timestamp::as_unix_micros),
                availability,
                locator.media_root(),
            ],
        )
        .map_err(mutation_error("persist resource locator"))?;
    if let Some(naming) = naming {
        transaction
            .execute(
                "INSERT INTO locator_sequence_namings (locator_id, prefix, suffix, padding)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    locator.id().as_bytes().as_slice(),
                    naming.prefix(),
                    naming.suffix(),
                    naming.padding(),
                ],
            )
            .map_err(mutation_error("persist locator sequence naming"))?;
    }
    Ok(())
}

fn identifier_target_exists(
    transaction: &Transaction<'_>,
    target_kind: i64,
    target_id: &[u8; 16],
) -> Result<bool> {
    let table = match target_kind {
        1 => "assets",
        2 => "representations",
        3 => "resources",
        4 => "activities",
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "external identifier target kind is not supported by this schema",
            ));
        }
    };
    transaction
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
            [target_id.as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check external identifier target"))
}

fn ensure_metadata_target_exists(
    transaction: &Transaction<'_>,
    target_kind: i64,
    target_id: &[u8; 16],
) -> Result<()> {
    let table = match target_kind {
        0 => "productions",
        1 => "assets",
        2 => "representations",
        3 => "resources",
        4 => "activities",
        5 => "jobs",
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "metadata target kind is not supported by this schema",
            ));
        }
    };
    let exists = transaction
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
            [target_id.as_slice()],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sqlite_error("check metadata target"))?;
    if !exists {
        return Err(Error::new(
            ErrorKind::NotFound,
            "metadata target does not exist",
        ));
    }
    Ok(())
}

fn next_metadata_position(
    transaction: &Transaction<'_>,
    target_kind: i64,
    target_id: &[u8; 16],
    property: &MetadataProperty,
) -> Result<i64> {
    transaction
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0)
             FROM metadata_assertions
             WHERE target_kind = ?1 AND target_id = ?2
               AND vocabulary = ?3 AND property = ?4",
            params![
                target_kind,
                target_id.as_slice(),
                property.vocabulary().as_str(),
                property.property().as_str(),
            ],
            |row| row.get(0),
        )
        .map_err(sqlite_error("choose metadata value position"))
}

fn insert_metadata_value(
    transaction: &Transaction<'_>,
    target_kind: i64,
    target_id: &[u8; 16],
    property: &MetadataProperty,
    position: i64,
    encoded: &[u8],
) -> Result<()> {
    transaction
        .execute(
            "INSERT INTO metadata_assertions (
                target_kind, target_id, vocabulary, property, position, encoded_value
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                target_kind,
                target_id.as_slice(),
                property.vocabulary().as_str(),
                property.property().as_str(),
                position,
                encoded,
            ],
        )
        .map(|_| ())
        .map_err(mutation_error("persist metadata value"))
}

fn delete_metadata_property(
    transaction: &Transaction<'_>,
    target_kind: i64,
    target_id: &[u8; 16],
    property: &MetadataProperty,
) -> Result<usize> {
    transaction
        .execute(
            "DELETE FROM metadata_assertions
             WHERE target_kind = ?1 AND target_id = ?2
               AND vocabulary = ?3 AND property = ?4",
            params![
                target_kind,
                target_id.as_slice(),
                property.vocabulary().as_str(),
                property.property().as_str(),
            ],
        )
        .map_err(mutation_error("remove metadata property"))
}

fn mutation_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> Error {
    move |error| {
        let kind = if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
            ErrorKind::AlreadyExists
        } else {
            ErrorKind::Storage
        };
        Error::new(kind, format!("{context}: {error}"))
    }
}

fn activity_edge_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> Error {
    move |error| {
        let kind = if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
            ErrorKind::NotFound
        } else {
            ErrorKind::Storage
        };
        Error::new(kind, format!("{context}: {error}"))
    }
}

fn activity_creates_cycle(transaction: &Transaction<'_>, activity: &Activity) -> Result<bool> {
    transaction
        .query_row(
            "WITH RECURSIVE descendants(representation_id) AS (
                SELECT representation_id FROM activity_outputs WHERE activity_id = ?1
                UNION
                SELECT outputs.representation_id
                FROM descendants
                JOIN activity_inputs inputs
                  ON inputs.representation_id = descendants.representation_id
                JOIN activity_outputs outputs
                  ON outputs.activity_id = inputs.activity_id
             )
             SELECT EXISTS(
                SELECT 1 FROM descendants
                JOIN activity_inputs inputs
                  ON inputs.activity_id = ?1
                 AND inputs.representation_id = descendants.representation_id
             )",
            [activity.id().as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check activity cycle"))
}

fn delete_activity(transaction: &Transaction<'_>, activity: &Activity) -> Result<()> {
    transaction
        .execute(
            "DELETE FROM activities WHERE id = ?1",
            [activity.id().as_bytes().as_slice()],
        )
        .map(|_| ())
        .map_err(sqlite_error("discard invalid activity"))
}
