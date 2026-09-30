//! SQLite production-file persistence for `PostProject`.
//!
//! This crate translates between domain values and a private, migrated SQLite
//! schema. SQLite types and errors are never part of the core API contract.

#![forbid(unsafe_code)]

mod artifact;
mod dependency_evaluation;
mod dependency_snapshot;
mod metadata_codec;
mod migrations;
mod query_cursor;
mod revision_wait;
mod transaction;

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use postproject_core::{
    Activity, ActivityEdgeSnapshot, ActivityId, ActivityInput, ActivityKind, ActivityOutput,
    ActivityOutputQuery, ActivityRole, AgentIdentity, ArtifactEvaluation, ArtifactEvaluationLimits,
    ArtifactKnowledgeState, ArtifactReproducibilityReport, Asset, AssetId, ContentStructure,
    Dependency, DependencyKind, DependencyQueryLimits, DependencyQueryMatch, DependencySet,
    DependencySetStatus, DependencyTarget, Error, ErrorKind, ExternalIdentifier, FileFacts,
    FilteredRevisionPage, FingerprintSnapshot, FrameRange, IdentifierScheme,
    ImageSequenceDescriptor, Job, JobClaim, JobClaimId, JobCompletion, JobFailure, JobId, JobKind,
    JobQuery, JobState, KnownMediaMatch, Locator, LocatorAvailability, LocatorId, LocatorIdentity,
    MAX_REGENERATION_PLANS, MAX_REVISION_PAGE_SIZE, MediaRoot, MediaRootId, MetadataAssertion,
    MetadataMatch, MetadataProperty, MetadataQuery, MetadataValue, ObjectRef, OriginIdentity,
    Production, ProductionId, ProductionRead, ProductionStore, PropertyId, ProvenanceQueryLimits,
    ProvenanceQueryMatch, QueryCursor, QueryPage, QueryPageRequest, RationalRate,
    RegenerationJobPlan, Representation, RepresentationFingerprint, RepresentationId,
    RepresentationKind, RequestedJobOutput, Resource, ResourceFingerprint, ResourceId,
    ResourceMember, ResourceRole, Result, Revision, RevisionEvent, RevisionEventFilter,
    RevisionEventKind, RevisionEventType, RevisionId, SequenceNaming, StaleArtifactQuery,
    Timestamp, ToolIdentity, TransactionId, VocabularyId,
};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, limits::Limit, params, params_from_iter, types::Value,
};

pub use migrations::CURRENT_SCHEMA_VERSION;
pub use revision_wait::{RevisionWaitCanceller, SqliteRevisionWaiter};
pub use transaction::SqliteTransaction;

use revision_wait::RevisionSignal;

const MAX_SQLITE_VALUE_BYTES: i32 = 16 * 1024 * 1024;

/// A production backed by one SQLite production file.
#[derive(Debug)]
pub struct SqliteProduction {
    path: PathBuf,
    connection: Connection,
    production: Production,
    revision_signal: Arc<RevisionSignal>,
}

struct StoredActivity {
    id: Vec<u8>,
    kind: String,
    started_at: Option<i64>,
    finished_at: Option<i64>,
    tool_name: Option<String>,
    tool_version: Option<String>,
    tool_uri: Option<String>,
    agent_name: Option<String>,
    agent_scheme: Option<String>,
    agent_value: Option<String>,
    agent_qualifier: Option<String>,
}

type StoredSequence = (ResourceId, i64, i64, i64, i64, i64);

struct StoredRevision {
    id: Vec<u8>,
    sequence: i64,
    transaction_id: Vec<u8>,
    committed_at: i64,
    origin_name: Option<String>,
    origin_version: Option<String>,
    origin_uri: Option<String>,
    message: Option<String>,
}

struct StoredRevisionEvent {
    position: i64,
    kind: i64,
    target_kind: Option<i64>,
    primary_id: Option<Vec<u8>>,
    secondary_id: Option<Vec<u8>>,
    structural_position: Option<i64>,
    vocabulary: Option<String>,
    property: Option<String>,
    identifier_scheme: Option<String>,
    identifier_value: Option<String>,
    identifier_qualifier: Option<String>,
    activity_kind: Option<String>,
    role: Option<String>,
    fingerprint_algorithm: Option<String>,
    fingerprint_version: Option<i64>,
}

struct StoredJob {
    id: Vec<u8>,
    kind: String,
    output_asset_id: Vec<u8>,
    output_representation_kind: i64,
    target_root: Option<String>,
    state: i64,
    claim_id: Option<Vec<u8>>,
    claim_tool_name: Option<String>,
    claim_tool_version: Option<String>,
    claim_tool_uri: Option<String>,
    claim_agent_name: Option<String>,
    claim_agent_scheme: Option<String>,
    claim_agent_value: Option<String>,
    claim_agent_qualifier: Option<String>,
    claim_expires_at: Option<i64>,
    completion_activity_id: Option<Vec<u8>>,
    completion_representation_id: Option<Vec<u8>>,
    failure_diagnostic: Option<String>,
}

struct StoredActivityEdge {
    id: i64,
    representation_id: RepresentationId,
    role: Option<ActivityRole>,
    snapshot_revision_sequence: Option<i64>,
}
type ActivityEdgesById<Edge> = BTreeMap<ActivityId, Vec<Edge>>;

impl SqliteProduction {
    /// Creates a new production file and persists its identity atomically.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::AlreadyExists`] if `path` already exists. I/O,
    /// migration, and storage failures are mapped to their domain categories.
    pub fn create(path: impl AsRef<Path>, display_name: Option<String>) -> Result<Self> {
        let path = path.as_ref();
        reserve_new_file(path)?;

        let mut connection = open_connection(path)?;
        migrations::migrate(&mut connection)?;

        let production = Production::new(
            ProductionId::new(),
            CURRENT_SCHEMA_VERSION,
            Timestamp::now()?,
            display_name,
        );
        persist_new_production(&mut connection, &production)?;

        Ok(Self::from_parts(path.to_path_buf(), connection, production))
    }

    /// Opens an existing production file, applying supported migrations first.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] if `path` does not exist. Invalid,
    /// unsupported, or inaccessible databases return a migration or storage error.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("production file does not exist: {}", path.display()),
            ));
        }

        let mut connection = open_connection(path)?;
        migrations::migrate(&mut connection)?;
        let production = load_production(&connection)?;

        Ok(Self::from_parts(path.to_path_buf(), connection, production))
    }

    fn from_parts(path: PathBuf, connection: Connection, production: Production) -> Self {
        Self {
            path,
            connection,
            production,
            revision_signal: Arc::default(),
        }
    }

    /// Returns the production-file path used by this backend.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the loaded production value.
    #[must_use]
    pub const fn production(&self) -> &Production {
        &self.production
    }

    /// Begins an explicit domain transaction for production mutations.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] if SQLite cannot start the transaction.
    pub fn begin_transaction(&mut self) -> Result<SqliteTransaction<'_>> {
        let (connection, production) = (&mut self.connection, &mut self.production);
        SqliteTransaction::begin(connection, production, &self.revision_signal)
    }

    /// Begins a transaction based on one durable production revision.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when `base_revision` is absent, or a
    /// storage error when SQLite cannot start the transaction.
    pub fn begin_transaction_at(
        &mut self,
        base_revision: RevisionId,
    ) -> Result<SqliteTransaction<'_>> {
        let (connection, production) = (&mut self.connection, &mut self.production);
        SqliteTransaction::begin_at(connection, production, &self.revision_signal, base_revision)
    }

    /// Creates a waiter for revisions committed to this production file.
    ///
    /// The waiter opens its own read connection. Commits through this
    /// production wake it immediately; commits from other processes or
    /// production handles are detected by polling the file's data version.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] when the file cannot be opened, or
    /// [`ErrorKind::Conflict`] when it no longer holds this production.
    pub fn revision_waiter(&self) -> Result<SqliteRevisionWaiter> {
        SqliteRevisionWaiter::open(
            &self.path,
            self.production.id(),
            Arc::clone(&self.revision_signal),
        )
    }

    /// Closes every revision waiter created from this production.
    ///
    /// Current and later waits on those waiters return
    /// [`postproject_core::RevisionWaitOutcome::Closed`]. Dropping the
    /// production does the same.
    pub fn close_revision_waiters(&self) {
        self.revision_signal.close();
    }

    /// Loads all assets in deterministic creation/identity order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] for query failures or invalid stored data.
    pub fn assets(&self) -> Result<Vec<Asset>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, created_at_micros, display_name, import_source
                 FROM assets ORDER BY created_at_micros, id",
            )
            .map_err(sqlite_error("prepare asset query"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(sqlite_error("query assets"))?;

        rows.map(|row| {
            let (id, created_at, display_name, import_source) =
                row.map_err(sqlite_error("read asset row"))?;
            Ok(Asset::new(
                AssetId::from_bytes(id_bytes(id, "asset")?),
                Timestamp::from_unix_micros(created_at),
                display_name,
                import_source,
            ))
        })
        .collect()
    }

    /// Loads one asset by identity.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when `asset_id` is absent, or
    /// [`ErrorKind::Storage`] for query failures or invalid stored data.
    pub fn asset(&self, asset_id: AssetId) -> Result<Asset> {
        let (id, created_at, display_name, import_source) = self
            .connection
            .query_row(
                "SELECT id, created_at_micros, display_name, import_source
                 FROM assets WHERE id = ?1",
                params![asset_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error("load asset"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "asset does not exist"))?;
        Ok(Asset::new(
            AssetId::from_bytes(id_bytes(id, "asset")?),
            Timestamp::from_unix_micros(created_at),
            display_name,
            import_source,
        ))
    }

    /// Loads one representation, with its structure and fingerprints, by
    /// identity.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when `representation_id` is absent, or
    /// [`ErrorKind::Storage`] for query failures or invalid stored data.
    pub fn representation(&self, representation_id: RepresentationId) -> Result<Representation> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM representations WHERE id = ?1)",
                [representation_id.as_bytes().as_slice()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error("check representation"))?;
        if !exists {
            return Err(Error::new(
                ErrorKind::NotFound,
                "representation does not exist",
            ));
        }
        self.load_representations_by_ids(&[representation_id.into_bytes()])?
            .pop()
            .ok_or_else(|| Error::new(ErrorKind::Storage, "representation row disappeared"))
    }

    /// Loads representations belonging to `asset_id` in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] for query failures or invalid stored data.
    pub fn representations(&self, asset_id: AssetId) -> Result<Vec<Representation>> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM representations WHERE asset_id = ?1 ORDER BY id")
            .map_err(sqlite_error("prepare representation query"))?;
        let ids = statement
            .query_map(params![asset_id.as_bytes().as_slice()], |row| {
                row.get::<_, Vec<u8>>(0)
            })
            .map_err(sqlite_error("query representations"))?
            .map(|row| {
                row.map_err(sqlite_error("read representation row"))
                    .and_then(|id| id_bytes(id, "representation"))
            })
            .collect::<Result<Vec<_>>>()?;
        self.load_representations_by_ids(&ids)
    }

    fn load_representation_by_id(
        &self,
        representation_id: RepresentationId,
    ) -> Result<Representation> {
        let stored = self
            .connection
            .query_row(
                "SELECT asset_id, kind, structure_kind FROM representations WHERE id = ?1",
                params![representation_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error("load representation"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "representation does not exist"))?;
        let asset_id = AssetId::from_bytes(id_bytes(stored.0, "asset")?);
        Ok(Representation::new(
            representation_id,
            asset_id,
            decode_representation_kind(stored.1)?,
            self.load_content_structure(representation_id, stored.2)?,
            self.load_representation_fingerprints(representation_id)?,
        ))
    }

    /// Loads resources for `representation_id` in structural order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] for query failures or invalid stored data.
    pub fn resources(&self, representation_id: RepresentationId) -> Result<Vec<Resource>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT r.id, r.file_size_bytes, r.modified_at_micros
                 FROM representation_resources rr
                 JOIN resources r ON r.id = rr.resource_id
                 WHERE rr.representation_id = ?1 ORDER BY rr.position",
            )
            .map_err(sqlite_error("prepare resource query"))?;
        let rows = statement
            .query_map(params![representation_id.as_bytes().as_slice()], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(sqlite_error("query resources"))?;

        rows.map(|row| {
            let (id, size, modified_at) = row.map_err(sqlite_error("read resource row"))?;
            let id = ResourceId::from_bytes(id_bytes(id, "resource")?);
            Ok(Resource::new(
                id,
                self.load_resource_fingerprints(id)?,
                decode_file_facts(size, modified_at)?,
            ))
        })
        .collect()
    }

    fn load_resource_by_id(&self, resource_id: ResourceId) -> Result<Resource> {
        let stored = self
            .connection
            .query_row(
                "SELECT file_size_bytes, modified_at_micros FROM resources WHERE id = ?1",
                params![resource_id.as_bytes().as_slice()],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()
            .map_err(sqlite_error("load resource"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "resource does not exist"))?;
        Ok(Resource::new(
            resource_id,
            self.load_resource_fingerprints(resource_id)?,
            decode_file_facts(stored.0, stored.1)?,
        ))
    }

    /// Loads known locators for `resource_id` in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] for query failures or invalid stored data.
    pub fn locators(&self, resource_id: ResourceId) -> Result<Vec<Locator>> {
        let mut statement = self
            .connection
            .prepare(&format!(
                "SELECT {LOCATOR_COLUMNS} WHERE l.resource_id = ?1 ORDER BY l.id"
            ))
            .map_err(sqlite_error("prepare locator query"))?;
        let rows = statement
            .query_map(
                params![resource_id.as_bytes().as_slice()],
                StoredLocator::read,
            )
            .map_err(sqlite_error("query locators"))?;

        rows.map(|row| {
            row.map_err(sqlite_error("read locator row"))?
                .into_locator(resource_id)
        })
        .collect()
    }

    /// Queries one bounded page of assets in creation/identity order.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor or unreadable stored data.
    pub fn assets_page(&self, page: &QueryPageRequest) -> Result<QueryPage<Asset>> {
        let position = query_cursor::position_fields(page, "assets", "all", 2)?
            .map(|fields| {
                let created_at = fields[0]
                    .parse::<i64>()
                    .map_err(|_| invalid_query_cursor())?;
                let id = fields[1]
                    .parse::<AssetId>()
                    .map_err(|_| invalid_query_cursor())?;
                Ok((created_at, id.into_bytes()))
            })
            .transpose()?;
        let mut parameters = Vec::<Value>::new();
        let predicate = if let Some((created_at, id)) = position {
            parameters.push(Value::Integer(created_at));
            parameters.push(Value::Blob(id.to_vec()));
            "WHERE created_at_micros > ?1 OR (created_at_micros = ?1 AND id > ?2)"
        } else {
            ""
        };
        parameters.push(Value::Integer(i64::from(page.limit()) + 1));
        let sql = format!(
            "SELECT id, created_at_micros, display_name, import_source
             FROM assets {predicate} ORDER BY created_at_micros, id LIMIT ?"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare paginated asset query"))?;
        let mut assets = statement
            .query_map(params_from_iter(parameters), |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(sqlite_error("query paginated assets"))?
            .map(|row| {
                let (id, created_at, display_name, import_source) =
                    row.map_err(sqlite_error("read paginated asset row"))?;
                Ok(Asset::new(
                    AssetId::from_bytes(id_bytes(id, "asset")?),
                    Timestamp::from_unix_micros(created_at),
                    display_name,
                    import_source,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let has_more = assets.len() > page.limit() as usize;
        assets.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            assets
                .last()
                .map(|asset| {
                    query_cursor::cursor(
                        "assets",
                        "all",
                        &[
                            asset.created_at().as_unix_micros().to_string(),
                            asset.id().to_string(),
                        ],
                    )
                })
                .transpose()?
        } else {
            None
        };
        Ok(QueryPage::new(assets, next_cursor, false))
    }

    /// Queries one bounded page of representations belonging to an asset.
    ///
    /// # Errors
    ///
    /// Returns an error when the asset is absent, the cursor is invalid, or
    /// stored representation data is malformed.
    pub fn representations_page(
        &self,
        asset_id: AssetId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>> {
        self.ensure_asset_exists(asset_id)?;
        let signature = query_cursor::signature(&[asset_id.as_bytes()]);
        let position =
            query_cursor::id_position::<RepresentationId>(page, "representations", &signature)?;
        let ids = self.query_representation_ids(
            "SELECT id FROM representations
             WHERE asset_id = ?1 AND id > ?2 ORDER BY id LIMIT ?3",
            asset_id.as_bytes(),
            position.as_ref(),
            page.limit(),
            "asset representation",
        )?;
        self.representation_page_from_ids(ids, page, "representations", &signature)
    }

    /// Queries one bounded page of resources in structural order.
    ///
    /// # Errors
    ///
    /// Returns an error when the representation is absent, the cursor is
    /// invalid, or stored resource data is malformed.
    pub fn resources_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Resource>> {
        self.ensure_representation_exists(representation_id)?;
        let signature = query_cursor::signature(&[representation_id.as_bytes()]);
        let position = query_cursor::position_fields(page, "resources", &signature, 1)?
            .map(|fields| fields[0].parse::<i64>().map_err(|_| invalid_query_cursor()))
            .transpose()?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT rr.position, r.id, r.file_size_bytes, r.modified_at_micros
                 FROM representation_resources rr
                 JOIN resources r ON r.id = rr.resource_id
                 WHERE rr.representation_id = ?1 AND rr.position > ?2
                 ORDER BY rr.position LIMIT ?3",
            )
            .map_err(sqlite_error("prepare paginated resource query"))?;
        let mut rows = statement
            .query_map(
                params![
                    representation_id.as_bytes().as_slice(),
                    position.unwrap_or(-1),
                    i64::from(page.limit()) + 1,
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                    ))
                },
            )
            .map_err(sqlite_error("query paginated resources"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_error("read paginated resource row"))?;
        let has_more = rows.len() > page.limit() as usize;
        rows.truncate(page.limit() as usize);
        let last_position = rows.last().map(|row| row.0);
        let resources = rows
            .into_iter()
            .map(|(_, id, size, modified_at)| {
                let id = ResourceId::from_bytes(id_bytes(id, "resource")?);
                Ok(Resource::new(
                    id,
                    self.load_resource_fingerprints(id)?,
                    decode_file_facts(size, modified_at)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if has_more {
            last_position
                .map(|position| {
                    query_cursor::cursor("resources", &signature, &[position.to_string()])
                })
                .transpose()?
        } else {
            None
        };
        Ok(QueryPage::new(resources, next_cursor, false))
    }

    /// Queries one bounded page of locators belonging to a resource.
    ///
    /// # Errors
    ///
    /// Returns an error when the resource is absent, the cursor is invalid, or
    /// stored locator data is malformed.
    pub fn locators_page(
        &self,
        resource_id: ResourceId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Locator>> {
        self.ensure_resource_exists(resource_id)?;
        let signature = query_cursor::signature(&[resource_id.as_bytes()]);
        let position = query_cursor::id_position::<LocatorId>(page, "locators", &signature)?;
        let mut statement = self
            .connection
            .prepare(&format!(
                "SELECT {LOCATOR_COLUMNS} WHERE l.resource_id = ?1 AND l.id > ?2
                 ORDER BY l.id LIMIT ?3"
            ))
            .map_err(sqlite_error("prepare paginated locator query"))?;
        let mut locators = statement
            .query_map(
                params![
                    resource_id.as_bytes().as_slice(),
                    position.unwrap_or([0; 16]).as_slice(),
                    i64::from(page.limit()) + 1,
                ],
                StoredLocator::read,
            )
            .map_err(sqlite_error("query paginated locators"))?
            .map(|row| {
                row.map_err(sqlite_error("read paginated locator row"))?
                    .into_locator(resource_id)
            })
            .collect::<Result<Vec<_>>>()?;
        let has_more = locators.len() > page.limit() as usize;
        locators.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            locators
                .last()
                .map(|locator| {
                    query_cursor::cursor("locators", &signature, &[locator.id().to_string()])
                })
                .transpose()?
        } else {
            None
        };
        Ok(QueryPage::new(locators, next_cursor, false))
    }

    /// Finds current resources and their owning objects by exact locator
    /// identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor or malformed stored data.
    pub fn find_known_media_by_locator(
        &self,
        locator: &LocatorIdentity,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<KnownMediaMatch>> {
        let naming = locator.sequence_naming();
        let naming_marker = if naming.is_some() { [1_u8] } else { [0_u8] };
        let prefix = naming.map_or("", SequenceNaming::prefix);
        let suffix = naming.map_or("", SequenceNaming::suffix);
        let padding = naming.map_or(0, SequenceNaming::padding);
        let padding_bytes = padding.to_be_bytes();
        let signature = query_cursor::signature(&[
            locator.uri().as_bytes(),
            &naming_marker,
            prefix.as_bytes(),
            suffix.as_bytes(),
            &padding_bytes,
        ]);
        let position = query_cursor::id_pair_position::<RepresentationId, ResourceId>(
            page,
            "known-media-locator",
            &signature,
        )?;
        let (representation_position, resource_position) = position.unwrap_or(([0; 16], [0; 16]));
        let mut statement = self
            .connection
            .prepare(
                "SELECT rr.representation_id, rr.resource_id
                 FROM locators l
                 LEFT JOIN locator_sequence_namings lsn ON lsn.locator_id = l.id
                 JOIN representation_resources rr ON rr.resource_id = l.resource_id
                 WHERE l.uri = ?1
                   AND ((?2 = 0 AND lsn.locator_id IS NULL)
                     OR (?2 = 1 AND lsn.prefix = ?3 AND lsn.suffix = ?4 AND lsn.padding = ?5))
                   AND (rr.representation_id > ?6
                     OR (rr.representation_id = ?6 AND rr.resource_id > ?7))
                 ORDER BY rr.representation_id, rr.resource_id LIMIT ?8",
            )
            .map_err(sqlite_error("prepare known-media locator query"))?;
        let ids = statement
            .query_map(
                params![
                    locator.uri(),
                    i64::from(naming.is_some()),
                    prefix,
                    suffix,
                    i64::from(padding),
                    representation_position.as_slice(),
                    resource_position.as_slice(),
                    i64::from(page.limit()) + 1,
                ],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .map_err(sqlite_error("query known media by locator"))?
            .map(|row| {
                let (representation, resource) =
                    row.map_err(sqlite_error("read known-media locator row"))?;
                Ok((
                    RepresentationId::from_bytes(id_bytes(representation, "representation")?),
                    ResourceId::from_bytes(id_bytes(resource, "resource")?),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        self.known_media_page_from_ids(ids, page, "known-media-locator", &signature)
    }

    /// Finds resources with an exact current effective fingerprint and their
    /// owning objects.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor or malformed stored data.
    pub fn find_known_media_by_fingerprint(
        &self,
        fingerprint: &ResourceFingerprint,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<KnownMediaMatch>> {
        let version_bytes = fingerprint.version().to_be_bytes();
        let signature = query_cursor::signature(&[
            fingerprint.algorithm().as_bytes(),
            &version_bytes,
            fingerprint.value(),
        ]);
        let position = query_cursor::id_pair_position::<RepresentationId, ResourceId>(
            page,
            "known-media-fingerprint",
            &signature,
        )?;
        let (representation_position, resource_position) = position.unwrap_or(([0; 16], [0; 16]));
        let mut statement = self
            .connection
            .prepare(
                "SELECT rr.representation_id, rr.resource_id
                 FROM resource_fingerprints rf
                 JOIN representation_resources rr ON rr.resource_id = rf.resource_id
                 WHERE rf.algorithm = ?1 AND rf.algorithm_version = ?2 AND rf.value = ?3
                   AND (rr.representation_id > ?4
                     OR (rr.representation_id = ?4 AND rr.resource_id > ?5))
                 ORDER BY rr.representation_id, rr.resource_id LIMIT ?6",
            )
            .map_err(sqlite_error("prepare known-media fingerprint query"))?;
        let ids = statement
            .query_map(
                params![
                    fingerprint.algorithm(),
                    fingerprint.version(),
                    fingerprint.value(),
                    representation_position.as_slice(),
                    resource_position.as_slice(),
                    i64::from(page.limit()) + 1,
                ],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .map_err(sqlite_error("query known media by fingerprint"))?
            .map(|row| {
                let (representation, resource) =
                    row.map_err(sqlite_error("read known-media fingerprint row"))?;
                Ok((
                    RepresentationId::from_bytes(id_bytes(representation, "representation")?),
                    ResourceId::from_bytes(id_bytes(resource, "resource")?),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        self.known_media_page_from_ids(ids, page, "known-media-fingerprint", &signature)
    }

    fn known_media_page_from_ids(
        &self,
        mut ids: Vec<(RepresentationId, ResourceId)>,
        page: &QueryPageRequest,
        query: &str,
        signature: &str,
    ) -> Result<QueryPage<KnownMediaMatch>> {
        let has_more = ids.len() > page.limit() as usize;
        ids.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            ids.last()
                .map(|(representation, resource)| {
                    query_cursor::cursor(
                        query,
                        signature,
                        &[representation.to_string(), resource.to_string()],
                    )
                })
                .transpose()?
        } else {
            None
        };
        let matches = ids
            .into_iter()
            .map(|(representation_id, resource_id)| {
                let representation = self.load_representation_by_id(representation_id)?;
                let asset = self.asset(representation.asset_id())?;
                let resource = self.load_resource_by_id(resource_id)?;
                Ok(KnownMediaMatch::new(asset, representation, resource))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(QueryPage::new(matches, next_cursor, false))
    }

    /// Queries representations that use a resource, in identity order.
    ///
    /// # Errors
    ///
    /// Returns an error when the resource is absent, the cursor is invalid, or
    /// stored representation data is malformed.
    pub fn representations_using_resource(
        &self,
        resource_id: ResourceId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>> {
        self.ensure_resource_exists(resource_id)?;
        let signature = query_cursor::signature(&[resource_id.as_bytes()]);
        let position = query_cursor::id_position::<RepresentationId>(
            page,
            "representations-resource",
            &signature,
        )?;
        let ids = self.query_representation_ids(
            "SELECT representation_id FROM representation_resources
             WHERE resource_id = ?1 AND representation_id > ?2
             ORDER BY representation_id LIMIT ?3",
            resource_id.as_bytes(),
            position.as_ref(),
            page.limit(),
            "resource representation",
        )?;
        self.representation_page_from_ids(ids, page, "representations-resource", &signature)
    }

    /// Queries representations with knowledge recorded under a logical root.
    ///
    /// # Errors
    ///
    /// Returns an error when the root is absent, the cursor is invalid, or
    /// stored representation data is malformed.
    pub fn representations_under_media_root(
        &self,
        root_name: &str,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>> {
        MediaRoot::validate_name(root_name)?;
        self.ensure_media_root_exists(root_name)?;
        let signature = query_cursor::signature(&[root_name.as_bytes()]);
        let position = query_cursor::id_position::<RepresentationId>(
            page,
            "representations-root",
            &signature,
        )?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT representation_id
                 FROM media_root_representations
                 WHERE media_root_name = ?1 AND representation_id > ?2
                 ORDER BY representation_id LIMIT ?3",
            )
            .map_err(sqlite_error("prepare media-root representation query"))?;
        let ids = statement
            .query_map(
                params![
                    root_name,
                    position.unwrap_or([0; 16]).as_slice(),
                    i64::from(page.limit()) + 1,
                ],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(sqlite_error("query media-root representations"))?
            .map(|row| {
                row.map_err(sqlite_error("read media-root representation row"))
                    .and_then(|id| id_bytes(id, "representation"))
            })
            .collect::<Result<Vec<_>>>()?;
        self.representation_page_from_ids(ids, page, "representations-root", &signature)
    }

    /// Queries representations whose required resources have no locator knowledge.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor or unreadable stored data.
    pub fn unresolved_media(&self, page: &QueryPageRequest) -> Result<QueryPage<RepresentationId>> {
        let position =
            query_cursor::id_position::<RepresentationId>(page, "unresolved-media", "all")?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT representation_id
                 FROM unresolved_memberships
                 WHERE representation_id > ?1
                 ORDER BY representation_id LIMIT ?2",
            )
            .map_err(sqlite_error("prepare unresolved-media query"))?;
        let mut ids = statement
            .query_map(
                params![
                    position.unwrap_or([0; 16]).as_slice(),
                    i64::from(page.limit()) + 1,
                ],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(sqlite_error("query unresolved media"))?
            .map(|row| {
                row.map_err(sqlite_error("read unresolved-media row"))
                    .and_then(|id| id_bytes(id, "representation"))
                    .map(RepresentationId::from_bytes)
            })
            .collect::<Result<Vec<_>>>()?;
        let has_more = ids.len() > page.limit() as usize;
        ids.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            ids.last()
                .map(|id| query_cursor::cursor("unresolved-media", "all", &[id.to_string()]))
                .transpose()?
        } else {
            None
        };
        Ok(QueryPage::new(ids, next_cursor, false))
    }

    fn load_content_structure(
        &self,
        representation_id: RepresentationId,
        structure_kind: i64,
    ) -> Result<ContentStructure> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT resource_id, role, required
                 FROM representation_resources
                 WHERE representation_id = ?1 ORDER BY position",
            )
            .map_err(sqlite_error("prepare content-membership query"))?;
        let rows = statement
            .query_map(params![representation_id.as_bytes().as_slice()], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            })
            .map_err(sqlite_error("query content memberships"))?;
        let rows: Vec<_> = rows
            .map(|row| {
                let (id, role, required) =
                    row.map_err(sqlite_error("read content-membership row"))?;
                Ok((
                    ResourceId::from_bytes(id_bytes(id, "resource")?),
                    role,
                    required,
                ))
            })
            .collect::<Result<_>>()?;

        match structure_kind {
            0 => match rows.as_slice() {
                [(resource_id, None, true)] => Ok(ContentStructure::single_resource(*resource_id)),
                _ => Err(stored_invariant("invalid single-resource membership")),
            },
            1 => {
                let resource_id = match rows.as_slice() {
                    [(resource_id, None, true)] => *resource_id,
                    _ => return Err(stored_invariant("invalid image-sequence membership")),
                };
                self.load_image_sequence(representation_id, resource_id)
                    .map(ContentStructure::image_sequence)
            }
            2 | 3 => {
                let members = rows
                    .into_iter()
                    .map(|(resource_id, role, required)| {
                        let role = role.ok_or_else(|| {
                            stored_invariant("compound resource membership has no role")
                        })?;
                        let role = ResourceRole::new(role)
                            .map_err(stored_domain_error("resource role"))?;
                        Ok(ResourceMember::new(resource_id, role, required))
                    })
                    .collect::<Result<_>>()?;
                if structure_kind == 2 {
                    ContentStructure::ordered_parts(members)
                } else {
                    ContentStructure::package(members)
                }
                .map_err(stored_domain_error("content structure"))
            }
            _ => Err(stored_invariant("invalid content-structure kind")),
        }
    }

    fn load_image_sequence(
        &self,
        representation_id: RepresentationId,
        resource_id: ResourceId,
    ) -> Result<ImageSequenceDescriptor> {
        let row = self
            .connection
            .query_row(
                "SELECT resource_id, start_frame, end_frame,
                        frame_step, rate_numerator, rate_denominator
                 FROM image_sequences WHERE representation_id = ?1",
                params![representation_id.as_bytes().as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .map_err(sqlite_error("load image-sequence descriptor"))?;
        let stored_resource = ResourceId::from_bytes(id_bytes(row.0, "sequence resource")?);
        if stored_resource != resource_id {
            return Err(stored_invariant(
                "image-sequence resource does not match membership",
            ));
        }
        let frames = FrameRange::new(row.1, row.2, stored_u32(row.3, "frame step")?)
            .map_err(stored_domain_error("image-sequence frame range"))?;
        let rate = RationalRate::new(
            stored_u32(row.4, "rate numerator")?,
            stored_u32(row.5, "rate denominator")?,
        )
        .map_err(stored_domain_error("image-sequence rate"))?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT frame FROM image_sequence_missing_frames
                 WHERE representation_id = ?1 ORDER BY frame",
            )
            .map_err(sqlite_error("prepare missing-frame query"))?;
        let missing = statement
            .query_map(params![representation_id.as_bytes().as_slice()], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(sqlite_error("query missing frames"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_error("read missing-frame row"))?;
        ImageSequenceDescriptor::new(resource_id, frames, rate, missing)
            .map_err(stored_domain_error("image-sequence descriptor"))
    }

    fn load_resource_fingerprints(
        &self,
        resource_id: ResourceId,
    ) -> Result<Vec<ResourceFingerprint>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT algorithm, algorithm_version, value
                 FROM resource_fingerprints
                 WHERE resource_id = ?1 ORDER BY algorithm, algorithm_version",
            )
            .map_err(sqlite_error("prepare resource-fingerprint query"))?;
        let rows = statement
            .query_map(params![resource_id.as_bytes().as_slice()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u16>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(sqlite_error("query resource fingerprints"))?;
        rows.map(|row| {
            let (algorithm, version, value) =
                row.map_err(sqlite_error("read resource-fingerprint row"))?;
            ResourceFingerprint::new(algorithm, version, value)
                .map_err(stored_domain_error("resource fingerprint"))
        })
        .collect()
    }

    fn load_representation_fingerprints(
        &self,
        representation_id: RepresentationId,
    ) -> Result<Vec<RepresentationFingerprint>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT algorithm, algorithm_version, value
                 FROM representation_fingerprints
                 WHERE representation_id = ?1 ORDER BY algorithm, algorithm_version",
            )
            .map_err(sqlite_error("prepare representation-fingerprint query"))?;
        let rows = statement
            .query_map(params![representation_id.as_bytes().as_slice()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u16>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(sqlite_error("query representation fingerprints"))?;
        rows.map(|row| {
            let (algorithm, version, value) =
                row.map_err(sqlite_error("read representation-fingerprint row"))?;
            RepresentationFingerprint::new(algorithm, version, value)
                .map_err(stored_domain_error("representation fingerprint"))
        })
        .collect()
    }

    /// Loads external identifiers attached to `target` in deterministic order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Unsupported`] for a target kind not yet persisted,
    /// or [`ErrorKind::Storage`] for query failures and malformed stored data.
    pub fn external_identifiers(&self, target: ObjectRef) -> Result<Vec<ExternalIdentifier>> {
        let (target_kind, target_id) = encode_identifier_target(&target)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT scheme, value, qualifier
                 FROM external_identifiers
                 WHERE target_kind = ?1 AND target_id = ?2
                 ORDER BY scheme, value, qualifier, id",
            )
            .map_err(sqlite_error("prepare external-identifier query"))?;
        let rows = statement
            .query_map(params![target_kind, target_id.as_slice()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(sqlite_error("query external identifiers"))?;

        rows.map(|row| {
            let (scheme, value, qualifier) =
                row.map_err(sqlite_error("read external-identifier row"))?;
            decode_external_identifier(scheme, value, qualifier)
        })
        .collect()
    }

    /// Finds objects carrying an exact external identifier scheme and value.
    ///
    /// A `qualifier` restricts matches to that exact qualifier; `None` matches
    /// any. Multiple qualifiers on one object produce that object only once.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for an invalid lookup value or
    /// qualifier, or [`ErrorKind::Storage`] for query failures and malformed
    /// target data.
    pub fn find_by_external_identifier(
        &self,
        scheme: &IdentifierScheme,
        value: &str,
        qualifier: Option<&str>,
    ) -> Result<Vec<ObjectRef>> {
        ExternalIdentifier::new(scheme.clone(), value, qualifier.map(str::to_owned))?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT target_kind, target_id
                 FROM external_identifiers
                 WHERE scheme = ?1 AND value = ?2 AND (?3 IS NULL OR qualifier = ?3)
                 ORDER BY target_kind, target_id",
            )
            .map_err(sqlite_error("prepare external-identifier lookup"))?;
        let rows = statement
            .query_map(params![scheme.as_str(), value, qualifier], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(sqlite_error("look up external identifier"))?;

        rows.map(|row| {
            let (kind, id) = row.map_err(sqlite_error("read external-identifier target"))?;
            decode_identifier_target(kind, id)
        })
        .collect()
    }

    /// Loads metadata attached to `target`, grouped by property and ordered by
    /// each value's insertion position.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] for query failures or malformed encoded
    /// values, or [`ErrorKind::Unsupported`] for an unknown target kind.
    pub fn metadata(&self, target: ObjectRef) -> Result<Vec<MetadataAssertion>> {
        let (target_kind, target_id) = encode_metadata_target(&target)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT vocabulary, property, encoded_value
                 FROM metadata_assertions
                 WHERE target_kind = ?1 AND target_id = ?2
                 ORDER BY vocabulary, property, position, id",
            )
            .map_err(sqlite_error("prepare metadata query"))?;
        let rows = statement
            .query_map(params![target_kind, target_id.as_slice()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(sqlite_error("query metadata"))?;

        rows.map(|row| {
            let (vocabulary, property, encoded) = row.map_err(sqlite_error("read metadata row"))?;
            decode_metadata_assertion(vocabulary, property, &encoded)
        })
        .collect()
    }

    /// Loads ordered repeated values for `property` on `target`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] for query failures or malformed encoded
    /// values, or [`ErrorKind::Unsupported`] for an unknown target kind.
    pub fn metadata_values(
        &self,
        target: ObjectRef,
        property: &MetadataProperty,
    ) -> Result<Vec<MetadataValue>> {
        let (target_kind, target_id) = encode_metadata_target(&target)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT encoded_value
                 FROM metadata_assertions
                 WHERE target_kind = ?1 AND target_id = ?2
                   AND vocabulary = ?3 AND property = ?4
                 ORDER BY position, id",
            )
            .map_err(sqlite_error("prepare metadata-value query"))?;
        let rows = statement
            .query_map(
                params![
                    target_kind,
                    target_id.as_slice(),
                    property.vocabulary().as_str(),
                    property.property().as_str(),
                ],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(sqlite_error("query metadata values"))?;

        rows.map(|row| {
            let encoded = row.map_err(sqlite_error("read metadata-value row"))?;
            metadata_codec::decode(&encoded)
        })
        .collect()
    }

    /// Finds every assertion using `property` in stable target/value order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] for query failures, malformed targets, or
    /// malformed encoded values.
    pub fn query_by_metadata_property(
        &self,
        property: &MetadataProperty,
    ) -> Result<Vec<MetadataMatch>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT target_kind, target_id, encoded_value
                 FROM metadata_assertions
                 WHERE vocabulary = ?1 AND property = ?2
                 ORDER BY target_kind, target_id, position, id",
            )
            .map_err(sqlite_error("prepare metadata-property lookup"))?;
        let rows = statement
            .query_map(
                params![property.vocabulary().as_str(), property.property().as_str(),],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )
            .map_err(sqlite_error("query metadata property"))?;

        rows.map(|row| {
            let (kind, id, encoded) = row.map_err(sqlite_error("read metadata match"))?;
            let target = decode_metadata_target(kind, id)?;
            let value = metadata_codec::decode(&encoded)?;
            Ok(MetadataMatch::new(
                target,
                MetadataAssertion::new(property.clone(), value),
            ))
        })
        .collect()
    }

    /// Queries one bounded page of metadata-property matches.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor, invalid predicate, or malformed
    /// persisted metadata.
    pub fn metadata_query(
        &self,
        query: &MetadataQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<MetadataMatch>> {
        let encoded_value = query
            .exact_value()
            .map(metadata_codec::encode)
            .transpose()?;
        let signature = query_cursor::signature(&[
            query.property().vocabulary().as_str().as_bytes(),
            query.property().property().as_str().as_bytes(),
            encoded_value.as_deref().unwrap_or_default(),
            &[u8::from(encoded_value.is_some())],
        ]);
        let position = query_cursor::position_fields(page, "metadata", &signature, 4)?
            .map(|fields| {
                let kind = fields[0]
                    .parse::<i64>()
                    .map_err(|_| invalid_query_cursor())?;
                let id = parse_metadata_cursor_id(kind, fields[1])?;
                let value_position = fields[2]
                    .parse::<i64>()
                    .map_err(|_| invalid_query_cursor())?;
                let row_id = fields[3]
                    .parse::<i64>()
                    .map_err(|_| invalid_query_cursor())?;
                Ok((kind, id, value_position, row_id))
            })
            .transpose()?;
        let mut clauses = vec!["vocabulary = ?", "property = ?"];
        let mut parameters = vec![
            Value::Text(query.property().vocabulary().as_str().to_owned()),
            Value::Text(query.property().property().as_str().to_owned()),
        ];
        if let Some(value) = encoded_value {
            clauses.push("encoded_value = ?");
            parameters.push(Value::Blob(value));
        }
        if let Some((kind, id, value_position, row_id)) = position {
            clauses.push("(target_kind, target_id, position, id) > (?, ?, ?, ?)");
            parameters.extend([
                Value::Integer(kind),
                Value::Blob(id.to_vec()),
                Value::Integer(value_position),
                Value::Integer(row_id),
            ]);
        }
        parameters.push(Value::Integer(i64::from(page.limit()) + 1));
        let sql = format!(
            "SELECT target_kind, target_id, position, id, encoded_value
             FROM metadata_assertions WHERE {}
             ORDER BY target_kind, target_id, position, id LIMIT ?",
            clauses.join(" AND ")
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare paginated metadata query"))?;
        let mut rows = statement
            .query_map(params_from_iter(parameters), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                ))
            })
            .map_err(sqlite_error("query paginated metadata"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_error("read paginated metadata row"))?;
        let has_more = rows.len() > page.limit() as usize;
        rows.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            rows.last()
                .map(|(kind, id, position, row_id, _)| {
                    let target = decode_metadata_target(*kind, id.clone())?;
                    query_cursor::cursor(
                        "metadata",
                        &signature,
                        &[
                            kind.to_string(),
                            object_ref_id_string(target),
                            position.to_string(),
                            row_id.to_string(),
                        ],
                    )
                })
                .transpose()?
        } else {
            None
        };
        let matches = rows
            .into_iter()
            .map(|(kind, id, _, _, encoded)| {
                let target = decode_metadata_target(kind, id)?;
                let value = metadata_codec::decode(&encoded)?;
                Ok(MetadataMatch::new(
                    target,
                    MetadataAssertion::new(query.property().clone(), value),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(QueryPage::new(matches, next_cursor, false))
    }

    /// Loads all production activities in stable identity order.
    ///
    /// Activity edges are loaded in two set-oriented queries rather than one
    /// query per activity.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] for query failures, malformed activity
    /// values, invalid edges, or orphaned edge rows.
    pub fn activities(&self) -> Result<Vec<Activity>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, kind, started_at_micros, finished_at_micros,
                        tool_name, tool_version, tool_uri, agent_name,
                        agent_identifier_scheme, agent_identifier_value,
                        agent_identifier_qualifier
                 FROM activities ORDER BY id",
            )
            .map_err(sqlite_error("prepare activity query"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(StoredActivity {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    started_at: row.get(2)?,
                    finished_at: row.get(3)?,
                    tool_name: row.get(4)?,
                    tool_version: row.get(5)?,
                    tool_uri: row.get(6)?,
                    agent_name: row.get(7)?,
                    agent_scheme: row.get(8)?,
                    agent_value: row.get(9)?,
                    agent_qualifier: row.get(10)?,
                })
            })
            .map_err(sqlite_error("query activities"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_error("read activity row"))?;
        let mut inputs = self.load_activity_inputs()?;
        let mut outputs = self.load_activity_outputs()?;
        let activities = rows
            .into_iter()
            .map(|stored| {
                let id = ActivityId::from_bytes(id_bytes(stored.id.clone(), "activity")?);
                decode_activity(
                    id,
                    stored,
                    inputs.remove(&id).unwrap_or_default(),
                    outputs.remove(&id).unwrap_or_default(),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        if !inputs.is_empty() || !outputs.is_empty() {
            return Err(stored_invariant(
                "activity edge refers to an absent activity",
            ));
        }
        Ok(activities)
    }

    /// Loads activities that produce `representation_id` in stable order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the representation is absent, or
    /// [`ErrorKind::Storage`] when activity data cannot be decoded safely.
    pub fn activities_producing(
        &self,
        representation_id: RepresentationId,
    ) -> Result<Vec<Activity>> {
        self.ensure_representation_exists(representation_id)?;
        self.activity_ids_for_relation("activity_outputs", representation_id, None, u32::MAX)?
            .into_iter()
            .map(|id| self.load_activity_by_id(ActivityId::from_bytes(id)))
            .collect()
    }

    /// Loads activities that consume `representation_id` in stable order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the representation is absent, or
    /// [`ErrorKind::Storage`] when activity data cannot be decoded safely.
    pub fn activities_consuming(
        &self,
        representation_id: RepresentationId,
    ) -> Result<Vec<Activity>> {
        self.ensure_representation_exists(representation_id)?;
        self.activity_ids_for_relation("activity_inputs", representation_id, None, u32::MAX)?
            .into_iter()
            .map(|id| self.load_activity_by_id(ActivityId::from_bytes(id)))
            .collect()
    }

    /// Queries activity outputs selected by exact activity or tool identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor or malformed persisted IDs.
    pub fn activity_outputs(
        &self,
        query: &ActivityOutputQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<RepresentationId>> {
        let (predicate, mut parameters, signature) = match query {
            ActivityOutputQuery::Kind(kind) => (
                "outputs.kind = ?".to_owned(),
                vec![Value::Text(kind.as_str().to_owned())],
                query_cursor::signature(&[b"kind", kind.as_str().as_bytes()]),
            ),
            ActivityOutputQuery::Tool(tool) => {
                let version = tool.version().unwrap_or_default();
                let uri = tool.uri().unwrap_or_default();
                (
                    "outputs.tool_name = ? AND outputs.tool_version IS ? AND outputs.tool_uri IS ?"
                        .to_owned(),
                    vec![
                        Value::Text(tool.name().to_owned()),
                        tool.version()
                            .map_or(Value::Null, |value| Value::Text(value.to_owned())),
                        tool.uri()
                            .map_or(Value::Null, |value| Value::Text(value.to_owned())),
                    ],
                    query_cursor::signature(&[
                        b"tool",
                        tool.name().as_bytes(),
                        version.as_bytes(),
                        uri.as_bytes(),
                        &[
                            u8::from(tool.version().is_some()),
                            u8::from(tool.uri().is_some()),
                        ],
                    ]),
                )
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "activity-output query is not supported by this schema",
                ));
            }
        };
        let position =
            query_cursor::id_position::<RepresentationId>(page, "activity-outputs", &signature)?;
        if let Some(position) = position {
            parameters.push(Value::Blob(position.to_vec()));
        } else {
            parameters.push(Value::Blob(vec![0; 16]));
        }
        parameters.push(Value::Integer(i64::from(page.limit()) + 1));
        let sql = format!(
            "SELECT DISTINCT outputs.representation_id
             FROM activity_output_keys outputs
             WHERE {predicate} AND outputs.representation_id > ?
             ORDER BY outputs.representation_id LIMIT ?"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare activity-output query"))?;
        let mut ids = statement
            .query_map(params_from_iter(parameters), |row| row.get::<_, Vec<u8>>(0))
            .map_err(sqlite_error("query activity outputs"))?
            .map(|row| {
                row.map_err(sqlite_error("read activity-output row"))
                    .and_then(|id| id_bytes(id, "activity output"))
                    .map(RepresentationId::from_bytes)
            })
            .collect::<Result<Vec<_>>>()?;
        id_page(&mut ids, page, "activity-outputs", &signature)
    }

    /// Queries a bounded page of producing activities.
    ///
    /// # Errors
    ///
    /// Returns an error when the representation is absent, the cursor is
    /// invalid, or stored activity data is malformed.
    pub fn activities_producing_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Activity>> {
        self.activities_for_relation_page(
            "activity_outputs",
            "activities-producing",
            representation_id,
            page,
        )
    }

    /// Queries a bounded page of consuming activities.
    ///
    /// # Errors
    ///
    /// Returns an error when the representation is absent, the cursor is
    /// invalid, or stored activity data is malformed.
    pub fn activities_consuming_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Activity>> {
        self.activities_for_relation_page(
            "activity_inputs",
            "activities-consuming",
            representation_id,
            page,
        )
    }

    /// Returns transitive input ancestry in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the representation is absent, or
    /// [`ErrorKind::Storage`] when traversal fails or stored IDs are malformed.
    pub fn ancestors(&self, representation_id: RepresentationId) -> Result<Vec<RepresentationId>> {
        self.related_representations(
            representation_id,
            "WITH RECURSIVE related(representation_id) AS (
                SELECT inputs.representation_id
                FROM activity_outputs outputs
                JOIN activity_inputs inputs ON inputs.activity_id = outputs.activity_id
                WHERE outputs.representation_id = ?1
                UNION
                SELECT inputs.representation_id
                FROM related
                JOIN activity_outputs outputs
                  ON outputs.representation_id = related.representation_id
                JOIN activity_inputs inputs ON inputs.activity_id = outputs.activity_id
             )
             SELECT representation_id FROM related ORDER BY representation_id",
            "ancestor",
        )
    }

    /// Returns transitive output descendants in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the representation is absent, or
    /// [`ErrorKind::Storage`] when traversal fails or stored IDs are malformed.
    pub fn descendants(
        &self,
        representation_id: RepresentationId,
    ) -> Result<Vec<RepresentationId>> {
        self.related_representations(
            representation_id,
            "WITH RECURSIVE related(representation_id) AS (
                SELECT outputs.representation_id
                FROM activity_inputs inputs
                JOIN activity_outputs outputs ON outputs.activity_id = inputs.activity_id
                WHERE inputs.representation_id = ?1
                UNION
                SELECT outputs.representation_id
                FROM related
                JOIN activity_inputs inputs
                  ON inputs.representation_id = related.representation_id
                JOIN activity_outputs outputs ON outputs.activity_id = inputs.activity_id
             )
             SELECT representation_id FROM related ORDER BY representation_id",
            "descendant",
        )
    }

    /// Queries bounded provenance ancestors with shortest depths.
    ///
    /// # Errors
    ///
    /// Returns an error when the root is absent, the cursor is invalid, or
    /// stored provenance cannot be traversed safely.
    pub fn ancestors_page(
        &self,
        representation_id: RepresentationId,
        limits: ProvenanceQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ProvenanceQueryMatch>> {
        self.provenance_page(representation_id, limits, page, true)
    }

    /// Queries bounded provenance descendants with shortest depths.
    ///
    /// # Errors
    ///
    /// Returns an error when the root is absent, the cursor is invalid, or
    /// stored provenance cannot be traversed safely.
    pub fn descendants_page(
        &self,
        representation_id: RepresentationId,
        limits: ProvenanceQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ProvenanceQueryMatch>> {
        self.provenance_page(representation_id, limits, page, false)
    }

    fn provenance_page(
        &self,
        representation_id: RepresentationId,
        limits: ProvenanceQueryLimits,
        page: &QueryPageRequest,
        ancestors: bool,
    ) -> Result<QueryPage<ProvenanceQueryMatch>> {
        self.ensure_representation_exists(representation_id)?;
        let direction = if ancestors {
            "ancestors"
        } else {
            "descendants"
        };
        let signature = query_cursor::signature(&[
            representation_id.as_bytes(),
            &limits.max_depth().to_be_bytes(),
            &limits.max_representations().to_be_bytes(),
        ]);
        let position = query_cursor::id_position::<RepresentationId>(page, direction, &signature)?;
        let mut visited = BTreeSet::from([representation_id]);
        let mut pending = VecDeque::from([(representation_id, 0_u32)]);
        let mut matches = BTreeMap::<RepresentationId, u32>::new();
        let mut truncated = false;
        while let Some((current, depth)) = pending.pop_front() {
            let direct = self.direct_provenance_relatives(current, ancestors)?;
            if depth == limits.max_depth() {
                truncated |= direct.iter().any(|id| !visited.contains(id));
                continue;
            }
            let next_depth = depth + 1;
            for id in direct {
                if id != representation_id {
                    matches
                        .entry(id)
                        .and_modify(|known| *known = (*known).min(next_depth))
                        .or_insert(next_depth);
                }
                if visited.contains(&id) {
                    continue;
                }
                if u32::try_from(visited.len()).unwrap_or(u32::MAX) >= limits.max_representations()
                {
                    truncated = true;
                    continue;
                }
                visited.insert(id);
                pending.push_back((id, next_depth));
            }
        }
        let mut selected = matches
            .into_iter()
            .filter(|(id, _)| position.is_none_or(|position| id.as_bytes() > &position))
            .take(page.limit() as usize + 1)
            .collect::<Vec<_>>();
        let has_more = selected.len() > page.limit() as usize;
        selected.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            selected
                .last()
                .map(|(id, _)| query_cursor::cursor(direction, &signature, &[id.to_string()]))
                .transpose()?
        } else {
            None
        };
        Ok(QueryPage::new(
            selected
                .into_iter()
                .map(|(id, depth)| ProvenanceQueryMatch::new(id, depth))
                .collect(),
            next_cursor,
            truncated,
        ))
    }

    fn direct_provenance_relatives(
        &self,
        representation_id: RepresentationId,
        ancestors: bool,
    ) -> Result<Vec<RepresentationId>> {
        let sql = if ancestors {
            "SELECT DISTINCT inputs.representation_id
             FROM activity_outputs outputs
             JOIN activity_inputs inputs ON inputs.activity_id = outputs.activity_id
             WHERE outputs.representation_id = ?1 ORDER BY inputs.representation_id"
        } else {
            "SELECT DISTINCT outputs.representation_id
             FROM activity_inputs inputs
             JOIN activity_outputs outputs ON outputs.activity_id = inputs.activity_id
             WHERE inputs.representation_id = ?1 ORDER BY outputs.representation_id"
        };
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(sqlite_error("prepare direct provenance query"))?;
        statement
            .query_map([representation_id.as_bytes().as_slice()], |row| {
                row.get::<_, Vec<u8>>(0)
            })
            .map_err(sqlite_error("query direct provenance"))?
            .map(|row| {
                row.map_err(sqlite_error("read direct provenance row"))
                    .and_then(|id| id_bytes(id, "provenance representation"))
                    .map(RepresentationId::from_bytes)
            })
            .collect()
    }

    /// Loads the complete dependency observation for `representation_id`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the source representation is absent,
    /// or a storage-domain error when persisted dependency data is malformed.
    pub fn dependency_set(
        &self,
        representation_id: RepresentationId,
    ) -> Result<Option<DependencySet>> {
        self.ensure_representation_exists(representation_id)?;
        load_dependency_set(&self.connection, representation_id)
    }

    /// Queries direct or transitive dependency targets with explicit bounds.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the source is absent,
    /// [`ErrorKind::InvalidArgument`] for a cursor from another query, or a
    /// storage-domain error when persisted dependencies are malformed.
    pub fn dependencies(
        &self,
        source: RepresentationId,
        limits: DependencyQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<DependencyQueryMatch>> {
        self.ensure_representation_exists(source)?;
        let position = query_cursor::dependency_position(page, source, limits)?;
        let mut visited = BTreeSet::from([source]);
        let mut pending = VecDeque::from([(source, 0_u32)]);
        let mut matches = BTreeMap::<(u8, [u8; 16]), (DependencyTarget, u32)>::new();
        let mut traversal_truncated = false;

        while let Some((representation_id, depth)) = pending.pop_front() {
            let dependencies = load_dependency_set(&self.connection, representation_id)?
                .map_or_else(Vec::new, |set| set.dependencies().to_vec());
            if depth == limits.max_depth() {
                traversal_truncated |= dependencies.iter().any(|dependency| {
                    let target = dependency.target();
                    let introduces_target = target != DependencyTarget::Representation(source)
                        && !matches.contains_key(&dependency_key(target));
                    let introduces_traversal = dependency_representation(dependency)
                        .is_some_and(|next| !visited.contains(&next));
                    introduces_target || introduces_traversal
                });
                continue;
            }
            let match_depth = depth + 1;
            for dependency in dependencies {
                let target = dependency.target();
                let Some(next) = dependency_representation(&dependency) else {
                    matches
                        .entry(dependency_key(target))
                        .and_modify(|(_, current_depth)| {
                            *current_depth = (*current_depth).min(match_depth);
                        })
                        .or_insert((target, match_depth));
                    continue;
                };
                if target != DependencyTarget::Representation(source) {
                    matches
                        .entry(dependency_key(target))
                        .and_modify(|(_, current_depth)| {
                            *current_depth = (*current_depth).min(match_depth);
                        })
                        .or_insert((target, match_depth));
                }
                if visited.contains(&next) {
                    continue;
                }
                if u32::try_from(visited.len()).unwrap_or(u32::MAX) >= limits.max_representations()
                {
                    traversal_truncated = true;
                    continue;
                }
                visited.insert(next);
                pending.push_back((next, match_depth));
            }
        }

        dependency_page(matches, position, page, traversal_truncated, |key| {
            query_cursor::dependency_cursor(source, limits, key)
        })
    }

    /// Loads representations that directly depend on `target`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the target is absent, or a
    /// storage-domain error when persisted IDs are malformed.
    fn direct_dependents(&self, target: DependencyTarget) -> Result<Vec<RepresentationId>> {
        let (query, target_id) = match target {
            DependencyTarget::Asset(id) => {
                let exists = self
                    .connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1)",
                        [id.as_bytes().as_slice()],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(sqlite_error("check dependency target asset"))?;
                if !exists {
                    return Err(Error::new(
                        ErrorKind::NotFound,
                        "dependency target asset does not exist",
                    ));
                }
                (
                    "SELECT DISTINCT source_representation_id FROM dependencies
                     WHERE target_kind = 1 AND target_id = ?1
                     ORDER BY source_representation_id",
                    id.into_bytes(),
                )
            }
            DependencyTarget::Representation(id) => {
                self.ensure_representation_exists(id)?;
                (
                    "SELECT DISTINCT source_representation_id FROM dependencies
                     WHERE (target_kind = 2 AND target_id = ?1)
                        OR resolved_representation_id = ?1
                     ORDER BY source_representation_id",
                    id.into_bytes(),
                )
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "dependency target kind is not supported by this schema",
                ));
            }
        };
        let mut statement = self
            .connection
            .prepare(query)
            .map_err(sqlite_error("prepare dependent query"))?;
        statement
            .query_map([target_id.as_slice()], |row| row.get::<_, Vec<u8>>(0))
            .map_err(sqlite_error("query dependents"))?
            .map(|row| {
                let id = row.map_err(sqlite_error("read dependent row"))?;
                id_bytes(id, "dependent representation").map(RepresentationId::from_bytes)
            })
            .collect()
    }

    /// Queries direct or transitive dependent representations with bounds.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the target is absent,
    /// [`ErrorKind::InvalidArgument`] for a cursor from another query, or a
    /// storage-domain error when persisted dependency IDs are malformed.
    pub fn dependents(
        &self,
        target: DependencyTarget,
        limits: DependencyQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<DependencyQueryMatch>> {
        let position = query_cursor::dependent_position(page, target, limits)?;
        let mut visited = BTreeSet::new();
        if let DependencyTarget::Representation(id) = target {
            visited.insert(id);
        }
        let mut pending = VecDeque::from([(target, 0_u32)]);
        let mut matches = BTreeMap::<(u8, [u8; 16]), (DependencyTarget, u32)>::new();
        let mut traversal_truncated = false;

        while let Some((current_target, depth)) = pending.pop_front() {
            let direct = self.direct_dependents(current_target)?;
            if depth == limits.max_depth() {
                traversal_truncated |= direct.iter().any(|id| !visited.contains(id));
                continue;
            }
            let match_depth = depth + 1;
            for source in direct {
                let source_target = DependencyTarget::Representation(source);
                if source_target != target {
                    matches
                        .entry(dependency_key(source_target))
                        .and_modify(|(_, current_depth)| {
                            *current_depth = (*current_depth).min(match_depth);
                        })
                        .or_insert((source_target, match_depth));
                }
                if visited.contains(&source) {
                    continue;
                }
                if u32::try_from(visited.len()).unwrap_or(u32::MAX) >= limits.max_representations()
                {
                    traversal_truncated = true;
                    continue;
                }
                visited.insert(source);
                pending.push_back((source_target, match_depth));
            }
        }

        dependency_page(
            matches,
            position.map(|id| (2, id)),
            page,
            traversal_truncated,
            |key| query_cursor::dependent_cursor(target, limits, key.1),
        )
    }

    /// Queries produced representations currently evaluated as stale.
    ///
    /// # Errors
    ///
    /// Returns an error when the source is absent, the cursor is invalid, or
    /// artifact knowledge cannot be evaluated safely.
    pub fn stale_artifacts(
        &self,
        query: StaleArtifactQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<RepresentationId>> {
        let limits = query.evaluation_limits();
        let source = query.source();
        if let Some(source) = source {
            self.ensure_representation_exists(source)?;
        }
        let source_text = source.map_or_else(|| "*".to_owned(), |id| id.to_string());
        let signature = query_cursor::signature(&[
            source_text.as_bytes(),
            &limits.max_depth().to_be_bytes(),
            &limits.max_representations().to_be_bytes(),
        ]);
        let position =
            query_cursor::id_position::<RepresentationId>(page, "stale-artifacts", &signature)?;
        let mut parameters = vec![Value::Blob(position.unwrap_or([0; 16]).to_vec())];
        let (restriction, traversal_truncated) = if let Some(source) = source {
            let provenance_limits = ProvenanceQueryLimits::new(
                limits
                    .max_depth()
                    .min(postproject_core::MAX_PROVENANCE_QUERY_DEPTH),
                limits
                    .max_representations()
                    .min(postproject_core::MAX_PROVENANCE_QUERY_REPRESENTATIONS),
            )?;
            let descendants = self.provenance_page(
                source,
                provenance_limits,
                &QueryPageRequest::new(postproject_core::MAX_QUERY_PAGE_SIZE, None)?,
                false,
            )?;
            let truncated = descendants.traversal_truncated()
                || descendants.next_cursor().is_some()
                || limits.max_depth() > postproject_core::MAX_PROVENANCE_QUERY_DEPTH
                || limits.max_representations()
                    > postproject_core::MAX_PROVENANCE_QUERY_REPRESENTATIONS;
            let ids = descendants
                .items()
                .iter()
                .map(|item| item.representation_id().into_bytes())
                .collect::<Vec<_>>();
            if ids.is_empty() {
                return Ok(QueryPage::new(Vec::new(), None, truncated));
            }
            parameters.extend(ids.iter().map(|id| Value::Blob(id.to_vec())));
            let placeholders = std::iter::repeat_n("?", ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            (
                format!("AND outputs.representation_id IN ({placeholders})"),
                truncated,
            )
        } else {
            (String::new(), false)
        };
        parameters.push(Value::Integer(i64::from(page.limit()) + 1));
        let sql = format!(
            "SELECT DISTINCT outputs.representation_id
             FROM activity_outputs outputs
             WHERE outputs.representation_id > ? {restriction}
             ORDER BY outputs.representation_id LIMIT ?"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare stale-artifact query"))?;
        let mut candidates = statement
            .query_map(params_from_iter(parameters), |row| row.get::<_, Vec<u8>>(0))
            .map_err(sqlite_error("query stale artifacts"))?
            .map(|row| {
                row.map_err(sqlite_error("read stale-artifact row"))
                    .and_then(|id| id_bytes(id, "stale artifact"))
                    .map(RepresentationId::from_bytes)
            })
            .collect::<Result<Vec<_>>>()?;
        let has_more = candidates.len() > page.limit() as usize;
        candidates.truncate(page.limit() as usize);
        let last_examined = candidates.last().copied();
        let stale = candidates
            .into_iter()
            .map(|id| Ok((id, self.evaluate_artifact(id, limits)?)))
            .filter_map(|result: Result<_>| match result {
                Ok((id, evaluation)) if evaluation.state() == ArtifactKnowledgeState::Stale => {
                    Some(Ok(id))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if has_more {
            last_examined
                .map(|id| query_cursor::cursor("stale-artifacts", &signature, &[id.to_string()]))
                .transpose()?
        } else {
            None
        };
        Ok(QueryPage::new(stale, next_cursor, traversal_truncated))
    }

    /// Queries durable jobs in stable identity order with optional exact predicates.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for a cursor from another query,
    /// or [`ErrorKind::Storage`] when persisted job data is malformed.
    pub fn jobs(&self, query: &JobQuery, page: &QueryPageRequest) -> Result<QueryPage<Job>> {
        let position = query_cursor::job_position(page, query)?;
        let mut clauses = Vec::new();
        let mut parameters = Vec::<Value>::new();
        if let Some(state) = query.state() {
            clauses.push("state = ?");
            parameters.push(Value::Integer(i64::from(query_cursor::job_state_code(
                state,
            ))));
        }
        if let Some(kind) = query.kind() {
            clauses.push("kind = ?");
            parameters.push(Value::Text(kind.as_str().to_owned()));
        }
        if let Some(position) = position {
            clauses.push("id > ?");
            parameters.push(Value::Blob(position.to_vec()));
        }
        let where_clause = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        parameters.push(Value::Integer(i64::from(page.limit()) + 1));
        let sql = format!(
            "SELECT id, kind, output_asset_id, output_representation_kind,
                    target_root, state, claim_id, claim_tool_name,
                    claim_tool_version, claim_tool_uri, claim_agent_name,
                    claim_agent_scheme, claim_agent_value, claim_agent_qualifier,
                    claim_expires_at_micros, completion_activity_id,
                    completion_representation_id, failure_diagnostic
             FROM jobs{where_clause} ORDER BY id LIMIT ?"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare paginated job query"))?;
        let mut stored = statement
            .query_map(params_from_iter(parameters), stored_job_row)
            .map_err(sqlite_error("query paginated jobs"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_error("read paginated job row"))?;
        let has_more = stored.len() > page.limit() as usize;
        stored.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            stored
                .last()
                .map(|job| {
                    id_bytes(job.id.clone(), "job")
                        .and_then(|id| query_cursor::job_cursor(query, id))
                })
                .transpose()?
        } else {
            None
        };
        let jobs = stored
            .into_iter()
            .map(|job| decode_job(&self.connection, job))
            .collect::<Result<Vec<_>>>()?;
        Ok(QueryPage::new(jobs, next_cursor, false))
    }

    /// Loads one durable job by identity.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when `job_id` is absent, or
    /// [`ErrorKind::Storage`] when persisted job data is malformed.
    pub fn job(&self, job_id: JobId) -> Result<Job> {
        let stored = self
            .connection
            .query_row(
                "SELECT id, kind, output_asset_id, output_representation_kind,
                        target_root, state, claim_id, claim_tool_name,
                        claim_tool_version, claim_tool_uri, claim_agent_name,
                        claim_agent_scheme, claim_agent_value, claim_agent_qualifier,
                        claim_expires_at_micros, completion_activity_id,
                        completion_representation_id, failure_diagnostic
                 FROM jobs WHERE id = ?1",
                [job_id.as_bytes().as_slice()],
                stored_job_row,
            )
            .optional()
            .map_err(sqlite_error("load job"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "job does not exist"))?;
        decode_job(&self.connection, stored)
    }

    /// Derives non-persisted requests that would regenerate existing artifacts.
    ///
    /// When the producing activity completed a job, the plan repeats that job's
    /// kind and target root; otherwise it uses the activity's kind and no target
    /// root. Inputs come from the activity and parameters from its metadata,
    /// because the activity records what the worker used.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for an excessive request,
    /// [`ErrorKind::NotFound`] for an absent artifact, [`ErrorKind::Conflict`]
    /// when an artifact lacks exactly one producer, or a storage-domain error.
    pub fn plan_regeneration(
        &self,
        representation_ids: &[RepresentationId],
    ) -> Result<Vec<RegenerationJobPlan>> {
        if representation_ids.len() > MAX_REGENERATION_PLANS {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("regeneration planning accepts at most {MAX_REGENERATION_PLANS} artifacts"),
            ));
        }
        let mut representation_ids = representation_ids.to_vec();
        representation_ids.sort_unstable();
        representation_ids.dedup();
        representation_ids
            .into_iter()
            .map(|representation_id| {
                let representation = self.load_representation_by_id(representation_id)?;
                let producers = self.activities_producing(representation_id)?;
                let [activity] = producers.as_slice() else {
                    return Err(Error::new(
                        ErrorKind::Conflict,
                        format!(
                            "artifact {representation_id} must have exactly one producing activity"
                        ),
                    ));
                };
                let mut inputs = activity
                    .inputs()
                    .iter()
                    .map(ActivityInput::representation_id)
                    .collect::<Vec<_>>();
                inputs.sort_unstable();
                inputs.dedup();
                let completed = self.job_completed_by(activity.id())?;
                let (kind, target_root) = match completed {
                    Some((kind, target_root)) => (kind, target_root),
                    None => (activity.kind().as_str().to_owned(), None),
                };
                let output = RequestedJobOutput::new(
                    representation.asset_id(),
                    representation.kind(),
                    target_root,
                )?;
                let job = Job::new(JobId::new(), JobKind::new(kind)?, inputs, output)?;
                let parameters = self.metadata(ObjectRef::Activity(activity.id()))?;
                Ok(RegenerationJobPlan::new(representation_id, job, parameters))
            })
            .collect()
    }

    /// Returns the kind and target root of the job an activity completed, if any.
    fn job_completed_by(
        &self,
        activity_id: ActivityId,
    ) -> Result<Option<(String, Option<String>)>> {
        self.connection
            .query_row(
                "SELECT kind, target_root FROM jobs
                 WHERE completion_activity_id = ?1
                 ORDER BY id
                 LIMIT 1",
                params![activity_id.as_bytes().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sqlite_error("load the job an activity completed"))
    }

    /// Returns the newest durable revision, if the journal is non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] when persisted revision data is malformed
    /// or cannot be read.
    pub fn latest_revision(&self) -> Result<Option<Revision>> {
        self.connection
            .query_row(
                "SELECT id, sequence, transaction_id, committed_at_micros,
                        origin_name, origin_version, origin_uri, message
                 FROM revisions ORDER BY sequence DESC LIMIT 1",
                [],
                stored_revision_row,
            )
            .optional()
            .map_err(sqlite_error("query latest revision"))?
            .map(decode_revision)
            .transpose()
    }

    /// Returns a bounded ascending page of revisions after `sequence`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when `limit` is zero or exceeds
    /// [`MAX_REVISION_PAGE_SIZE`], or [`ErrorKind::Storage`] for invalid data.
    pub fn changes_since(&self, sequence: u64, limit: u32) -> Result<Vec<Revision>> {
        if limit == 0 || limit > MAX_REVISION_PAGE_SIZE {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("revision page limit must be 1-{MAX_REVISION_PAGE_SIZE}"),
            ));
        }
        let Ok(sequence) = i64::try_from(sequence) else {
            return Ok(Vec::new());
        };
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, sequence, transaction_id, committed_at_micros,
                        origin_name, origin_version, origin_uri, message
                 FROM revisions WHERE sequence > ?1
                 ORDER BY sequence LIMIT ?2",
            )
            .map_err(sqlite_error("prepare revision page query"))?;
        statement
            .query_map(params![sequence, i64::from(limit)], stored_revision_row)
            .map_err(sqlite_error("query revision page"))?
            .map(|row| {
                row.map_err(sqlite_error("read revision row"))
                    .and_then(decode_revision)
            })
            .collect()
    }

    /// Returns a bounded ascending page of revisions after `sequence` that
    /// contain at least one event of the filter's types.
    ///
    /// The page's through sequence is the next cursor: a full page ends at its
    /// last revision, and a short page ends at the newest revision read in the
    /// same snapshot, or at `sequence` if that is newer.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when `limit` is zero or exceeds
    /// [`MAX_REVISION_PAGE_SIZE`], or [`ErrorKind::Storage`] for invalid data.
    pub fn changes_since_filtered(
        &self,
        sequence: u64,
        filter: &RevisionEventFilter,
        limit: u32,
    ) -> Result<FilteredRevisionPage> {
        if limit == 0 || limit > MAX_REVISION_PAGE_SIZE {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("revision page limit must be 1-{MAX_REVISION_PAGE_SIZE}"),
            ));
        }
        let Ok(after) = i64::try_from(sequence) else {
            return FilteredRevisionPage::new(Vec::new(), sequence);
        };
        // One bounded key range per requested kind keeps the read proportional
        // to the page rather than to the journal suffix.
        let branches = (0..filter.types().len())
            .map(|index| {
                format!(
                    "SELECT sequence FROM (
                         SELECT sequence FROM revision_event_kinds
                         WHERE kind = ?{} AND sequence > ?1
                         ORDER BY sequence LIMIT ?2
                     )",
                    index + 3
                )
            })
            .collect::<Vec<_>>()
            .join(" UNION ");
        let mut parameters = vec![Value::Integer(after), Value::Integer(i64::from(limit))];
        for event_type in filter.types() {
            parameters.push(Value::Integer(stored_revision_event_kind(event_type)?));
        }
        let snapshot = self
            .connection
            .unchecked_transaction()
            .map_err(sqlite_error("begin filtered revision snapshot"))?;
        let revisions = {
            let mut statement = snapshot
                .prepare(&format!(
                    "SELECT id, sequence, transaction_id, committed_at_micros,
                            origin_name, origin_version, origin_uri, message
                     FROM revisions WHERE sequence IN ({branches})
                     ORDER BY sequence LIMIT ?2"
                ))
                .map_err(sqlite_error("prepare filtered revision page query"))?;
            statement
                .query_map(params_from_iter(parameters), stored_revision_row)
                .map_err(sqlite_error("query filtered revision page"))?
                .map(|row| {
                    row.map_err(sqlite_error("read revision row"))
                        .and_then(decode_revision)
                })
                .collect::<Result<Vec<_>>>()?
        };
        let through_sequence = match revisions.last() {
            Some(last) if revisions.len() == limit as usize => last.sequence(),
            _ => {
                let latest: i64 = snapshot
                    .query_row(
                        "SELECT coalesce(max(sequence), 0) FROM revisions",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(sqlite_error("query latest revision sequence"))?;
                u64::try_from(latest)
                    .map_err(|_| {
                        Error::new(ErrorKind::Storage, "stored revision sequence is negative")
                    })?
                    .max(sequence)
            }
        };
        drop(snapshot);
        FilteredRevisionPage::new(revisions, through_sequence)
    }

    /// Loads one revision's semantic events in stable position order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when `revision_id` is absent, or
    /// [`ErrorKind::Storage`] when persisted event data is malformed.
    pub fn events_for_revision(&self, revision_id: RevisionId) -> Result<Vec<RevisionEvent>> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM revisions WHERE id = ?1)",
                [revision_id.as_bytes().as_slice()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error("check revision existence"))?;
        if !exists {
            return Err(Error::new(ErrorKind::NotFound, "revision does not exist"));
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT position, kind, target_kind, primary_id, secondary_id,
                        structural_position, vocabulary, property,
                        identifier_scheme, identifier_value, identifier_qualifier,
                        activity_kind, role, fingerprint_algorithm, fingerprint_version
                 FROM revision_events WHERE revision_id = ?1 ORDER BY position",
            )
            .map_err(sqlite_error("prepare revision event query"))?;
        statement
            .query_map(
                [revision_id.as_bytes().as_slice()],
                stored_revision_event_row,
            )
            .map_err(sqlite_error("query revision events"))?
            .map(|row| {
                row.map_err(sqlite_error("read revision event row"))
                    .and_then(|event| decode_revision_event(revision_id, event))
            })
            .collect()
    }

    /// Queries distinct metadata-capable objects touched after a revision.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor or malformed journal data.
    pub fn objects_changed_since(
        &self,
        sequence: u64,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ObjectRef>> {
        let signature = query_cursor::signature(&[&sequence.to_be_bytes()]);
        let position = query_cursor::position_fields(page, "objects-changed", &signature, 2)?
            .map(|fields| {
                let kind = fields[0]
                    .parse::<i64>()
                    .map_err(|_| invalid_query_cursor())?;
                Ok((kind, parse_metadata_cursor_id(kind, fields[1])?))
            })
            .transpose()?;
        let sequence = i64::try_from(sequence).unwrap_or(i64::MAX);
        let (kind, id) = position.unwrap_or((-1, [0; 16]));
        let mut statement = self
            .connection
            .prepare(
                "WITH changed_events AS (
                     SELECT e.kind, e.target_kind, e.primary_id, e.secondary_id
                     FROM revision_events e
                     JOIN revisions r ON r.id = e.revision_id
                     WHERE r.sequence > ?1
                 ), changed_targets(target_kind, target_id) AS (
                     SELECT 1, primary_id FROM changed_events WHERE kind = 1
                     UNION ALL SELECT 2, primary_id FROM changed_events WHERE kind = 2
                     UNION ALL SELECT 1, secondary_id FROM changed_events WHERE kind = 2
                     UNION ALL SELECT 3, primary_id FROM changed_events WHERE kind = 3
                     UNION ALL SELECT 2, primary_id FROM changed_events WHERE kind = 4
                     UNION ALL SELECT 3, secondary_id FROM changed_events WHERE kind = 4
                     UNION ALL SELECT 3, secondary_id FROM changed_events WHERE kind IN (5, 14)
                     UNION ALL
                         SELECT 0, p.id FROM changed_events CROSS JOIN productions p
                         WHERE changed_events.kind IN (6, 15, 16)
                     UNION ALL
                         SELECT target_kind, primary_id FROM changed_events
                         WHERE kind BETWEEN 7 AND 10
                     UNION ALL SELECT 4, primary_id FROM changed_events WHERE kind = 11
                     UNION ALL SELECT 4, primary_id FROM changed_events WHERE kind IN (12, 13)
                     UNION ALL SELECT 2, secondary_id FROM changed_events WHERE kind IN (12, 13)
                     UNION ALL SELECT 3, primary_id FROM changed_events WHERE kind = 17
                     UNION ALL SELECT 2, primary_id FROM changed_events WHERE kind IN (18, 19)
                     UNION ALL SELECT 5, primary_id FROM changed_events WHERE kind BETWEEN 20 AND 26
                 )
                 SELECT DISTINCT target_kind, target_id FROM changed_targets
                 WHERE target_id IS NOT NULL
                   AND (target_kind > ?2 OR (target_kind = ?2 AND target_id > ?3))
                 ORDER BY target_kind, target_id LIMIT ?4",
            )
            .map_err(sqlite_error("prepare changed-object query"))?;
        let mut rows = statement
            .query_map(
                params![sequence, kind, id.as_slice(), i64::from(page.limit()) + 1],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .map_err(sqlite_error("query changed objects"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_error("read changed-object row"))?;
        let has_more = rows.len() > page.limit() as usize;
        rows.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            rows.last()
                .map(|(kind, id)| {
                    let target = decode_metadata_target(*kind, id.clone())?;
                    query_cursor::cursor(
                        "objects-changed",
                        &signature,
                        &[kind.to_string(), object_ref_id_string(target)],
                    )
                })
                .transpose()?
        } else {
            None
        };
        let targets = rows
            .into_iter()
            .map(|(kind, id)| decode_metadata_target(kind, id))
            .collect::<Result<Vec<_>>>()?;
        Ok(QueryPage::new(targets, next_cursor, false))
    }

    fn related_representations(
        &self,
        representation_id: RepresentationId,
        query: &'static str,
        label: &'static str,
    ) -> Result<Vec<RepresentationId>> {
        self.ensure_representation_exists(representation_id)?;
        let mut statement = self
            .connection
            .prepare(query)
            .map_err(sqlite_error("prepare provenance traversal"))?;
        let rows = statement
            .query_map([representation_id.as_bytes().as_slice()], |row| {
                row.get::<_, Vec<u8>>(0)
            })
            .map_err(sqlite_error("query provenance traversal"))?;
        rows.map(|row| {
            let id = row.map_err(sqlite_error("read provenance traversal row"))?;
            Ok(RepresentationId::from_bytes(id_bytes(id, label)?))
        })
        .collect()
    }

    fn ensure_representation_exists(&self, representation_id: RepresentationId) -> Result<()> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM representations WHERE id = ?1)",
                [representation_id.as_bytes().as_slice()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error("check provenance representation"))?;
        if !exists {
            return Err(Error::new(
                ErrorKind::NotFound,
                "provenance representation does not exist",
            ));
        }
        Ok(())
    }

    fn ensure_asset_exists(&self, asset_id: AssetId) -> Result<()> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1)",
                [asset_id.as_bytes().as_slice()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error("check query asset"))?;
        if exists {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::NotFound, "asset does not exist"))
        }
    }

    fn ensure_resource_exists(&self, resource_id: ResourceId) -> Result<()> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM resources WHERE id = ?1)",
                [resource_id.as_bytes().as_slice()],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error("check query resource"))?;
        if exists {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::NotFound, "resource does not exist"))
        }
    }

    fn ensure_media_root_exists(&self, root_name: &str) -> Result<()> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_roots WHERE name = ?1)",
                [root_name],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error("check query media root"))?;
        if exists {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::NotFound, "media root does not exist"))
        }
    }

    fn query_representation_ids(
        &self,
        sql: &str,
        scope: &[u8; 16],
        position: Option<&[u8; 16]>,
        limit: u32,
        label: &'static str,
    ) -> Result<Vec<[u8; 16]>> {
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(sqlite_error("prepare paginated representation query"))?;
        statement
            .query_map(
                params![
                    scope.as_slice(),
                    position.copied().unwrap_or([0; 16]).as_slice(),
                    i64::from(limit) + 1,
                ],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(sqlite_error("query paginated representations"))?
            .map(|row| {
                row.map_err(sqlite_error("read paginated representation row"))
                    .and_then(|id| id_bytes(id, label))
            })
            .collect()
    }

    fn representation_page_from_ids(
        &self,
        mut ids: Vec<[u8; 16]>,
        page: &QueryPageRequest,
        query: &str,
        signature: &str,
    ) -> Result<QueryPage<Representation>> {
        let has_more = ids.len() > page.limit() as usize;
        ids.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            ids.last()
                .map(|id| {
                    query_cursor::cursor(
                        query,
                        signature,
                        &[RepresentationId::from_bytes(*id).to_string()],
                    )
                })
                .transpose()?
        } else {
            None
        };
        let representations = self.load_representations_by_ids(&ids)?;
        Ok(QueryPage::new(representations, next_cursor, false))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "set-oriented representation decoding keeps its coordinated row maps auditable"
    )]
    fn load_representations_by_ids(&self, ids: &[[u8; 16]]) -> Result<Vec<Representation>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let parameters = || {
            ids.iter()
                .map(|id| Value::Blob(id.to_vec()))
                .collect::<Vec<_>>()
        };

        let mut base_statement = self
            .connection
            .prepare(&format!(
                "SELECT id, asset_id, kind, structure_kind FROM representations
                 WHERE id IN ({placeholders}) ORDER BY id"
            ))
            .map_err(sqlite_error("prepare representation-page rows"))?;
        let base_rows = base_statement
            .query_map(params_from_iter(parameters()), |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(sqlite_error("query representation-page rows"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sqlite_error("read representation-page row"))?;

        let mut member_statement = self
            .connection
            .prepare(&format!(
                "SELECT representation_id, resource_id, role, required
                 FROM representation_resources
                 WHERE representation_id IN ({placeholders})
                 ORDER BY representation_id, position"
            ))
            .map_err(sqlite_error("prepare representation-page members"))?;
        let mut members =
            BTreeMap::<RepresentationId, Vec<(ResourceId, Option<String>, bool)>>::new();
        for row in member_statement
            .query_map(params_from_iter(parameters()), |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            })
            .map_err(sqlite_error("query representation-page members"))?
        {
            let (representation_id, resource_id, role, required) =
                row.map_err(sqlite_error("read representation-page member"))?;
            members
                .entry(RepresentationId::from_bytes(id_bytes(
                    representation_id,
                    "representation member",
                )?))
                .or_default()
                .push((
                    ResourceId::from_bytes(id_bytes(resource_id, "member resource")?),
                    role,
                    required,
                ));
        }

        let mut sequence_statement = self
            .connection
            .prepare(&format!(
                "SELECT representation_id, resource_id,
                        start_frame, end_frame, frame_step, rate_numerator, rate_denominator
                 FROM image_sequences WHERE representation_id IN ({placeholders})"
            ))
            .map_err(sqlite_error("prepare representation-page sequences"))?;
        let mut sequences = BTreeMap::<RepresentationId, StoredSequence>::new();
        for row in sequence_statement
            .query_map(params_from_iter(parameters()), |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })
            .map_err(sqlite_error("query representation-page sequences"))?
        {
            let row = row.map_err(sqlite_error("read representation-page sequence"))?;
            sequences.insert(
                RepresentationId::from_bytes(id_bytes(row.0, "sequence representation")?),
                (
                    ResourceId::from_bytes(id_bytes(row.1, "sequence resource")?),
                    row.2,
                    row.3,
                    row.4,
                    row.5,
                    row.6,
                ),
            );
        }

        let mut missing_statement = self
            .connection
            .prepare(&format!(
                "SELECT representation_id, frame FROM image_sequence_missing_frames
                 WHERE representation_id IN ({placeholders})
                 ORDER BY representation_id, frame"
            ))
            .map_err(sqlite_error("prepare representation-page missing frames"))?;
        let mut missing = BTreeMap::<RepresentationId, Vec<i64>>::new();
        for row in missing_statement
            .query_map(params_from_iter(parameters()), |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(sqlite_error("query representation-page missing frames"))?
        {
            let (representation_id, frame) =
                row.map_err(sqlite_error("read representation-page missing frame"))?;
            missing
                .entry(RepresentationId::from_bytes(id_bytes(
                    representation_id,
                    "missing-frame representation",
                )?))
                .or_default()
                .push(frame);
        }

        let mut fingerprint_statement = self
            .connection
            .prepare(&format!(
                "SELECT representation_id, algorithm, algorithm_version, value
                 FROM representation_fingerprints
                 WHERE representation_id IN ({placeholders})
                 ORDER BY representation_id, algorithm, algorithm_version"
            ))
            .map_err(sqlite_error("prepare representation-page fingerprints"))?;
        let mut fingerprints = BTreeMap::<RepresentationId, Vec<RepresentationFingerprint>>::new();
        for row in fingerprint_statement
            .query_map(params_from_iter(parameters()), |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u16>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            })
            .map_err(sqlite_error("query representation-page fingerprints"))?
        {
            let (representation_id, algorithm, version, value) =
                row.map_err(sqlite_error("read representation-page fingerprint"))?;
            fingerprints
                .entry(RepresentationId::from_bytes(id_bytes(
                    representation_id,
                    "fingerprint representation",
                )?))
                .or_default()
                .push(
                    RepresentationFingerprint::new(algorithm, version, value)
                        .map_err(stored_domain_error("representation fingerprint"))?,
                );
        }

        let mut decoded = BTreeMap::new();
        for (id, asset_id, kind, structure_kind) in base_rows {
            let id = RepresentationId::from_bytes(id_bytes(id, "representation")?);
            let asset_id = AssetId::from_bytes(id_bytes(asset_id, "asset")?);
            let stored_members = members.remove(&id).unwrap_or_default();
            let structure = match structure_kind {
                0 => match stored_members.as_slice() {
                    [(resource_id, None, true)] => ContentStructure::single_resource(*resource_id),
                    _ => return Err(stored_invariant("invalid single-resource membership")),
                },
                1 => {
                    let resource_id = match stored_members.as_slice() {
                        [(resource_id, None, true)] => *resource_id,
                        _ => return Err(stored_invariant("invalid image-sequence membership")),
                    };
                    let sequence = sequences
                        .remove(&id)
                        .ok_or_else(|| stored_invariant("image-sequence descriptor is absent"))?;
                    if sequence.0 != resource_id {
                        return Err(stored_invariant(
                            "image-sequence resource does not match membership",
                        ));
                    }
                    ContentStructure::image_sequence(
                        ImageSequenceDescriptor::new(
                            resource_id,
                            FrameRange::new(
                                sequence.1,
                                sequence.2,
                                stored_u32(sequence.3, "frame step")?,
                            )
                            .map_err(stored_domain_error("image-sequence frame range"))?,
                            RationalRate::new(
                                stored_u32(sequence.4, "rate numerator")?,
                                stored_u32(sequence.5, "rate denominator")?,
                            )
                            .map_err(stored_domain_error("image-sequence rate"))?,
                            missing.remove(&id).unwrap_or_default(),
                        )
                        .map_err(stored_domain_error("image-sequence descriptor"))?,
                    )
                }
                2 | 3 => {
                    let members = stored_members
                        .into_iter()
                        .map(|(resource_id, role, required)| {
                            let role = role.ok_or_else(|| {
                                stored_invariant("compound resource membership has no role")
                            })?;
                            Ok(ResourceMember::new(
                                resource_id,
                                ResourceRole::new(role)
                                    .map_err(stored_domain_error("resource role"))?,
                                required,
                            ))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    if structure_kind == 2 {
                        ContentStructure::ordered_parts(members)
                    } else {
                        ContentStructure::package(members)
                    }
                    .map_err(stored_domain_error("content structure"))?
                }
                _ => return Err(stored_invariant("invalid content-structure kind")),
            };
            decoded.insert(
                id,
                Representation::new(
                    id,
                    asset_id,
                    decode_representation_kind(kind)?,
                    structure,
                    fingerprints.remove(&id).unwrap_or_default(),
                ),
            );
        }
        ids.iter()
            .map(|id| {
                decoded
                    .remove(&RepresentationId::from_bytes(*id))
                    .ok_or_else(|| stored_invariant("representation page row is absent"))
            })
            .collect()
    }

    fn activity_ids_for_relation(
        &self,
        table: &'static str,
        representation_id: RepresentationId,
        position: Option<[u8; 16]>,
        limit: u32,
    ) -> Result<Vec<[u8; 16]>> {
        let sql = format!(
            "SELECT DISTINCT activity_id FROM {table}
             WHERE representation_id = ?1 AND activity_id > ?2
             ORDER BY activity_id LIMIT ?3"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare related-activity query"))?;
        statement
            .query_map(
                params![
                    representation_id.as_bytes().as_slice(),
                    position.unwrap_or([0; 16]).as_slice(),
                    i64::from(limit) + 1,
                ],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .map_err(sqlite_error("query related activities"))?
            .map(|row| {
                row.map_err(sqlite_error("read related-activity row"))
                    .and_then(|id| id_bytes(id, "activity"))
            })
            .collect()
    }

    fn activities_for_relation_page(
        &self,
        table: &'static str,
        query: &'static str,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Activity>> {
        self.ensure_representation_exists(representation_id)?;
        let signature = query_cursor::signature(&[representation_id.as_bytes()]);
        let position = query_cursor::id_position::<ActivityId>(page, query, &signature)?;
        let mut ids =
            self.activity_ids_for_relation(table, representation_id, position, page.limit())?;
        let has_more = ids.len() > page.limit() as usize;
        ids.truncate(page.limit() as usize);
        let next_cursor = if has_more {
            ids.last()
                .map(|id| {
                    query_cursor::cursor(
                        query,
                        &signature,
                        &[ActivityId::from_bytes(*id).to_string()],
                    )
                })
                .transpose()?
        } else {
            None
        };
        let activities = ids
            .into_iter()
            .map(|id| self.load_activity_by_id(ActivityId::from_bytes(id)))
            .collect::<Result<Vec<_>>>()?;
        Ok(QueryPage::new(activities, next_cursor, false))
    }

    fn load_activity_by_id(&self, activity_id: ActivityId) -> Result<Activity> {
        let stored = self
            .connection
            .query_row(
                "SELECT id, kind, started_at_micros, finished_at_micros,
                        tool_name, tool_version, tool_uri, agent_name,
                        agent_identifier_scheme, agent_identifier_value,
                        agent_identifier_qualifier
                 FROM activities WHERE id = ?1",
                [activity_id.as_bytes().as_slice()],
                |row| {
                    Ok(StoredActivity {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        started_at: row.get(2)?,
                        finished_at: row.get(3)?,
                        tool_name: row.get(4)?,
                        tool_version: row.get(5)?,
                        tool_uri: row.get(6)?,
                        agent_name: row.get(7)?,
                        agent_scheme: row.get(8)?,
                        agent_value: row.get(9)?,
                        agent_qualifier: row.get(10)?,
                    })
                },
            )
            .optional()
            .map_err(sqlite_error("load query activity"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "activity does not exist"))?;
        decode_activity(
            activity_id,
            stored,
            self.load_activity_inputs_for(activity_id)?,
            self.load_activity_outputs_for(activity_id)?,
        )
    }

    fn load_activity_inputs_for(&self, activity_id: ActivityId) -> Result<Vec<ActivityInput>> {
        self.load_stored_activity_edges_for("activity_inputs", activity_id)?
            .into_iter()
            .map(|edge| {
                let mut input = ActivityInput::new(edge.representation_id, edge.role);
                if let Some(sequence) = edge.snapshot_revision_sequence {
                    input = input.with_snapshot(ActivityEdgeSnapshot::new(
                        stored_u64(sequence, "activity input snapshot revision")?,
                        self.load_edge_fingerprint_snapshots(
                            "activity_input_fingerprint_snapshots",
                            "activity_input_id",
                            edge.id,
                        )?,
                    )?);
                }
                Ok(input)
            })
            .collect()
    }

    fn load_activity_outputs_for(&self, activity_id: ActivityId) -> Result<Vec<ActivityOutput>> {
        self.load_stored_activity_edges_for("activity_outputs", activity_id)?
            .into_iter()
            .map(|edge| {
                let mut output = ActivityOutput::new(edge.representation_id, edge.role);
                if let Some(sequence) = edge.snapshot_revision_sequence {
                    output = output.with_snapshot(ActivityEdgeSnapshot::new(
                        stored_u64(sequence, "activity output snapshot revision")?,
                        self.load_edge_fingerprint_snapshots(
                            "activity_output_fingerprint_snapshots",
                            "activity_output_id",
                            edge.id,
                        )?,
                    )?);
                }
                Ok(output)
            })
            .collect()
    }

    fn load_stored_activity_edges_for(
        &self,
        table: &'static str,
        activity_id: ActivityId,
    ) -> Result<Vec<StoredActivityEdge>> {
        let sql = format!(
            "SELECT id, representation_id, role, snapshot_revision_sequence
             FROM {table} WHERE activity_id = ?1
             ORDER BY representation_id, role, id"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare activity edge page"))?;
        statement
            .query_map([activity_id.as_bytes().as_slice()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(sqlite_error("query activity edge page"))?
            .map(|row| {
                let (id, representation_id, role, snapshot_revision_sequence) =
                    row.map_err(sqlite_error("read activity edge page row"))?;
                Ok(StoredActivityEdge {
                    id,
                    representation_id: RepresentationId::from_bytes(id_bytes(
                        representation_id,
                        "activity representation",
                    )?),
                    role: role
                        .map(ActivityRole::new)
                        .transpose()
                        .map_err(stored_domain_error("activity role"))?,
                    snapshot_revision_sequence,
                })
            })
            .collect()
    }

    fn load_edge_fingerprint_snapshots(
        &self,
        table: &'static str,
        edge_column: &'static str,
        edge_id: i64,
    ) -> Result<Vec<FingerprintSnapshot>> {
        let sql = format!(
            "SELECT algorithm, algorithm_version, value, observed_revision_sequence
             FROM {table} WHERE {edge_column} = ?1
             ORDER BY algorithm, algorithm_version"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare activity snapshot page"))?;
        statement
            .query_map([edge_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(sqlite_error("query activity snapshot page"))?
            .map(|row| {
                let (algorithm, version, value, observed) =
                    row.map_err(sqlite_error("read activity snapshot page row"))?;
                FingerprintSnapshot::new(
                    algorithm,
                    u16::try_from(version).map_err(|_| {
                        stored_invariant("activity snapshot version is outside u16")
                    })?,
                    value,
                    observed
                        .map(|sequence| stored_u64(sequence, "fingerprint observation revision"))
                        .transpose()?,
                )
                .map_err(stored_domain_error("activity fingerprint snapshot"))
            })
            .collect()
    }

    fn load_activity_inputs(&self) -> Result<ActivityEdgesById<ActivityInput>> {
        let snapshots = load_activity_edge_snapshots(
            &self.connection,
            "SELECT activity_input_id, algorithm, algorithm_version, value,
                    observed_revision_sequence
             FROM activity_input_fingerprint_snapshots
             ORDER BY activity_input_id, algorithm, algorithm_version",
        )?;
        load_activity_edges(
            &self.connection,
            "SELECT id, activity_id, representation_id, role, snapshot_revision_sequence
             FROM activity_inputs
             ORDER BY activity_id, representation_id, role, id",
        )?
        .into_iter()
        .try_fold(
            ActivityEdgesById::<ActivityInput>::new(),
            |mut result, (activity_id, edge)| {
                let mut input = ActivityInput::new(edge.representation_id, edge.role);
                if let Some(sequence) = edge.snapshot_revision_sequence {
                    input = input.with_snapshot(ActivityEdgeSnapshot::new(
                        stored_u64(sequence, "activity input snapshot revision")?,
                        snapshots.get(&edge.id).cloned().unwrap_or_default(),
                    )?);
                }
                result.entry(activity_id).or_default().push(input);
                Ok(result)
            },
        )
    }

    fn load_activity_outputs(&self) -> Result<ActivityEdgesById<ActivityOutput>> {
        let snapshots = load_activity_edge_snapshots(
            &self.connection,
            "SELECT activity_output_id, algorithm, algorithm_version, value,
                    observed_revision_sequence
             FROM activity_output_fingerprint_snapshots
             ORDER BY activity_output_id, algorithm, algorithm_version",
        )?;
        load_activity_edges(
            &self.connection,
            "SELECT id, activity_id, representation_id, role, snapshot_revision_sequence
             FROM activity_outputs
             ORDER BY activity_id, representation_id, role, id",
        )?
        .into_iter()
        .try_fold(
            ActivityEdgesById::<ActivityOutput>::new(),
            |mut result, (activity_id, edge)| {
                let mut output = ActivityOutput::new(edge.representation_id, edge.role);
                if let Some(sequence) = edge.snapshot_revision_sequence {
                    output = output.with_snapshot(ActivityEdgeSnapshot::new(
                        stored_u64(sequence, "activity output snapshot revision")?,
                        snapshots.get(&edge.id).cloned().unwrap_or_default(),
                    )?);
                }
                result.entry(activity_id).or_default().push(output);
                Ok(result)
            },
        )
    }

    /// Reports whether SQLite foreign-key enforcement is active on this connection.
    ///
    /// This is primarily useful for diagnostics and integration tests.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Storage`] if SQLite cannot query the setting.
    pub fn foreign_keys_enabled(&self) -> Result<bool> {
        self.connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, bool>(0))
            .map_err(sqlite_error("query foreign-key enforcement"))
    }
}

impl ProductionRead for SqliteProduction {
    fn production(&self) -> &Production {
        SqliteProduction::production(self)
    }

    fn assets(&self) -> Result<Vec<Asset>> {
        SqliteProduction::assets(self)
    }

    fn assets_page(&self, page: &QueryPageRequest) -> Result<QueryPage<Asset>> {
        SqliteProduction::assets_page(self, page)
    }

    fn asset(&self, asset_id: AssetId) -> Result<Asset> {
        SqliteProduction::asset(self, asset_id)
    }

    fn representation(&self, representation_id: RepresentationId) -> Result<Representation> {
        SqliteProduction::representation(self, representation_id)
    }

    fn representations(&self, asset_id: AssetId) -> Result<Vec<Representation>> {
        SqliteProduction::representations(self, asset_id)
    }

    fn representations_page(
        &self,
        asset_id: AssetId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>> {
        SqliteProduction::representations_page(self, asset_id, page)
    }

    fn resources(&self, representation_id: RepresentationId) -> Result<Vec<Resource>> {
        SqliteProduction::resources(self, representation_id)
    }

    fn resources_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Resource>> {
        SqliteProduction::resources_page(self, representation_id, page)
    }

    fn locators(&self, resource_id: ResourceId) -> Result<Vec<Locator>> {
        SqliteProduction::locators(self, resource_id)
    }

    fn locators_page(
        &self,
        resource_id: ResourceId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Locator>> {
        SqliteProduction::locators_page(self, resource_id, page)
    }

    fn find_known_media_by_locator(
        &self,
        locator: &LocatorIdentity,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<KnownMediaMatch>> {
        SqliteProduction::find_known_media_by_locator(self, locator, page)
    }

    fn find_known_media_by_fingerprint(
        &self,
        fingerprint: &ResourceFingerprint,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<KnownMediaMatch>> {
        SqliteProduction::find_known_media_by_fingerprint(self, fingerprint, page)
    }

    fn representations_under_media_root(
        &self,
        root_name: &str,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>> {
        SqliteProduction::representations_under_media_root(self, root_name, page)
    }

    fn representations_using_resource(
        &self,
        resource_id: ResourceId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Representation>> {
        SqliteProduction::representations_using_resource(self, resource_id, page)
    }

    fn unresolved_media(&self, page: &QueryPageRequest) -> Result<QueryPage<RepresentationId>> {
        SqliteProduction::unresolved_media(self, page)
    }

    fn external_identifiers(&self, target: ObjectRef) -> Result<Vec<ExternalIdentifier>> {
        SqliteProduction::external_identifiers(self, target)
    }

    fn find_by_external_identifier(
        &self,
        scheme: &IdentifierScheme,
        value: &str,
        qualifier: Option<&str>,
    ) -> Result<Vec<ObjectRef>> {
        SqliteProduction::find_by_external_identifier(self, scheme, value, qualifier)
    }

    fn metadata(&self, target: ObjectRef) -> Result<Vec<MetadataAssertion>> {
        SqliteProduction::metadata(self, target)
    }

    fn metadata_values(
        &self,
        target: ObjectRef,
        property: &MetadataProperty,
    ) -> Result<Vec<MetadataValue>> {
        SqliteProduction::metadata_values(self, target, property)
    }

    fn query_by_metadata_property(
        &self,
        property: &MetadataProperty,
    ) -> Result<Vec<MetadataMatch>> {
        SqliteProduction::query_by_metadata_property(self, property)
    }

    fn metadata_query(
        &self,
        query: &MetadataQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<MetadataMatch>> {
        SqliteProduction::metadata_query(self, query, page)
    }

    fn activities(&self) -> Result<Vec<Activity>> {
        SqliteProduction::activities(self)
    }

    fn activities_producing(&self, representation_id: RepresentationId) -> Result<Vec<Activity>> {
        SqliteProduction::activities_producing(self, representation_id)
    }

    fn activities_consuming(&self, representation_id: RepresentationId) -> Result<Vec<Activity>> {
        SqliteProduction::activities_consuming(self, representation_id)
    }

    fn activity_outputs(
        &self,
        query: &ActivityOutputQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<RepresentationId>> {
        SqliteProduction::activity_outputs(self, query, page)
    }

    fn activities_producing_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Activity>> {
        SqliteProduction::activities_producing_page(self, representation_id, page)
    }

    fn activities_consuming_page(
        &self,
        representation_id: RepresentationId,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<Activity>> {
        SqliteProduction::activities_consuming_page(self, representation_id, page)
    }

    fn ancestors(&self, representation_id: RepresentationId) -> Result<Vec<RepresentationId>> {
        SqliteProduction::ancestors(self, representation_id)
    }

    fn descendants(&self, representation_id: RepresentationId) -> Result<Vec<RepresentationId>> {
        SqliteProduction::descendants(self, representation_id)
    }

    fn ancestors_page(
        &self,
        representation_id: RepresentationId,
        limits: ProvenanceQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ProvenanceQueryMatch>> {
        SqliteProduction::ancestors_page(self, representation_id, limits, page)
    }

    fn descendants_page(
        &self,
        representation_id: RepresentationId,
        limits: ProvenanceQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ProvenanceQueryMatch>> {
        SqliteProduction::descendants_page(self, representation_id, limits, page)
    }

    fn dependency_set(&self, representation_id: RepresentationId) -> Result<Option<DependencySet>> {
        SqliteProduction::dependency_set(self, representation_id)
    }

    fn dependencies(
        &self,
        source: RepresentationId,
        limits: DependencyQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<DependencyQueryMatch>> {
        SqliteProduction::dependencies(self, source, limits, page)
    }

    fn dependents(
        &self,
        target: DependencyTarget,
        limits: DependencyQueryLimits,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<DependencyQueryMatch>> {
        SqliteProduction::dependents(self, target, limits, page)
    }

    fn evaluate_artifact(
        &self,
        representation_id: RepresentationId,
        limits: ArtifactEvaluationLimits,
    ) -> Result<ArtifactEvaluation> {
        SqliteProduction::evaluate_artifact(self, representation_id, limits)
    }

    fn artifact_reproducibility(
        &self,
        representation_id: RepresentationId,
    ) -> Result<ArtifactReproducibilityReport> {
        SqliteProduction::artifact_reproducibility(self, representation_id)
    }

    fn stale_artifacts(
        &self,
        query: StaleArtifactQuery,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<RepresentationId>> {
        SqliteProduction::stale_artifacts(self, query, page)
    }

    fn jobs(&self, query: &JobQuery, page: &QueryPageRequest) -> Result<QueryPage<Job>> {
        SqliteProduction::jobs(self, query, page)
    }

    fn job(&self, job_id: JobId) -> Result<Job> {
        SqliteProduction::job(self, job_id)
    }

    fn plan_regeneration(
        &self,
        representation_ids: &[RepresentationId],
    ) -> Result<Vec<RegenerationJobPlan>> {
        SqliteProduction::plan_regeneration(self, representation_ids)
    }

    fn latest_revision(&self) -> Result<Option<Revision>> {
        SqliteProduction::latest_revision(self)
    }

    fn changes_since(&self, sequence: u64, limit: u32) -> Result<Vec<Revision>> {
        SqliteProduction::changes_since(self, sequence, limit)
    }

    fn changes_since_filtered(
        &self,
        sequence: u64,
        filter: &RevisionEventFilter,
        limit: u32,
    ) -> Result<FilteredRevisionPage> {
        SqliteProduction::changes_since_filtered(self, sequence, filter, limit)
    }

    fn events_for_revision(&self, revision_id: RevisionId) -> Result<Vec<RevisionEvent>> {
        SqliteProduction::events_for_revision(self, revision_id)
    }

    fn objects_changed_since(
        &self,
        sequence: u64,
        page: &QueryPageRequest,
    ) -> Result<QueryPage<ObjectRef>> {
        SqliteProduction::objects_changed_since(self, sequence, page)
    }
}

impl Drop for SqliteProduction {
    fn drop(&mut self) {
        self.revision_signal.close();
    }
}

impl ProductionStore for SqliteProduction {
    type Transaction<'production> = SqliteTransaction<'production>;

    fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>> {
        SqliteProduction::begin_transaction(self)
    }

    fn begin_transaction_at(&mut self, base_revision: RevisionId) -> Result<Self::Transaction<'_>> {
        SqliteProduction::begin_transaction_at(self, base_revision)
    }
}

fn reserve_new_file(path: &Path) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| {
            let kind = if error.kind() == std::io::ErrorKind::AlreadyExists {
                ErrorKind::AlreadyExists
            } else {
                ErrorKind::Io
            };
            Error::new(
                kind,
                format!("cannot create production file {}: {error}", path.display()),
            )
        })
}

/// Longest a production connection blocks on another writer before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

fn open_connection(path: &Path) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(sqlite_error("open production database"))?;
    configure_length_limit(&connection, MAX_SQLITE_VALUE_BYTES)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(sqlite_error("configure SQLite busy timeout"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA trusted_schema = OFF;")
        .map_err(sqlite_error("configure SQLite connection"))?;
    Ok(connection)
}

fn configure_length_limit(connection: &Connection, maximum: i32) -> Result<()> {
    connection
        .set_limit(Limit::SQLITE_LIMIT_LENGTH, maximum)
        .map(|_| ())
        .map_err(sqlite_error("configure SQLite value length limit"))
}

fn persist_new_production(connection: &mut Connection, production: &Production) -> Result<()> {
    let transaction = connection
        .transaction()
        .map_err(sqlite_error("begin production creation"))?;
    transaction
        .execute(
            "INSERT INTO productions (
                singleton, id, schema_version, created_at_micros, display_name
             ) VALUES (1, ?1, ?2, ?3, ?4)",
            params![
                production.id().as_bytes().as_slice(),
                production.schema_version(),
                production.created_at().as_unix_micros(),
                production.display_name(),
            ],
        )
        .map_err(sqlite_error("persist new production"))?;
    transaction
        .commit()
        .map_err(sqlite_error("commit production creation"))
}

fn load_production(connection: &Connection) -> Result<Production> {
    let stored = connection
        .query_row(
            "SELECT id, schema_version, created_at_micros, display_name
             FROM productions WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(sqlite_error("load production record"))?
        .ok_or_else(|| Error::new(ErrorKind::Storage, "database has no production record"))?;

    let id = ProductionId::from_bytes(id_bytes(stored.0, "production")?);
    if stored.1 != CURRENT_SCHEMA_VERSION {
        return Err(Error::new(
            ErrorKind::Migration,
            format!(
                "production record schema version {} does not match database version {}",
                stored.1, CURRENT_SCHEMA_VERSION
            ),
        ));
    }

    let mut production = Production::new(
        id,
        stored.1,
        Timestamp::from_unix_micros(stored.2),
        stored.3,
    );
    production.set_media_roots(load_media_roots(connection)?);
    Ok(production)
}

fn load_media_roots(connection: &Connection) -> Result<Vec<MediaRoot>> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, label, legacy_uri, priority, enabled
             FROM media_roots ORDER BY priority, id",
        )
        .map_err(sqlite_error("prepare media-root query"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, bool>(5)?,
            ))
        })
        .map_err(sqlite_error("query media roots"))?;
    rows.map(|row| {
        let (id, name, label, legacy_uri, priority, enabled) =
            row.map_err(sqlite_error("read media-root row"))?;
        MediaRoot::new(
            MediaRootId::from_bytes(id_bytes(id, "media root")?),
            name,
            label,
            legacy_uri,
            priority,
            enabled,
        )
        .map_err(stored_domain_error("media root"))
    })
    .collect()
}

fn decode_representation_kind(value: i64) -> Result<RepresentationKind> {
    match value {
        0 => Ok(RepresentationKind::Original),
        1 => Ok(RepresentationKind::Proxy),
        2 => Ok(RepresentationKind::Optimized),
        3 => Ok(RepresentationKind::Derived),
        _ => Err(Error::new(
            ErrorKind::Storage,
            format!("stored representation kind {value} is invalid"),
        )),
    }
}

/// Selected locator columns, with the sequence naming when there is one.
const LOCATOR_COLUMNS: &str = "l.id, l.uri, l.last_seen_micros, l.availability, l.media_root_name,
     n.prefix, n.suffix, n.padding
     FROM locators l LEFT JOIN locator_sequence_namings n ON n.locator_id = l.id";

/// One stored locator row as selected by [`LOCATOR_COLUMNS`].
struct StoredLocator {
    id: Vec<u8>,
    uri: String,
    last_seen: Option<i64>,
    availability: i64,
    media_root: Option<String>,
    naming: Option<(String, String, i64)>,
}

impl StoredLocator {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        let prefix = row.get::<_, Option<String>>(5)?;
        let suffix = row.get::<_, Option<String>>(6)?;
        let padding = row.get::<_, Option<i64>>(7)?;
        Ok(Self {
            id: row.get(0)?,
            uri: row.get(1)?,
            last_seen: row.get(2)?,
            availability: row.get(3)?,
            media_root: row.get(4)?,
            naming: prefix
                .zip(suffix)
                .zip(padding)
                .map(|((prefix, suffix), padding)| (prefix, suffix, padding)),
        })
    }

    fn into_locator(self, resource_id: ResourceId) -> Result<Locator> {
        let mut locator = Locator::new(
            LocatorId::from_bytes(id_bytes(self.id, "locator")?),
            resource_id,
            self.uri,
            self.last_seen.map(Timestamp::from_unix_micros),
            decode_availability(self.availability)?,
        )
        .map_err(stored_domain_error("locator"))?;
        if let Some(name) = self.media_root {
            locator = locator
                .with_media_root(name)
                .map_err(stored_domain_error("locator media root"))?;
        }
        if let Some((prefix, suffix, padding)) = self.naming {
            locator = locator.with_sequence_naming(
                SequenceNaming::new(prefix, suffix, stored_u8(padding, "padding")?)
                    .map_err(stored_domain_error("locator sequence naming"))?,
            );
        }
        Ok(locator)
    }
}

fn decode_availability(value: i64) -> Result<LocatorAvailability> {
    match value {
        0 => Ok(LocatorAvailability::Unknown),
        1 => Ok(LocatorAvailability::Online),
        2 => Ok(LocatorAvailability::Offline),
        _ => Err(Error::new(
            ErrorKind::Storage,
            format!("stored locator availability {value} is invalid"),
        )),
    }
}

fn decode_file_facts(size: Option<i64>, modified_at: Option<i64>) -> Result<Option<FileFacts>> {
    match (size, modified_at) {
        (None, None) => Ok(None),
        (Some(size), modified_at) => {
            let size = u64::try_from(size).map_err(|error| {
                Error::new(
                    ErrorKind::Storage,
                    format!("stored file size is invalid: {error}"),
                )
            })?;
            Ok(Some(FileFacts::new(
                size,
                modified_at.map(Timestamp::from_unix_micros),
            )))
        }
        (None, Some(_)) => Err(Error::new(
            ErrorKind::Storage,
            "stored modification time has no corresponding file size",
        )),
    }
}

fn decode_external_identifier(
    scheme: String,
    value: String,
    qualifier: Option<String>,
) -> Result<ExternalIdentifier> {
    let scheme = IdentifierScheme::new(scheme).map_err(stored_domain_error("identifier scheme"))?;
    ExternalIdentifier::new(scheme, value, qualifier)
        .map_err(stored_domain_error("external identifier"))
}

fn decode_metadata_assertion(
    vocabulary: String,
    property: String,
    encoded: &[u8],
) -> Result<MetadataAssertion> {
    let vocabulary =
        VocabularyId::new(vocabulary).map_err(stored_domain_error("metadata vocabulary"))?;
    let property = PropertyId::new(property).map_err(stored_domain_error("metadata property"))?;
    let value = metadata_codec::decode(encoded)?;
    Ok(MetadataAssertion::new(
        MetadataProperty::new(vocabulary, property),
        value,
    ))
}

fn load_activity_edges(
    connection: &Connection,
    query: &'static str,
) -> Result<Vec<(ActivityId, StoredActivityEdge)>> {
    let mut statement = connection
        .prepare(query)
        .map_err(sqlite_error("prepare activity-edge query"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .map_err(sqlite_error("query activity edges"))?;
    rows.map(|row| {
        let (edge_id, activity_id, representation_id, role, snapshot_revision_sequence) =
            row.map_err(sqlite_error("read activity-edge row"))?;
        let activity_id = ActivityId::from_bytes(id_bytes(activity_id, "activity edge")?);
        let representation_id =
            RepresentationId::from_bytes(id_bytes(representation_id, "activity representation")?);
        let role = role
            .map(ActivityRole::new)
            .transpose()
            .map_err(stored_domain_error("activity role"))?;
        Ok((
            activity_id,
            StoredActivityEdge {
                id: edge_id,
                representation_id,
                role,
                snapshot_revision_sequence,
            },
        ))
    })
    .collect()
}

fn load_activity_edge_snapshots(
    connection: &Connection,
    query: &'static str,
) -> Result<BTreeMap<i64, Vec<FingerprintSnapshot>>> {
    let mut statement = connection
        .prepare(query)
        .map_err(sqlite_error("prepare activity snapshot query"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Option<i64>>(4)?,
            ))
        })
        .map_err(sqlite_error("query activity snapshots"))?;
    rows.map(|row| {
        let (edge_id, algorithm, version, value, observed_sequence) =
            row.map_err(sqlite_error("read activity snapshot row"))?;
        let version = u16::try_from(version)
            .map_err(|_| stored_invariant("activity snapshot version is outside u16"))?;
        let observed_sequence = observed_sequence
            .map(|sequence| stored_u64(sequence, "fingerprint observation revision"))
            .transpose()?;
        let fingerprint = FingerprintSnapshot::new(algorithm, version, value, observed_sequence)
            .map_err(stored_domain_error("activity fingerprint snapshot"))?;
        Ok((edge_id, fingerprint))
    })
    .try_fold(
        BTreeMap::<i64, Vec<FingerprintSnapshot>>::new(),
        |mut result, row| {
            let (edge_id, fingerprint) = row?;
            result.entry(edge_id).or_default().push(fingerprint);
            Ok(result)
        },
    )
}

fn decode_activity(
    id: ActivityId,
    stored: StoredActivity,
    inputs: Vec<ActivityInput>,
    outputs: Vec<ActivityOutput>,
) -> Result<Activity> {
    let kind = ActivityKind::new(stored.kind).map_err(stored_domain_error("activity kind"))?;
    let mut activity = Activity::new(id, kind, inputs, outputs)
        .map_err(stored_domain_error("activity edges"))?
        .with_timing(
            stored.started_at.map(Timestamp::from_unix_micros),
            stored.finished_at.map(Timestamp::from_unix_micros),
        )
        .map_err(stored_domain_error("activity timing"))?;
    match (stored.tool_name, stored.tool_version, stored.tool_uri) {
        (Some(name), version, uri) => {
            activity = activity.with_tool(
                ToolIdentity::new(name, version, uri)
                    .map_err(stored_domain_error("activity tool"))?,
            );
        }
        (None, None, None) => {}
        (None, _, _) => return Err(stored_invariant("activity tool detail has no name")),
    }
    let identifier = match (
        stored.agent_scheme,
        stored.agent_value,
        stored.agent_qualifier,
    ) {
        (Some(scheme), Some(value), qualifier) => {
            Some(decode_external_identifier(scheme, value, qualifier)?)
        }
        (None, None, None) => None,
        _ => return Err(stored_invariant("activity agent identifier is incomplete")),
    };
    if stored.agent_name.is_some() || identifier.is_some() {
        activity = activity.with_agent(
            AgentIdentity::new(stored.agent_name, identifier)
                .map_err(stored_domain_error("activity agent"))?,
        );
    }
    Ok(activity)
}

fn stored_revision_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRevision> {
    Ok(StoredRevision {
        id: row.get(0)?,
        sequence: row.get(1)?,
        transaction_id: row.get(2)?,
        committed_at: row.get(3)?,
        origin_name: row.get(4)?,
        origin_version: row.get(5)?,
        origin_uri: row.get(6)?,
        message: row.get(7)?,
    })
}

fn stored_job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredJob> {
    Ok(StoredJob {
        id: row.get(0)?,
        kind: row.get(1)?,
        output_asset_id: row.get(2)?,
        output_representation_kind: row.get(3)?,
        target_root: row.get(4)?,
        state: row.get(5)?,
        claim_id: row.get(6)?,
        claim_tool_name: row.get(7)?,
        claim_tool_version: row.get(8)?,
        claim_tool_uri: row.get(9)?,
        claim_agent_name: row.get(10)?,
        claim_agent_scheme: row.get(11)?,
        claim_agent_value: row.get(12)?,
        claim_agent_qualifier: row.get(13)?,
        claim_expires_at: row.get(14)?,
        completion_activity_id: row.get(15)?,
        completion_representation_id: row.get(16)?,
        failure_diagnostic: row.get(17)?,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "keeping persisted job-state validation together makes corruption handling auditable"
)]
fn decode_job(connection: &Connection, stored: StoredJob) -> Result<Job> {
    let id = JobId::from_bytes(id_bytes(stored.id.clone(), "job")?);
    let mut statement = connection
        .prepare(
            "SELECT position, representation_id
             FROM job_inputs WHERE job_id = ?1 ORDER BY position",
        )
        .map_err(sqlite_error("prepare job-input query"))?;
    let inputs = statement
        .query_map([id.as_bytes().as_slice()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(sqlite_error("query job inputs"))?
        .enumerate()
        .map(|(expected, row)| {
            let (position, representation_id) = row.map_err(sqlite_error("read job-input row"))?;
            let expected = i64::try_from(expected)
                .map_err(|_| stored_invariant("job input position is too large"))?;
            if position != expected {
                return Err(stored_invariant("job input positions are not contiguous"));
            }
            id_bytes(representation_id, "job input").map(RepresentationId::from_bytes)
        })
        .collect::<Result<Vec<_>>>()?;

    let state = match stored.state {
        1 | 5 => {
            ensure_job_detail_empty(&stored)?;
            if stored.state == 1 {
                JobState::Requested
            } else {
                JobState::Cancelled
            }
        }
        2 => {
            if stored.completion_activity_id.is_some()
                || stored.completion_representation_id.is_some()
                || stored.failure_diagnostic.is_some()
            {
                return Err(stored_invariant("claimed job has terminal detail"));
            }
            let claim_id = JobClaimId::from_bytes(id_bytes(
                required_stored(stored.claim_id, "job claim ID")?,
                "job claim",
            )?);
            let tool = ToolIdentity::new(
                required_stored(stored.claim_tool_name, "job claim tool name")?,
                stored.claim_tool_version,
                stored.claim_tool_uri,
            )
            .map_err(stored_domain_error("job claim tool"))?;
            let identifier = match (stored.claim_agent_scheme, stored.claim_agent_value) {
                (Some(scheme), Some(value)) => Some(decode_external_identifier(
                    scheme,
                    value,
                    stored.claim_agent_qualifier,
                )?),
                (None, None) if stored.claim_agent_qualifier.is_none() => None,
                _ => return Err(stored_invariant("job claim agent identifier is incomplete")),
            };
            let agent = match (stored.claim_agent_name, identifier) {
                (None, None) => None,
                (name, identifier) => Some(
                    AgentIdentity::new(name, identifier)
                        .map_err(stored_domain_error("job claim agent"))?,
                ),
            };
            JobState::Claimed(JobClaim::new(
                claim_id,
                tool,
                agent,
                Timestamp::from_unix_micros(required_stored(
                    stored.claim_expires_at,
                    "job claim expiry",
                )?),
            ))
        }
        3 => {
            ensure_job_claim_detail_empty(&stored)?;
            if stored.failure_diagnostic.is_some() {
                return Err(stored_invariant("succeeded job has failure detail"));
            }
            JobState::Succeeded(JobCompletion::new(
                ActivityId::from_bytes(id_bytes(
                    required_stored(stored.completion_activity_id, "job completion activity")?,
                    "job completion activity",
                )?),
                RepresentationId::from_bytes(id_bytes(
                    required_stored(
                        stored.completion_representation_id,
                        "job completion representation",
                    )?,
                    "job completion representation",
                )?),
            ))
        }
        4 => {
            ensure_job_claim_detail_empty(&stored)?;
            if stored.completion_activity_id.is_some()
                || stored.completion_representation_id.is_some()
            {
                return Err(stored_invariant("failed job has completion detail"));
            }
            JobState::Failed(
                JobFailure::new(required_stored(
                    stored.failure_diagnostic,
                    "job failure diagnostic",
                )?)
                .map_err(stored_domain_error("job failure"))?,
            )
        }
        value => {
            return Err(Error::new(
                ErrorKind::Storage,
                format!("stored job state {value} is invalid"),
            ));
        }
    };
    let output = RequestedJobOutput::new(
        AssetId::from_bytes(id_bytes(stored.output_asset_id, "job output asset")?),
        decode_representation_kind(stored.output_representation_kind)?,
        stored.target_root,
    )
    .map_err(stored_domain_error("job requested output"))?;
    let job = Job::new(
        id,
        JobKind::new(stored.kind).map_err(stored_domain_error("job kind"))?,
        inputs.clone(),
        output,
    )
    .map_err(stored_domain_error("job"))?;
    if job.inputs() != inputs {
        return Err(stored_invariant("job inputs are not in canonical order"));
    }
    Ok(job.with_state(state))
}

fn ensure_job_claim_detail_empty(stored: &StoredJob) -> Result<()> {
    if stored.claim_id.is_some()
        || stored.claim_tool_name.is_some()
        || stored.claim_tool_version.is_some()
        || stored.claim_tool_uri.is_some()
        || stored.claim_agent_name.is_some()
        || stored.claim_agent_scheme.is_some()
        || stored.claim_agent_value.is_some()
        || stored.claim_agent_qualifier.is_some()
        || stored.claim_expires_at.is_some()
    {
        return Err(stored_invariant("unclaimed job has claim detail"));
    }
    Ok(())
}

fn ensure_job_detail_empty(stored: &StoredJob) -> Result<()> {
    ensure_job_claim_detail_empty(stored)?;
    if stored.completion_activity_id.is_some()
        || stored.completion_representation_id.is_some()
        || stored.failure_diagnostic.is_some()
    {
        return Err(stored_invariant("nonterminal job has terminal detail"));
    }
    Ok(())
}

fn decode_revision(stored: StoredRevision) -> Result<Revision> {
    let id = RevisionId::from_bytes(id_bytes(stored.id, "revision")?);
    let sequence = u64::try_from(stored.sequence)
        .map_err(|_| stored_invariant("revision sequence is negative"))?;
    let transaction_id =
        TransactionId::from_bytes(id_bytes(stored.transaction_id, "revision transaction")?);
    let origin = match (stored.origin_name, stored.origin_version, stored.origin_uri) {
        (Some(name), version, uri) => Some(
            OriginIdentity::new(name, version, uri)
                .map_err(stored_domain_error("revision origin"))?,
        ),
        (None, None, None) => None,
        (None, _, _) => return Err(stored_invariant("revision origin detail has no name")),
    };
    Revision::new(
        id,
        sequence,
        transaction_id,
        Timestamp::from_unix_micros(stored.committed_at),
        origin,
        stored.message,
    )
    .map_err(stored_domain_error("revision"))
}

fn stored_revision_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRevisionEvent> {
    Ok(StoredRevisionEvent {
        position: row.get(0)?,
        kind: row.get(1)?,
        target_kind: row.get(2)?,
        primary_id: row.get(3)?,
        secondary_id: row.get(4)?,
        structural_position: row.get(5)?,
        vocabulary: row.get(6)?,
        property: row.get(7)?,
        identifier_scheme: row.get(8)?,
        identifier_value: row.get(9)?,
        identifier_qualifier: row.get(10)?,
        activity_kind: row.get(11)?,
        role: row.get(12)?,
        fingerprint_algorithm: row.get(13)?,
        fingerprint_version: row.get(14)?,
    })
}

/// Maps an event type to its persisted `revision_events.kind` code.
fn stored_revision_event_kind(event_type: RevisionEventType) -> Result<i64> {
    Ok(match event_type {
        RevisionEventType::AssetImported => 1,
        RevisionEventType::RepresentationAdded => 2,
        RevisionEventType::ResourceAdded => 3,
        RevisionEventType::RepresentationResourceAdded => 4,
        RevisionEventType::LocatorAdded => 5,
        RevisionEventType::MediaRootAdded => 6,
        RevisionEventType::ExternalIdentifierAdded => 7,
        RevisionEventType::ExternalIdentifierRemoved => 8,
        RevisionEventType::MetadataAddedOrReplaced => 9,
        RevisionEventType::MetadataRemoved => 10,
        RevisionEventType::ActivityCreated => 11,
        RevisionEventType::ActivityInputAdded => 12,
        RevisionEventType::ActivityOutputAdded => 13,
        RevisionEventType::LocatorRetired => 14,
        RevisionEventType::MediaRootEnabledChanged => 15,
        RevisionEventType::MediaRootRemoved => 16,
        RevisionEventType::ResourceFingerprintObserved => 17,
        RevisionEventType::RepresentationFingerprintObserved => 18,
        RevisionEventType::DependencySetRecorded => 19,
        RevisionEventType::JobRequested => 20,
        RevisionEventType::JobClaimed => 21,
        RevisionEventType::JobClaimRenewed => 22,
        RevisionEventType::JobClaimReleased => 23,
        RevisionEventType::JobSucceeded => 24,
        RevisionEventType::JobFailed => 25,
        RevisionEventType::JobCancelled => 26,
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("revision event type {event_type} is not stored by this build"),
            ));
        }
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "keeping the exhaustive semantic-event schema mapping together is auditable"
)]
fn decode_revision_event(
    revision_id: RevisionId,
    stored: StoredRevisionEvent,
) -> Result<RevisionEvent> {
    let position = stored_u32(stored.position, "revision event position")?;
    let primary_id = |label| required_stored_id(stored.primary_id.clone(), label);
    let secondary_id = |label| required_stored_id(stored.secondary_id.clone(), label);
    let target = || {
        let kind = required_stored(stored.target_kind, "revision event target kind")?;
        decode_metadata_target(
            kind,
            required_stored(stored.primary_id.clone(), "target ID")?,
        )
    };
    let identifier = || {
        decode_external_identifier(
            required_stored(
                stored.identifier_scheme.clone(),
                "revision event identifier scheme",
            )?,
            required_stored(
                stored.identifier_value.clone(),
                "revision event identifier value",
            )?,
            stored.identifier_qualifier.clone(),
        )
    };
    let property = || {
        let vocabulary = VocabularyId::new(required_stored(
            stored.vocabulary.clone(),
            "revision event vocabulary",
        )?)
        .map_err(stored_domain_error("revision event vocabulary"))?;
        let property = PropertyId::new(required_stored(
            stored.property.clone(),
            "revision event property",
        )?)
        .map_err(stored_domain_error("revision event property"))?;
        Ok(MetadataProperty::new(vocabulary, property))
    };

    let kind = match stored.kind {
        1 => RevisionEventKind::AssetImported {
            asset_id: AssetId::from_bytes(primary_id("asset")?),
        },
        2 => RevisionEventKind::RepresentationAdded {
            asset_id: AssetId::from_bytes(secondary_id("asset")?),
            representation_id: RepresentationId::from_bytes(primary_id("representation")?),
        },
        3 => RevisionEventKind::ResourceAdded {
            resource_id: ResourceId::from_bytes(primary_id("resource")?),
        },
        4 => RevisionEventKind::RepresentationResourceAdded {
            representation_id: RepresentationId::from_bytes(primary_id("representation")?),
            resource_id: ResourceId::from_bytes(secondary_id("resource")?),
            position: stored_u32(
                required_stored(
                    stored.structural_position,
                    "revision event structural position",
                )?,
                "revision event structural position",
            )?,
        },
        5 => RevisionEventKind::LocatorAdded {
            resource_id: ResourceId::from_bytes(secondary_id("resource")?),
            locator_id: LocatorId::from_bytes(primary_id("locator")?),
        },
        6 => RevisionEventKind::MediaRootAdded {
            media_root_id: MediaRootId::from_bytes(primary_id("media root")?),
        },
        14 => RevisionEventKind::LocatorRetired {
            resource_id: ResourceId::from_bytes(secondary_id("resource")?),
            locator_id: LocatorId::from_bytes(primary_id("locator")?),
        },
        15 => RevisionEventKind::MediaRootEnabledChanged {
            media_root_id: MediaRootId::from_bytes(primary_id("media root")?),
            enabled: match required_stored(
                stored.structural_position,
                "revision event enabled state",
            )? {
                0 => false,
                1 => true,
                value => {
                    return Err(Error::new(
                        ErrorKind::Storage,
                        format!("stored media-root enabled state {value} is invalid"),
                    ));
                }
            },
        },
        16 => RevisionEventKind::MediaRootRemoved {
            media_root_id: MediaRootId::from_bytes(primary_id("media root")?),
        },
        7 => RevisionEventKind::ExternalIdentifierAdded {
            target: target()?,
            identifier: identifier()?,
        },
        8 => RevisionEventKind::ExternalIdentifierRemoved {
            target: target()?,
            identifier: identifier()?,
        },
        9 => RevisionEventKind::MetadataAddedOrReplaced {
            target: target()?,
            property: property()?,
        },
        10 => RevisionEventKind::MetadataRemoved {
            target: target()?,
            property: property()?,
        },
        11 => RevisionEventKind::ActivityCreated {
            activity_id: ActivityId::from_bytes(primary_id("activity")?),
            kind: ActivityKind::new(required_stored(
                stored.activity_kind,
                "revision event activity kind",
            )?)
            .map_err(stored_domain_error("revision event activity kind"))?,
        },
        12 | 13 => {
            let activity_id = ActivityId::from_bytes(primary_id("activity")?);
            let representation_id = RepresentationId::from_bytes(secondary_id("representation")?);
            let role = stored
                .role
                .map(ActivityRole::new)
                .transpose()
                .map_err(stored_domain_error("revision event role"))?;
            if stored.kind == 12 {
                RevisionEventKind::ActivityInputAdded {
                    activity_id,
                    representation_id,
                    role,
                }
            } else {
                RevisionEventKind::ActivityOutputAdded {
                    activity_id,
                    representation_id,
                    role,
                }
            }
        }
        17 | 18 => {
            let algorithm = required_stored(
                stored.fingerprint_algorithm,
                "revision event fingerprint algorithm",
            )?;
            let version = u16::try_from(required_stored(
                stored.fingerprint_version,
                "revision event fingerprint version",
            )?)
            .map_err(|_| stored_invariant("revision event fingerprint version is invalid"))?;
            ResourceFingerprint::new(algorithm.clone(), version, vec![1])
                .map_err(stored_domain_error("revision event fingerprint domain"))?;
            if stored.kind == 17 {
                RevisionEventKind::ResourceFingerprintObserved {
                    resource_id: ResourceId::from_bytes(primary_id("resource")?),
                    algorithm,
                    version,
                }
            } else {
                RevisionEventKind::RepresentationFingerprintObserved {
                    representation_id: RepresentationId::from_bytes(primary_id("representation")?),
                    algorithm,
                    version,
                }
            }
        }
        19 => RevisionEventKind::DependencySetRecorded {
            representation_id: RepresentationId::from_bytes(primary_id("representation")?),
        },
        20 => RevisionEventKind::JobRequested {
            job_id: JobId::from_bytes(primary_id("job")?),
        },
        21 => RevisionEventKind::JobClaimed {
            job_id: JobId::from_bytes(primary_id("job")?),
        },
        22 => RevisionEventKind::JobClaimRenewed {
            job_id: JobId::from_bytes(primary_id("job")?),
        },
        23 => RevisionEventKind::JobClaimReleased {
            job_id: JobId::from_bytes(primary_id("job")?),
        },
        24 => RevisionEventKind::JobSucceeded {
            job_id: JobId::from_bytes(primary_id("job")?),
        },
        25 => RevisionEventKind::JobFailed {
            job_id: JobId::from_bytes(primary_id("job")?),
        },
        26 => RevisionEventKind::JobCancelled {
            job_id: JobId::from_bytes(primary_id("job")?),
        },
        kind => {
            return Err(Error::new(
                ErrorKind::Storage,
                format!("stored revision event kind {kind} is invalid"),
            ));
        }
    };
    Ok(RevisionEvent::new(revision_id, position, kind))
}

fn required_stored<T>(value: Option<T>, label: &'static str) -> Result<T> {
    value.ok_or_else(|| Error::new(ErrorKind::Storage, format!("stored {label} is missing")))
}

fn required_stored_id(value: Option<Vec<u8>>, label: &'static str) -> Result<[u8; 16]> {
    id_bytes(required_stored(value, label)?, label)
}

pub(crate) fn encode_identifier_target(target: &ObjectRef) -> Result<(i64, &[u8; 16])> {
    match target {
        ObjectRef::Asset(id) => Ok((1, id.as_bytes())),
        ObjectRef::Representation(id) => Ok((2, id.as_bytes())),
        ObjectRef::Resource(id) => Ok((3, id.as_bytes())),
        ObjectRef::Activity(id) => Ok((4, id.as_bytes())),
        ObjectRef::Production(_) => Err(Error::new(
            ErrorKind::Unsupported,
            "external identifiers do not support productions",
        )),
        _ => Err(Error::new(
            ErrorKind::Unsupported,
            "external identifier target kind is not supported by this schema",
        )),
    }
}

fn decode_identifier_target(kind: i64, id: Vec<u8>) -> Result<ObjectRef> {
    let id = id_bytes(id, "external identifier target")?;
    match kind {
        1 => Ok(ObjectRef::Asset(AssetId::from_bytes(id))),
        2 => Ok(ObjectRef::Representation(RepresentationId::from_bytes(id))),
        3 => Ok(ObjectRef::Resource(ResourceId::from_bytes(id))),
        4 => Ok(ObjectRef::Activity(
            postproject_core::ActivityId::from_bytes(id),
        )),
        _ => Err(Error::new(
            ErrorKind::Storage,
            format!("stored external identifier target kind {kind} is invalid"),
        )),
    }
}

pub(crate) fn encode_metadata_target(target: &ObjectRef) -> Result<(i64, &[u8; 16])> {
    match target {
        ObjectRef::Production(id) => Ok((0, id.as_bytes())),
        ObjectRef::Asset(id) => Ok((1, id.as_bytes())),
        ObjectRef::Representation(id) => Ok((2, id.as_bytes())),
        ObjectRef::Resource(id) => Ok((3, id.as_bytes())),
        ObjectRef::Activity(id) => Ok((4, id.as_bytes())),
        ObjectRef::Job(id) => Ok((5, id.as_bytes())),
        _ => Err(Error::new(
            ErrorKind::Unsupported,
            "metadata target kind is not supported by this schema",
        )),
    }
}

fn decode_metadata_target(kind: i64, id: Vec<u8>) -> Result<ObjectRef> {
    let id = id_bytes(id, "metadata target")?;
    match kind {
        0 => Ok(ObjectRef::Production(ProductionId::from_bytes(id))),
        1 => Ok(ObjectRef::Asset(AssetId::from_bytes(id))),
        2 => Ok(ObjectRef::Representation(RepresentationId::from_bytes(id))),
        3 => Ok(ObjectRef::Resource(ResourceId::from_bytes(id))),
        4 => Ok(ObjectRef::Activity(
            postproject_core::ActivityId::from_bytes(id),
        )),
        5 => Ok(ObjectRef::Job(JobId::from_bytes(id))),
        _ => Err(Error::new(
            ErrorKind::Storage,
            format!("stored metadata target kind {kind} is invalid"),
        )),
    }
}

fn decode_dependency(
    source_resource: Option<Vec<u8>>,
    kind: String,
    target_kind: i64,
    target: Vec<u8>,
    resolved: Option<Vec<u8>>,
    required: i64,
    authored_reference: String,
) -> Result<Dependency> {
    let source_resource_id = source_resource
        .map(|value| id_bytes(value, "dependency source resource").map(ResourceId::from_bytes))
        .transpose()?;
    let target = id_bytes(target, "dependency target")?;
    let target = match target_kind {
        1 => DependencyTarget::Asset(AssetId::from_bytes(target)),
        2 => DependencyTarget::Representation(RepresentationId::from_bytes(target)),
        _ => return Err(stored_invariant("dependency target kind is invalid")),
    };
    let resolved_representation_id = resolved
        .map(|value| {
            id_bytes(value, "resolved dependency representation").map(RepresentationId::from_bytes)
        })
        .transpose()?;
    let required = match required {
        0 => false,
        1 => true,
        _ => return Err(stored_invariant("dependency requiredness is invalid")),
    };
    Dependency::new(
        source_resource_id,
        DependencyKind::new(kind).map_err(stored_domain_error("dependency kind"))?,
        target,
        resolved_representation_id,
        required,
        authored_reference,
    )
    .map_err(stored_domain_error("dependency"))
}

pub(crate) fn load_dependency_set(
    connection: &Connection,
    representation_id: RepresentationId,
) -> Result<Option<DependencySet>> {
    let header = connection
        .query_row(
            "SELECT recorded_revision_sequence, needs_extraction
             FROM dependency_sets WHERE source_representation_id = ?1",
            [representation_id.as_bytes().as_slice()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(sqlite_error("query dependency-set observation"))?;
    let Some((revision, needs_extraction)) = header else {
        return Ok(None);
    };
    let mut statement = connection
        .prepare(
            "SELECT source_resource_id, kind, target_kind, target_id,
                    resolved_representation_id, required, authored_reference
             FROM dependencies WHERE source_representation_id = ?1
             ORDER BY position",
        )
        .map_err(sqlite_error("prepare dependency query"))?;
    let dependencies = statement
        .query_map([representation_id.as_bytes().as_slice()], |row| {
            Ok((
                row.get::<_, Option<Vec<u8>>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Option<Vec<u8>>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(sqlite_error("query dependencies"))?
        .map(|row| {
            let (source_resource, kind, target_kind, target, resolved, required, authored) =
                row.map_err(sqlite_error("read dependency row"))?;
            decode_dependency(
                source_resource,
                kind,
                target_kind,
                target,
                resolved,
                required,
                authored,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let status = match needs_extraction {
        0 => DependencySetStatus::Current,
        1 => DependencySetStatus::NeedsExtraction,
        _ => return Err(stored_invariant("dependency-set status is invalid")),
    };
    DependencySet::new(
        representation_id,
        stored_u64(revision, "dependency-set revision")?,
        status,
        dependencies,
    )
    .map(Some)
    .map_err(stored_domain_error("dependency set"))
}

fn dependency_key(target: DependencyTarget) -> (u8, [u8; 16]) {
    match target {
        DependencyTarget::Asset(id) => (1, id.into_bytes()),
        DependencyTarget::Representation(id) => (2, id.into_bytes()),
        _ => (0, [0; 16]),
    }
}

fn dependency_representation(dependency: &Dependency) -> Option<RepresentationId> {
    match dependency.target() {
        DependencyTarget::Asset(_) => dependency.resolved_representation_id(),
        DependencyTarget::Representation(id) => Some(id),
        _ => None,
    }
}

fn dependency_page<F>(
    matches: BTreeMap<(u8, [u8; 16]), (DependencyTarget, u32)>,
    position: Option<(u8, [u8; 16])>,
    page: &QueryPageRequest,
    traversal_truncated: bool,
    cursor: F,
) -> Result<QueryPage<DependencyQueryMatch>>
where
    F: FnOnce((u8, [u8; 16])) -> Result<QueryCursor>,
{
    let mut selected = matches
        .into_iter()
        .filter(|(key, _)| position.is_none_or(|position| *key > position))
        .take(page.limit() as usize + 1)
        .collect::<Vec<_>>();
    let has_more = selected.len() > page.limit() as usize;
    selected.truncate(page.limit() as usize);
    let next_cursor = if has_more {
        selected.last().map(|(key, _)| cursor(*key)).transpose()?
    } else {
        None
    };
    let items = selected
        .into_iter()
        .map(|(_, (target, depth))| DependencyQueryMatch::new(target, depth))
        .collect();
    Ok(QueryPage::new(items, next_cursor, traversal_truncated))
}

fn id_page(
    ids: &mut Vec<RepresentationId>,
    page: &QueryPageRequest,
    query: &str,
    signature: &str,
) -> Result<QueryPage<RepresentationId>> {
    let has_more = ids.len() > page.limit() as usize;
    ids.truncate(page.limit() as usize);
    let next_cursor = if has_more {
        ids.last()
            .map(|id| query_cursor::cursor(query, signature, &[id.to_string()]))
            .transpose()?
    } else {
        None
    };
    Ok(QueryPage::new(std::mem::take(ids), next_cursor, false))
}

pub(crate) fn id_bytes(value: Vec<u8>, label: &str) -> Result<[u8; 16]> {
    value.try_into().map_err(|value: Vec<u8>| {
        Error::new(
            ErrorKind::Storage,
            format!("stored {label} ID has {} bytes; expected 16", value.len()),
        )
    })
}

fn invalid_query_cursor() -> Error {
    Error::new(
        ErrorKind::InvalidArgument,
        "query cursor does not match this query and its parameters",
    )
}

fn object_ref_id_string(target: ObjectRef) -> String {
    match target {
        ObjectRef::Production(id) => id.to_string(),
        ObjectRef::Asset(id) => id.to_string(),
        ObjectRef::Representation(id) => id.to_string(),
        ObjectRef::Resource(id) => id.to_string(),
        ObjectRef::Activity(id) => id.to_string(),
        ObjectRef::Job(id) => id.to_string(),
        _ => String::new(),
    }
}

fn parse_metadata_cursor_id(kind: i64, value: &str) -> Result<[u8; 16]> {
    match kind {
        0 => value.parse::<ProductionId>().map(ProductionId::into_bytes),
        1 => value.parse::<AssetId>().map(AssetId::into_bytes),
        2 => value
            .parse::<RepresentationId>()
            .map(RepresentationId::into_bytes),
        3 => value.parse::<ResourceId>().map(ResourceId::into_bytes),
        4 => value.parse::<ActivityId>().map(ActivityId::into_bytes),
        5 => value.parse::<JobId>().map(JobId::into_bytes),
        _ => Err(invalid_query_cursor()),
    }
    .map_err(|_| invalid_query_cursor())
}

fn stored_u32(value: i64, label: &str) -> Result<u32> {
    u32::try_from(value).map_err(|error| {
        Error::new(
            ErrorKind::Storage,
            format!("stored {label} is invalid: {error}"),
        )
    })
}

fn stored_u64(value: i64, label: &str) -> Result<u64> {
    u64::try_from(value).map_err(|error| {
        Error::new(
            ErrorKind::Storage,
            format!("stored {label} is invalid: {error}"),
        )
    })
}

fn stored_u8(value: i64, label: &str) -> Result<u8> {
    u8::try_from(value).map_err(|error| {
        Error::new(
            ErrorKind::Storage,
            format!("stored {label} is invalid: {error}"),
        )
    })
}

fn stored_invariant(message: &'static str) -> Error {
    Error::new(ErrorKind::Storage, message)
}

pub(crate) fn sqlite_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> Error {
    move |error| Error::new(ErrorKind::Storage, format!("{context}: {error}"))
}

fn stored_domain_error(label: &'static str) -> impl FnOnce(Error) -> Error {
    move |error| {
        Error::new(
            ErrorKind::Storage,
            format!("stored {label} is invalid: {error}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_value_limit_rejects_oversized_results() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        configure_length_limit(&connection, 1_024).expect("configure test limit");

        let result =
            connection.query_row("SELECT zeroblob(1025)", [], |row| row.get::<_, Vec<u8>>(0));

        assert!(result.is_err(), "oversized value unexpectedly loaded");
    }
}
