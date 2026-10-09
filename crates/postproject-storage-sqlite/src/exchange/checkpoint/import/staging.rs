//! A private directory on the destination filesystem; closed-file publication.

use std::{
    fs::File,
    path::{Path, PathBuf},
};

use postproject_core::{Error, ErrorKind};
use rusqlite::Connection;
use tempfile::{TempDir, TempPath};

use crate::{ExchangeResult, migrations, open_connection, sqlite_error};

use super::limits::{CheckpointLimits, budget};

pub(super) struct Staging {
    path: TempPath,
    directory: TempDir,
    destination: PathBuf,
}

impl Staging {
    pub(super) fn new(
        destination: &Path,
        limits: CheckpointLimits,
    ) -> ExchangeResult<(Self, Connection)> {
        if destination.try_exists().map_err(|error| io_error(&error))? {
            return Err(
                Error::new(ErrorKind::AlreadyExists, "checkpoint destination exists").into(),
            );
        }
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let directory = tempfile::Builder::new()
            .prefix(".postproject-import-")
            .tempdir_in(parent)
            .map_err(|error| io_error(&error))?;
        let path = tempfile::Builder::new()
            .prefix("mirror-")
            .suffix(".pproj")
            .tempfile_in(directory.path())
            .map_err(|error| io_error(&error))?
            .into_temp_path();
        let mut connection = open_connection(&path)?;
        // New imports have no live readers. Reserve half the disk budget for
        // the rollback journal; never create a WAL or promote open handles.
        let page_size: u32 = connection
            .query_row("PRAGMA page_size", [], |row| row.get(0))
            .map_err(sqlite_error("read staging page size"))?;
        let pages = (limits.disk_bytes / 2 / u64::from(page_size)).min(4_294_967_294);
        if pages == 0 {
            return Err(budget().into());
        }
        connection.execute_batch(&format!("PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL; PRAGMA max_page_count = {pages};"))
            .map_err(sqlite_error("bound checkpoint staging disk"))?;
        migrations::migrate(&mut connection)?;
        let stage = Self {
            path,
            directory,
            destination: destination.to_path_buf(),
        };
        stage.check_disk(limits.disk_bytes)?;
        Ok((stage, connection))
    }

    pub(super) fn check_disk(&self, maximum: u64) -> ExchangeResult<()> {
        let mut size = 0_u64;
        for entry in std::fs::read_dir(self.directory.path()).map_err(|error| io_error(&error))? {
            size = size
                .checked_add(
                    entry
                        .map_err(|error| io_error(&error))?
                        .metadata()
                        .map_err(|error| io_error(&error))?
                        .len(),
                )
                .ok_or_else(budget)?;
        }
        if size > maximum {
            return Err(budget().into());
        }
        Ok(())
    }

    pub(super) fn promote(self, connection: Connection) -> ExchangeResult<PathBuf> {
        connection
            .close()
            .map_err(|(_, error)| sqlite_error("close checkpoint staging")(error))?;
        File::open(&self.path)
            .and_then(|file| file.sync_all())
            .map_err(|error| io_error(&error))?;
        promote(&self.path, &self.destination)?;
        Ok(self.destination)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn promote(source: &Path, destination: &Path) -> ExchangeResult<()> {
    // Fail on unsupported kernels/filesystems; do not fall back to overwriting
    // rename or link/unlink. Linux and Apple expose exclusive atomic rename.
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|error| io_error(&error.into()))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn promote(source: &Path, destination: &Path) -> ExchangeResult<()> {
    // tempfile's Windows implementation calls MoveFileExW without REPLACE_EXISTING.
    TempPath::try_from_path(source)
        .map_err(|error| io_error(&error))?
        .persist_noclobber(destination)
        .map_err(|error| io_error(&error.error))?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn promote(_: &Path, _: &Path) -> ExchangeResult<()> {
    Err(Error::new(
        ErrorKind::Unsupported,
        "atomic checkpoint promotion is unavailable on this platform",
    )
    .into())
}

fn io_error(error: &std::io::Error) -> Error {
    let kind = if error.kind() == std::io::ErrorKind::AlreadyExists {
        ErrorKind::AlreadyExists
    } else {
        ErrorKind::Io
    };
    Error::new(kind, format!("checkpoint staging filesystem: {error}"))
}
