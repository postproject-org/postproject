//! Validation uses the same complete private materialization as mirror import.

use std::{fs::File, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use postproject_protocol::Limits;
use postproject_storage_sqlite::{
    CheckpointLimits, SqliteProduction,
    exchange_file::{self, FileLimits},
};

use super::files;

#[derive(Debug, Args)]
pub(crate) struct Receiver {
    /// Maximum encoded file bytes; raise explicitly for larger productions.
    #[arg(long = "max-encoded-bytes", default_value_t = 1_073_741_824)]
    encoded_bytes: u64,
    /// Maximum private SQLite database and journal bytes.
    #[arg(long = "max-disk-bytes", default_value_t = 2_147_483_648)]
    disk_bytes: u64,
    /// Maximum domain frames and dependency traversal work.
    #[arg(long = "max-frames", default_value_t = 10_000_000)]
    frames: u64,
}

impl Receiver {
    fn limits(&self) -> Result<(FileLimits, CheckpointLimits)> {
        let document = Limits::default();
        Ok((
            FileLimits::new(self.encoded_bytes, document)?,
            CheckpointLimits::new(self.encoded_bytes, self.disk_bytes, self.frames, document)?,
        ))
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum CheckpointCommand {
    /// Export one pinned complete view to a new sealed file.
    Export {
        production: PathBuf,
        output: PathBuf,
    },
    /// Validate all domain facts in private temporary storage, then discard it.
    Validate {
        input: PathBuf,
        #[command(flatten)]
        limits: Receiver,
    },
    /// Create a new passive mirror; existing destinations reject.
    Import {
        input: PathBuf,
        destination: PathBuf,
        #[command(flatten)]
        limits: Receiver,
    },
}

pub(super) fn execute(command: CheckpointCommand, json: bool) -> Result<()> {
    match command {
        CheckpointCommand::Export { production, output } => {
            let source = SqliteProduction::open(production)?;
            let manifest = files::publish(&output, |file| {
                Ok(exchange_file::write_checkpoint(&source, file)?)
            })?;
            files::report(manifest.head(), json)
        }
        CheckpointCommand::Import {
            input,
            destination,
            limits,
        } => {
            let (file_limits, checkpoint_limits) = limits.limits()?;
            let mirror = exchange_file::import_checkpoint(
                destination,
                File::open(input).context("open sealed checkpoint")?,
                file_limits,
                checkpoint_limits,
            )?;
            files::report(mirror.exchange_head()?, json)
        }
        CheckpointCommand::Validate { input, limits } => {
            let (file_limits, checkpoint_limits) = limits.limits()?;
            let directory =
                tempfile::tempdir().context("create private checkpoint validation directory")?;
            let mirror = exchange_file::import_checkpoint(
                directory.path().join("validation.pproj"),
                File::open(input).context("open sealed checkpoint")?,
                file_limits,
                checkpoint_limits,
            )?;
            let head = mirror.exchange_head()?;
            drop(mirror);
            files::report(head, json)
        }
    }
}
