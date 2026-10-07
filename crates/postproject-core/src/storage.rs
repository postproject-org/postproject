//! Domain-shaped contracts implemented by persistence backends.

use std::time::Duration;

use crate::{
    Activity, ActivityOutputQuery, AgentIdentity, ArtifactEvaluation, ArtifactEvaluationLimits,
    ArtifactReproducibilityReport, Asset, AssetId, CommitReceipt, DecisionBase, Dependency,
    DependencyQueryLimits, DependencyQueryMatch, DependencySet, DependencyTarget,
    ExternalIdentifier, FileFacts, FilteredRevisionPage, IdentifierScheme, Job, JobFailure, JobId,
    JobQuery, KnownMediaMatch, Locator, LocatorIdentity, MediaRoot, MetadataAssertion,
    MetadataMatch, MetadataProperty, MetadataQuery, MetadataValue, ObjectRef, OriginalMediaImport,
    Production, ProductionReadSession, ProvenanceQueryLimits, ProvenanceQueryMatch, QueryPage,
    QueryPageRequest, RegenerationJobPlan, Representation, RepresentationFingerprint,
    RepresentationId, RepresentationImport, Resource, ResourceFingerprint, ResourceId, Result,
    Revision, RevisionContext, RevisionEvent, RevisionEventFilter, RevisionId, StaleArtifactQuery,
    ToolIdentity, TransactionId, TransactionState,
};

/// Read operations required from a production persistence backend.
///
/// The contract returns domain values and deliberately contains no generic CRUD,
/// query language, connection, or database-row concepts.
pub trait ProductionRead {
    /// Returns the loaded production identity and header metadata.
    fn production(&self) -> &Production;

    /// Queries a bounded page of roots in priority/identity order.
    ///
    /// Live readers load current facts; retained sessions use their pinned view.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an invalid cursor or unreadable stored data.
    fn media_roots_page(&self, page: &QueryPageRequest) -> Result<QueryPage<MediaRoot>>;

    /// Loads current roots up to [`crate::MAX_QUERY_PAGE_SIZE`].
    ///
    /// # Errors
    ///
    /// Returns an unsupported error if the result exceeds that cap; callers
    /// needing more roots use pages. Storage and cursor errors propagate.
    fn media_roots(&self) -> Result<Vec<MediaRoot>> {
        let page =
            self.media_roots_page(&QueryPageRequest::new(crate::MAX_QUERY_PAGE_SIZE, None)?)?;
        if page.next_cursor().is_some() {
            return Err(crate::Error::new(
                crate::ErrorKind::Unsupported,
                "media-root collection exceeds 1000 roots; use bounded pages",
            ));
        }
        Ok(page.into_items())
    }

    /// Loads all assets in deterministic order.
    ///
    /// Complete-list convenience is capped at 1000; use pages for larger results.
    /// It returns `Unsupported` rather than truncating the collection.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when persisted data cannot be read or
    /// decoded safely.
    fn assets(&self) -> Result<Vec<Asset>>;

    /// Queries one bounded page of assets in creation/identity order.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an invalid cursor or unreadable storage.
    fn assets_page(&self, page: &QueryPageRequest) -> Result<QueryPage<Asset>>;

    /// Loads one asset by identity.
    ///
    /// # Errors
    ///
    /// Returns a not-found error when the asset is absent, or a storage-domain
    /// error when persisted data cannot be read or decoded safely.
    fn asset(&self, asset_id: AssetId) -> Result<Asset>;

    /// Loads one representation, with its structure and fingerprints, by
    /// identity.
    ///
    /// # Errors
    ///
    /// Returns a not-found error when the representation is absent, or a
    /// storage-domain error when persisted data cannot be decoded safely.
    fn representation(&self, representation_id: RepresentationId) -> Result<Representation>;

    /// Loads every representation belonging to an asset in deterministic order.
    ///
    /// Complete-list convenience is capped at 1000; use pages for larger results.
    /// It returns `Unsupported` rather than truncating the collection.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when persisted data cannot be read or
    /// decoded safely.
    fn representations(&self, asset_id: AssetId) -> Result<Vec<Representation>>;

    /// Queries one bounded page of representations belonging to an asset.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the asset is absent, the cursor is invalid,
    /// or persisted representation data cannot be decoded safely.
    fn representations_page(
        &self,
        asset_id: AssetId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>>;

    /// Loads resources used by a representation in structural order.
    ///
    /// Complete-list convenience is capped at 1000; use pages for larger results.
    /// It returns `Unsupported` rather than truncating the collection.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when persisted data cannot be read or
    /// decoded safely.
    fn resources(&self, representation_id: RepresentationId) -> Result<Vec<Resource>>;

    /// Queries one bounded page of resources in structural order.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the representation is absent, the cursor is
    /// invalid, or persisted resource data cannot be decoded safely.
    fn resources_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Resource>>;

    /// Loads every known locator belonging to a resource.
    ///
    /// Complete-list convenience is capped at 1000; use pages for larger results.
    /// It returns `Unsupported` rather than truncating the collection.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when persisted data cannot be read or
    /// decoded safely.
    fn locators(&self, resource_id: ResourceId) -> Result<Vec<Locator>>;

    /// Queries one bounded page of locators belonging to a resource.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the resource is absent, the cursor is
    /// invalid, or persisted locator data cannot be decoded safely.
    fn locators_page(
        &self,
        resource_id: ResourceId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Locator>>;

    /// Finds current resources and their owning production objects by exact
    /// canonical locator identity.
    ///
    /// The operation is read-only. It returns every candidate and never adopts,
    /// relinks, or merges media implicitly.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an invalid cursor or unreadable storage.
    fn find_known_media_by_locator(
        &self,
        locator: &LocatorIdentity,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<KnownMediaMatch>>;

    /// Finds resources whose current effective fingerprint exactly matches,
    /// together with every owning representation and asset.
    ///
    /// Historical fingerprint observations are not searched. The operation is
    /// read-only and returns every candidate without inferring logical identity.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an invalid cursor or unreadable storage.
    fn find_known_media_by_fingerprint(
        &self,
        fingerprint: &ResourceFingerprint,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<KnownMediaMatch>>;

    /// Queries representations with a locator recorded under a logical root.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the root is absent, the cursor is invalid,
    /// or persisted representation data cannot be decoded safely.
    fn representations_under_media_root(
        &self,
        root_name: &str,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>>;

    /// Queries representations that use a resource, in identity order.
    ///
    /// A resource is usually used by one representation, but the model allows
    /// several representations to share it, so the result is a bounded page.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the resource is absent, the cursor is
    /// invalid, or persisted representation data cannot be decoded safely.
    fn representations_using_resource(
        &self,
        resource_id: ResourceId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>>;

    /// Queries representations whose required resources have no active locator.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an invalid cursor or unreadable storage.
    fn unresolved_media(&self, page: &QueryPageRequest) -> Result<QueryPage<RepresentationId>>;

    /// Loads external identifiers attached to `target` in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when persisted data cannot be read or
    /// decoded safely, or when the target kind is not supported.
    fn external_identifiers(&self, target: ObjectRef) -> Result<Vec<ExternalIdentifier>>;

    /// Finds objects carrying the exact external scheme and value.
    ///
    /// A `qualifier` restricts matches to identifiers with exactly that
    /// qualifier; `None` matches any qualifier, including none.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the lookup value or qualifier is invalid
    /// or persisted data cannot be decoded safely.
    fn find_by_external_identifier(
        &self,
        scheme: &IdentifierScheme,
        value: &str,
        qualifier: Option<&str>,
    ) -> Result<Vec<ObjectRef>>;

    /// Loads all metadata assertions attached to `target` in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the target kind is unsupported or persisted
    /// data cannot be decoded safely.
    fn metadata(&self, target: ObjectRef) -> Result<Vec<MetadataAssertion>>;

    /// Loads every ordered value for one property on `target`.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the target kind is unsupported or persisted
    /// data cannot be decoded safely.
    fn metadata_values(
        &self,
        target: ObjectRef,
        property: &MetadataProperty,
    ) -> Result<Vec<MetadataValue>>;

    /// Finds every object carrying `property`, preserving value repetition.
    ///
    /// # Errors
    ///
    /// Returns a domain error when persisted data cannot be decoded safely.
    fn query_by_metadata_property(&self, property: &MetadataProperty)
    -> Result<Vec<MetadataMatch>>;

    /// Queries one bounded page of objects carrying a metadata property.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an invalid cursor, invalid predicate, or
    /// malformed persisted metadata.
    fn metadata_query(
        &self,
        query: &MetadataQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<MetadataMatch>>;

    /// Loads all production activities in deterministic identity order.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when persisted activity data cannot be
    /// read or decoded safely.
    fn activities(&self) -> Result<Vec<Activity>>;

    /// Loads activities that produce `representation_id`.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the representation is absent or persisted
    /// activity data cannot be read safely.
    fn activities_producing(&self, representation_id: RepresentationId) -> Result<Vec<Activity>>;

    /// Loads activities that consume `representation_id`.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the representation is absent or persisted
    /// activity data cannot be read safely.
    fn activities_consuming(&self, representation_id: RepresentationId) -> Result<Vec<Activity>>;

    /// Queries activity outputs selected by exact activity or tool identity.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an invalid cursor or malformed persisted IDs.
    fn activity_outputs(
        &self,
        query: &ActivityOutputQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<RepresentationId>>;

    /// Queries a bounded page of activities producing one representation.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the representation is absent, the cursor is
    /// invalid, or activity data cannot be decoded safely.
    fn activities_producing_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Activity>>;

    /// Queries a bounded page of activities consuming one representation.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the representation is absent, the cursor is
    /// invalid, or activity data cannot be decoded safely.
    fn activities_consuming_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Activity>>;

    /// Returns every transitive provenance ancestor of `representation_id`.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the representation is absent or persisted
    /// provenance cannot be traversed safely.
    fn ancestors(&self, representation_id: RepresentationId) -> Result<Vec<RepresentationId>>;

    /// Returns every transitive provenance descendant of `representation_id`.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the representation is absent or persisted
    /// provenance cannot be traversed safely.
    fn descendants(&self, representation_id: RepresentationId) -> Result<Vec<RepresentationId>>;

    /// Queries bounded, shortest-depth provenance ancestors.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the root is absent, the cursor is invalid,
    /// or persisted provenance cannot be traversed safely.
    fn ancestors_page(
        &self,
        representation_id: RepresentationId,
        limits: ProvenanceQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ProvenanceQueryMatch>>;

    /// Queries bounded, shortest-depth provenance descendants.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the root is absent, the cursor is invalid,
    /// or persisted provenance cannot be traversed safely.
    fn descendants_page(
        &self,
        representation_id: RepresentationId,
        limits: ProvenanceQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ProvenanceQueryMatch>>;

    /// Loads the complete dependency observation for a representation.
    ///
    /// `None` means that no dependency set has been recorded. An empty set is
    /// returned as `Some` and is distinct from missing knowledge.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the representation is absent or persisted
    /// dependency data cannot be decoded safely.
    fn dependency_set(&self, representation_id: RepresentationId) -> Result<Option<DependencySet>>;

    /// Queries direct or transitive dependency targets in stable key order.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the source is absent, the cursor does not
    /// belong to the query, or persisted dependencies cannot be decoded safely.
    fn dependencies(
        &self,
        source: RepresentationId,
        limits: DependencyQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<DependencyQueryMatch>>;

    /// Queries direct or transitive dependent representations in stable order.
    ///
    /// A representation target includes pinned references and floating asset
    /// references recorded as resolved to that representation.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the target is absent or persisted IDs cannot
    /// be decoded safely.
    fn dependents(
        &self,
        target: DependencyTarget,
        limits: DependencyQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<DependencyQueryMatch>>;

    /// Evaluates whether an activity-produced representation still reflects
    /// its recorded inputs and output snapshot.
    ///
    /// This operation reads production knowledge only and never resolves or
    /// accesses media files.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the target is absent, bounds are invalid,
    /// or stored provenance cannot be decoded safely.
    fn evaluate_artifact(
        &self,
        representation_id: RepresentationId,
        limits: ArtifactEvaluationLimits,
    ) -> Result<ArtifactEvaluation>;

    /// Reports whether recorded production knowledge can reproduce an artifact.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the target is absent or stored provenance
    /// cannot be decoded safely.
    fn artifact_reproducibility(
        &self,
        representation_id: RepresentationId,
    ) -> Result<ArtifactReproducibilityReport>;

    /// Queries representations currently evaluated as stale.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the source is absent, the cursor is invalid,
    /// or artifact knowledge cannot be evaluated safely.
    fn stale_artifacts(
        &self,
        query: StaleArtifactQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<RepresentationId>>;

    /// Queries durable jobs in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when persisted job data cannot be read
    /// or decoded safely.
    fn jobs(&self, query: &JobQuery, page: &QueryPageRequest) -> Result<QueryPage<Job>>;

    /// Loads one durable job by identity.
    ///
    /// # Errors
    ///
    /// Returns a not-found error when the job is absent, or a storage-domain
    /// error when its persisted data cannot be decoded safely.
    fn job(&self, job_id: JobId) -> Result<Job>;

    /// Derives non-persisted job requests for existing artifacts.
    ///
    /// Each artifact must have exactly one producing activity. The plan copies
    /// that activity's kind, distinct input representations, and typed metadata
    /// parameters. The caller explicitly enqueues any returned plan.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the request is excessive, an artifact is
    /// absent, its producing activity is missing or ambiguous, or persisted
    /// activity/metadata data cannot be decoded safely.
    fn plan_regeneration(
        &self,
        representation_ids: &[RepresentationId],
    ) -> Result<Vec<RegenerationJobPlan>>;

    /// Returns the newest durable revision, or `None` for an empty journal.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when persisted revision data is invalid.
    fn latest_revision(&self) -> Result<Option<Revision>>;

    /// Returns revisions after `sequence` in ascending order, capped by `limit`.
    ///
    /// # Errors
    ///
    /// Returns a domain error when `limit` is zero or excessive, or when
    /// persisted revision data is invalid.
    fn changes_since(&self, sequence: u64, limit: u32) -> Result<Vec<Revision>>;

    /// Returns revisions after `sequence` that contain at least one event of
    /// the filter's types, in ascending order, capped by `limit`.
    ///
    /// The page's through sequence is the cursor for the next filtered page;
    /// every matching revision up to it is included.
    ///
    /// # Errors
    ///
    /// Returns a domain error when `limit` is zero or excessive, or when
    /// persisted revision data is invalid.
    fn changes_since_filtered(
        &self,
        sequence: u64,
        filter: &RevisionEventFilter,
        limit: u32,
    ) -> Result<FilteredRevisionPage>;

    /// Loads the semantic events for one revision in stable position order.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the revision is absent or persisted event
    /// data is invalid.
    fn events_for_revision(&self, revision_id: RevisionId) -> Result<Vec<RevisionEvent>>;

    /// Returns one bounded page of a revision's events in position order.
    ///
    /// Cursors belong to the revision, production and retained read view.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an absent revision, an invalid cursor or
    /// malformed persisted event data.
    fn events_for_revision_page(
        &self,
        revision_id: RevisionId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<RevisionEvent>>;

    /// Queries distinct metadata-capable objects touched after a revision.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an invalid cursor or malformed journal data.
    fn objects_changed_since(
        &self,
        sequence: u64,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ObjectRef>>;
}

/// Transactional mutation operations required from a persistence backend.
pub trait ProductionStoreTransaction {
    /// Backend-owned worker capability activated by this edit's commit.
    type Lease: crate::JobLease;
    /// Returns this transaction's stable identity.
    fn id(&self) -> TransactionId;

    /// Returns the current lifecycle state.
    fn state(&self) -> TransactionState;

    /// Sets the origin and message for the revision created on commit.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is already closed.
    fn set_revision_context(&mut self, context: RevisionContext) -> Result<()>;

    /// Stages one prepared original-media aggregate atomically.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed or persistence
    /// rejects the aggregate.
    fn import_original(&mut self, import: &OriginalMediaImport) -> Result<()>;

    /// Stages a representation and its newly imported resources on an existing asset.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the owning asset
    /// does not exist, or persistence rejects the aggregate.
    fn add_representation(&mut self, import: &RepresentationImport) -> Result<()>;

    /// Stages an explicitly confirmed resource locator.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed or persistence
    /// rejects the locator.
    fn add_locator(&mut self, locator: &Locator) -> Result<()>;

    /// Stages retirement of one superseded resource locator.
    ///
    /// Requires a decision base. Early rejection leaves the transaction open.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the locator does
    /// not exist, or persistence fails.
    fn retire_locator(&mut self, locator_id: crate::LocatorId) -> Result<()>;

    /// Stages a configured resolver search root.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed or persistence
    /// rejects the root.
    fn add_media_root(&mut self, root: MediaRoot) -> Result<()>;

    /// Enables or disables a configured resolver search root.
    ///
    /// Requires an edit with a decision base. Setting the existing state is an
    /// idempotent no-op; early rejection leaves the transaction open.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the root does not
    /// exist, or persistence fails.
    fn set_media_root_enabled(&mut self, root_id: crate::MediaRootId, enabled: bool) -> Result<()>;

    /// Stages removal of a configured resolver search root.
    /// Requires an edit with a decision base; early rejection leaves it open.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the root does not
    /// exist, or persistence fails.
    fn remove_media_root(&mut self, root_id: crate::MediaRootId) -> Result<()>;

    /// Stages an external identifier attachment.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the target does
    /// not exist, the attachment already exists, or persistence fails.
    fn add_external_identifier(
        &mut self,
        target: ObjectRef,
        identifier: &ExternalIdentifier,
    ) -> Result<()>;

    /// Stages removal of one exact external identifier attachment.
    ///
    /// Requires a decision base. Early rejection leaves the transaction open.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the attachment
    /// does not exist, the target kind is unsupported, or persistence fails.
    fn remove_external_identifier(
        &mut self,
        target: ObjectRef,
        identifier: &ExternalIdentifier,
    ) -> Result<()>;

    /// Appends one value to an object's metadata property.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the target does
    /// not exist or is unsupported, encoding fails, or persistence fails.
    fn add_metadata_value(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
        value: &MetadataValue,
    ) -> Result<()>;

    /// Atomically replaces all values of one metadata property.
    ///
    /// An empty value slice removes the property.
    /// Requires a decision base; unbased transactions may append values.
    ///
    /// # Errors
    ///
    /// Returns a domain error when a decision base is absent, the transaction
    /// is closed, the target is absent/unsupported, or encoding/persistence fails.
    fn replace_metadata_values(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
        values: &[MetadataValue],
    ) -> Result<()>;

    /// Removes all values of one metadata property.
    /// Requires a decision base.
    ///
    /// # Errors
    ///
    /// Returns a domain error when a decision base is absent, the transaction
    /// is closed, the property is absent, or the target/persistence fails.
    fn remove_metadata_property(
        &mut self,
        target: ObjectRef,
        property: &MetadataProperty,
    ) -> Result<()>;

    /// Stages a complete production activity with its input and output edges.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, a referenced
    /// representation is absent, the activity already exists, its edges would
    /// create a provenance cycle, or persistence fails.
    fn create_activity(&mut self, activity: &Activity) -> Result<()>;

    /// Replaces one representation's complete ordered dependency observation.
    ///
    /// Returns `true` when state changed and `false` for an identical current
    /// observation. An empty slice explicitly records a known empty set.
    /// Requires a decision base, including initial and unchanged observations;
    /// early rejection leaves the transaction open.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, a referenced
    /// object is absent or inconsistent, or persistence fails.
    fn record_dependency_set(
        &mut self,
        representation_id: RepresentationId,
        dependencies: &[Dependency],
    ) -> Result<bool>;

    /// Records a resource fingerprint as the current observation in its domain.
    ///
    /// Requires a decision base, including first and unchanged observations.
    /// Returns `true` when state changed and `false` for an identical no-op.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the resource is
    /// absent, or persistence fails.
    fn record_resource_fingerprint(
        &mut self,
        resource_id: ResourceId,
        fingerprint: &ResourceFingerprint,
    ) -> Result<bool>;

    /// Records a resource's current size and modification time as part of a
    /// content observation. Changed facts emit an event and guard the resource's
    /// file-facts conflict key. Requires a decision base; facts have no history.
    ///
    /// Returns `true` when state changed and `false` for identical facts.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the resource is
    /// absent, or persistence fails.
    fn record_resource_file_facts(
        &mut self,
        resource_id: ResourceId,
        facts: FileFacts,
    ) -> Result<bool>;

    /// Records a representation fingerprint as the current observation.
    ///
    /// Requires a decision base, including first and unchanged observations.
    /// Returns `true` when state changed and `false` for an identical no-op.
    /// A changed observation clears that representation's recomputation marker.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the
    /// representation is absent, or persistence fails.
    fn record_representation_fingerprint(
        &mut self,
        representation_id: RepresentationId,
        fingerprint: &RepresentationFingerprint,
    ) -> Result<bool>;

    /// Stages one requested job with its canonical inputs.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed, the supplied job
    /// is not requested, a referenced object is absent, the job already exists,
    /// or persistence fails.
    fn request_job(&mut self, job: &Job) -> Result<()>;

    /// Claims work for a checked duration using current store authority time.
    ///
    /// # Errors
    /// Rejects invalid durations, unavailable jobs, clock rollback or storage errors.
    fn claim_job_lease(
        &mut self,
        job: JobId,
        tool: &ToolIdentity,
        agent: Option<&AgentIdentity>,
        duration: Duration,
    ) -> Result<Self::Lease>;

    /// Renews current ownership; the edit must commit before its previous expiry.
    ///
    /// # Errors
    /// Rejects wrong scope, expired/superseded ownership, invalid duration or storage errors.
    fn renew_job_lease(&mut self, lease: &Self::Lease, duration: Duration) -> Result<()>;

    /// Releases current unexpired ownership, closing the handle on commit.
    ///
    /// # Errors
    /// Rejects wrong scope, expired/superseded ownership or storage errors.
    fn release_job_lease(&mut self, lease: &Self::Lease) -> Result<()>;

    /// Fails current unexpired ownership, closing the handle on commit.
    ///
    /// # Errors
    /// Rejects wrong scope, expired/superseded ownership or storage errors.
    fn fail_job_lease(&mut self, lease: &Self::Lease, failure: &JobFailure) -> Result<()>;

    /// Atomically publishes requested output/provenance and closes ownership.
    ///
    /// # Errors
    /// Rejects wrong scope, expired/superseded ownership, invalid facts or storage errors.
    fn complete_job_lease(
        &mut self,
        lease: &Self::Lease,
        output: &RepresentationImport,
        activity: &Activity,
    ) -> Result<()>;

    /// Cancels a requested or claimed job administratively.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the job is absent or terminal, the
    /// transaction is closed, or persistence fails.
    fn cancel_job(&mut self, job_id: JobId) -> Result<()>;

    /// Atomically makes every staged mutation durable.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed or commit fails.
    fn commit(&mut self) -> Result<()>;

    /// Commits atomically and returns only the revision this transaction made.
    ///
    /// A commit attempt is terminal, including failure. An absent revision in
    /// the receipt means that no new revision was created.
    ///
    /// # Errors
    ///
    /// Returns structured semantic conflicts, closed-state or storage errors.
    fn commit_with_receipt(&mut self) -> Result<CommitReceipt>;

    /// Explicitly discards every staged mutation.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the transaction is closed or rollback fails.
    fn rollback(&mut self) -> Result<()>;
}

/// A production persistence backend with explicit domain transactions.
pub trait ProductionStore: ProductionRead {
    /// Backend-owned worker capability shared by its domain edits.
    type Lease: crate::JobLease;

    /// Imports explicit scoped credentials against current store authority.
    ///
    /// # Errors
    /// Rejects invalid scope/encoding, expired/superseded claims or storage errors.
    fn import_job_lease(&mut self, token: &str) -> Result<Self::Lease>;
    /// Independently owned coherent read view.
    type ReadSession: ProductionReadSession;

    /// Opens a coherent view and captures its decision base.
    ///
    /// # Errors
    ///
    /// Returns storage errors or a changed-production conflict.
    fn read_session(&self) -> Result<Self::ReadSession>;

    /// Begins a short write guarded by a production-scoped decision base.
    ///
    /// # Errors
    ///
    /// Rejects wrong scope and invalid revisions, or returns storage errors.
    fn begin_edit(&mut self, base: DecisionBase) -> Result<Self::Transaction<'_>>;
    /// Backend-specific transaction implementation borrowing this store.
    type Transaction<'production>: ProductionStoreTransaction<Lease = Self::Lease>
    where
        Self: 'production;

    /// Begins a transaction for domain mutations.
    ///
    /// # Errors
    ///
    /// Returns a storage-domain error when a transaction cannot be started.
    fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>>;

    /// Begins a transaction whose decisions were made from `base_revision`.
    ///
    /// Independent additive work may still commit. A commit that touches a
    /// non-mergeable semantic fact changed after the base fails atomically
    /// with structured conflict detail.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ErrorKind::NotFound`] when the base revision does not exist,
    /// or a storage-domain error when a transaction cannot be started.
    fn begin_transaction_at(
        &mut self,
        base_revision: crate::RevisionId,
    ) -> Result<Self::Transaction<'_>>;
}
