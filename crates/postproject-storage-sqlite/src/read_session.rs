//! Read transactions on private, independently owned connections.

use postproject_core::{
    DecisionBase, Error, ErrorKind, ProductionRead, ProductionReadSession, Result,
};

use crate::{SqliteProduction, load_production, open_connection, sqlite_error};

/// A pinned read view. Domain results are owned and survive this session.
///
/// Readers do not block writers in WAL mode, but a retained view can delay WAL
/// reclamation. Drop it before long host work after detaching the decision base.
pub struct SqliteReadSession {
    reader: SqliteProduction,
    base: DecisionBase,
}

impl SqliteReadSession {
    pub(crate) fn open(production: &SqliteProduction) -> Result<Self> {
        let mode: String = production
            .connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(sqlite_error("enable coherent-read WAL"))?;
        if mode != "wal" {
            return Err(Error::new(
                ErrorKind::Storage,
                "coherent reads require WAL journaling",
            ));
        }
        let connection = open_connection(production.path())?;
        connection
            .execute_batch("PRAGMA query_only = ON; BEGIN DEFERRED;")
            .map_err(sqlite_error("begin coherent production read"))?;
        // This first read actually establishes the view. Reload roots too:
        // the originating connection's cached production may be older.
        let metadata = load_production(&connection)?;
        if metadata.id() != production.production().id() {
            return Err(Error::new(
                ErrorKind::Conflict,
                "production identity changed",
            ));
        }
        let nonce: Vec<u8> = connection
            .query_row("SELECT randomblob(16)", [], |row| row.get(0))
            .map_err(sqlite_error("create read cursor scope"))?;
        let nonce = crate::id_bytes(nonce, "read cursor scope")?;
        let mut reader =
            SqliteProduction::from_parts(production.path().to_path_buf(), connection, metadata)?;
        reader.read_scope = Some(nonce);
        let revision = reader.latest_revision()?;
        let base = DecisionBase::new(
            reader.production().id(),
            revision.as_ref().map(postproject_core::Revision::id),
            revision
                .as_ref()
                .map_or(0, postproject_core::Revision::sequence),
        )?;
        Ok(Self { reader, base })
    }

    /// Returns domain read operations, all using the same pinned view.
    #[must_use]
    pub fn read(&self) -> &dyn ProductionRead {
        &self.reader
    }

    /// Transfers the pinned connection to a read-only adapter facade.
    ///
    /// Every read retains this view and its cursor scope. Write transactions,
    /// nested sessions and live waiters are rejected. Detach the decision base
    /// first when the adapter also needs to begin edits on the original store.
    #[must_use]
    pub fn into_read_only(self) -> SqliteProduction {
        self.reader
    }

    /// Returns a detached base that remains usable after this session is dropped.
    #[must_use]
    pub const fn decision_base(&self) -> DecisionBase {
        self.base
    }
}

impl ProductionReadSession for SqliteReadSession {
    fn read(&self) -> &dyn ProductionRead {
        self.read()
    }
    fn decision_base(&self) -> DecisionBase {
        self.decision_base()
    }
}
