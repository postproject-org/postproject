//! Bounded waiting for revisions committed to one production file.
//!
//! A waiter owns a dedicated read connection. Commits through the production
//! it was created from wake it immediately; commits from other connections and
//! processes are detected by polling `PRAGMA data_version` with bounded
//! backoff. The data version is authoritative and the in-process signal only
//! shortens the wait (ADR 0027).

use std::{
    path::Path,
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use postproject_core::{
    Error, ErrorKind, ProductionId, Result, Revision, RevisionWaitOutcome, RevisionWaiter,
    validate_revision_wait,
};
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, params};

use crate::{
    BUSY_TIMEOUT, MAX_SQLITE_VALUE_BYTES, configure_length_limit, decode_revision, sqlite_error,
    stored_revision_row,
};

/// First interval between data-version polls after a change or a wait start.
const FIRST_POLL_INTERVAL: Duration = Duration::from_millis(5);
/// Longest interval between data-version polls while nothing changes.
const MAX_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Longest a waiter's read blocks on a writer before retrying at the next poll.
const WAITER_BUSY_TIMEOUT: Duration = Duration::from_millis(20);

/// In-process commit signal shared by a production and its waiters.
#[derive(Debug, Default)]
pub(crate) struct RevisionSignal {
    state: Mutex<SignalState>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct SignalState {
    generation: u64,
    closed: bool,
}

impl RevisionSignal {
    /// Wakes every waiter after a commit that created a revision.
    pub(crate) fn notify_commit(&self) {
        let mut state = self.lock();
        state.generation = state.generation.wrapping_add(1);
        self.changed.notify_all();
    }

    /// Permanently closes every waiter sharing this signal.
    pub(crate) fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, SignalState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Waits for revisions committed to one production file.
///
/// Create it with [`crate::SqliteProduction::revision_waiter`]. The waiter
/// holds its own SQLite connection and never uses the production's connection,
/// so it may block on another thread while the production is used or dropped.
/// Dropping or explicitly closing the production makes every wait return
/// [`RevisionWaitOutcome::Closed`].
#[derive(Debug)]
pub struct SqliteRevisionWaiter {
    connection: Connection,
    signal: Arc<RevisionSignal>,
    cancelled: Arc<AtomicBool>,
    data_version: Option<i64>,
}

/// Thread-safe handle that cancels one [`SqliteRevisionWaiter`].
#[derive(Clone, Debug)]
pub struct RevisionWaitCanceller {
    signal: Arc<RevisionSignal>,
    cancelled: Arc<AtomicBool>,
}

impl RevisionWaitCanceller {
    /// Ends the waiter's current wait and makes every later wait return
    /// [`RevisionWaitOutcome::Cancelled`]. Repeated calls are harmless.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        // Taking the lock orders the flag before a waiter's check-then-wait.
        let _state = self.signal.lock();
        self.signal.changed.notify_all();
    }
}

impl SqliteRevisionWaiter {
    pub(crate) fn open(
        path: &Path,
        production_id: ProductionId,
        signal: Arc<RevisionSignal>,
    ) -> Result<Self> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(sqlite_error("open revision waiter connection"))?;
        configure_length_limit(&connection, MAX_SQLITE_VALUE_BYTES)?;
        // The identity check is not retried, so it waits for a concurrent
        // writer like any other production open; only polls use the short
        // timeout below.
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(sqlite_error("configure revision waiter busy timeout"))?;
        connection
            .execute_batch("PRAGMA trusted_schema = OFF; PRAGMA query_only = ON;")
            .map_err(sqlite_error("configure revision waiter connection"))?;
        let stored_id = connection
            .query_row(
                "SELECT id FROM productions WHERE singleton = 1",
                [],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(sqlite_error("read waiter production identity"))?;
        if stored_id.as_deref() != Some(production_id.as_bytes().as_slice()) {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!(
                    "production file no longer holds production {production_id}: {}",
                    path.display()
                ),
            ));
        }
        connection
            .busy_timeout(WAITER_BUSY_TIMEOUT)
            .map_err(sqlite_error("configure revision waiter busy timeout"))?;
        Ok(Self {
            connection,
            signal,
            cancelled: Arc::new(AtomicBool::new(false)),
            data_version: None,
        })
    }

    /// Returns a handle that can cancel this waiter from any thread.
    #[must_use]
    pub fn canceller(&self) -> RevisionWaitCanceller {
        RevisionWaitCanceller {
            signal: Arc::clone(&self.signal),
            cancelled: Arc::clone(&self.cancelled),
        }
    }

    /// Waits until at least one revision after `after_sequence` exists, then
    /// returns up to `limit` of them in ascending order.
    ///
    /// A zero `timeout` checks once without blocking. Closed and cancelled
    /// outcomes are terminal.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for a zero or excessive `limit`
    /// or a timeout above [`postproject_core::MAX_REVISION_WAIT`], and
    /// [`ErrorKind::Storage`] when the journal cannot be read.
    pub fn wait_for_revisions(
        &mut self,
        after_sequence: u64,
        limit: u32,
        timeout: Duration,
    ) -> Result<RevisionWaitOutcome> {
        validate_revision_wait(limit, timeout)?;
        let started = Instant::now();
        let mut poll_interval = FIRST_POLL_INTERVAL;
        let mut read_journal = true;
        loop {
            let generation = match self.generation() {
                Ok(generation) => generation,
                Err(outcome) => return Ok(outcome),
            };
            if let Some(version) = self.observed_data_version()? {
                if read_journal || self.data_version != Some(version) {
                    if let Some(revisions) = self.revisions_after(after_sequence, limit)? {
                        // Recording the version read before the journal is
                        // conservative: a commit in between is re-read.
                        self.data_version = Some(version);
                        read_journal = false;
                        if !revisions.is_empty() {
                            return Ok(RevisionWaitOutcome::Revisions(revisions));
                        }
                    }
                }
            }
            let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                return Ok(RevisionWaitOutcome::TimedOut);
            };
            if remaining.is_zero() {
                return Ok(RevisionWaitOutcome::TimedOut);
            }
            poll_interval = if self.sleep(generation, poll_interval.min(remaining)) {
                FIRST_POLL_INTERVAL
            } else {
                (poll_interval * 2).min(MAX_POLL_INTERVAL)
            };
        }
    }

    /// Returns the commit generation, or the terminal outcome.
    fn generation(&self) -> std::result::Result<u64, RevisionWaitOutcome> {
        let state = self.signal.lock();
        if state.closed {
            Err(RevisionWaitOutcome::Closed)
        } else if self.cancelled.load(Ordering::SeqCst) {
            Err(RevisionWaitOutcome::Cancelled)
        } else {
            Ok(state.generation)
        }
    }

    /// Sleeps until a commit signal, close, cancellation, or `duration`.
    /// Returns whether it was woken rather than timed out.
    fn sleep(&self, generation: u64, duration: Duration) -> bool {
        let started = Instant::now();
        let mut state = self.signal.lock();
        loop {
            if state.generation != generation
                || state.closed
                || self.cancelled.load(Ordering::SeqCst)
            {
                return true;
            }
            let Some(remaining) = duration.checked_sub(started.elapsed()) else {
                return false;
            };
            if remaining.is_zero() {
                return false;
            }
            state = self
                .signal
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Reads the data version, or `None` while a writer keeps the file busy.
    fn observed_data_version(&self) -> Result<Option<i64>> {
        match self
            .connection
            .query_row("PRAGMA data_version", [], |row| row.get(0))
        {
            Ok(version) => Ok(Some(version)),
            Err(error) if is_busy(&error) => Ok(None),
            Err(error) => Err(sqlite_error("poll production data version")(error)),
        }
    }

    /// Reads one revision page, or `None` while a writer keeps the file busy.
    fn revisions_after(&self, after_sequence: u64, limit: u32) -> Result<Option<Vec<Revision>>> {
        let Ok(after_sequence) = i64::try_from(after_sequence) else {
            return Ok(Some(Vec::new()));
        };
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, sequence, transaction_id, committed_at_micros,
                        origin_name, origin_version, origin_uri, message
                 FROM revisions WHERE sequence > ?1
                 ORDER BY sequence LIMIT ?2",
            )
            .map_err(sqlite_error("prepare waiter revision query"))?;
        let rows = match statement.query_map(
            params![after_sequence, i64::from(limit)],
            stored_revision_row,
        ) {
            Ok(rows) => rows,
            Err(error) if is_busy(&error) => return Ok(None),
            Err(error) => return Err(sqlite_error("query waiter revisions")(error)),
        };
        let mut revisions = Vec::new();
        for row in rows {
            match row {
                Ok(row) => revisions.push(decode_revision(row)?),
                Err(error) if is_busy(&error) => return Ok(None),
                Err(error) => return Err(sqlite_error("read waiter revision row")(error)),
            }
        }
        Ok(Some(revisions))
    }
}

impl RevisionWaiter for SqliteRevisionWaiter {
    fn wait_for_revisions(
        &mut self,
        after_sequence: u64,
        limit: u32,
        timeout: Duration,
    ) -> Result<RevisionWaitOutcome> {
        Self::wait_for_revisions(self, after_sequence, limit, timeout)
    }
}

fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}
