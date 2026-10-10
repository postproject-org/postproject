//! New output files are published only after a complete, flushed stream.

use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Subcommand;
use postproject_core::{Error, ErrorKind};
use postproject_protocol::{Document, Limits, Position};
use postproject_storage_sqlite::{
    ReplayLimits, SqliteProduction,
    exchange_file::{self, FileLimits},
};

#[derive(Debug, Subcommand)]
pub(crate) enum ChangesCommand {
    /// Export complete records through one pinned head to a new file.
    Export {
        production: PathBuf,
        output: PathBuf,
        #[arg(long)]
        from: PathBuf,
    },
    /// Apply complete records to a passive mirror; retry the same file on failure.
    Apply {
        production: PathBuf,
        input: PathBuf,
        #[arg(long, default_value_t = 1_073_741_824)]
        max_encoded_bytes: u64,
    },
}

pub(super) fn changes(command: ChangesCommand, json: bool) -> Result<()> {
    let through = match command {
        ChangesCommand::Export {
            production,
            output,
            from,
        } => {
            let from = load_position(&from)?;
            let source = SqliteProduction::open(production)?;
            publish(&output, |file| {
                Ok(exchange_file::write_changes(&source, from, file)?)
            })?
        }
        ChangesCommand::Apply {
            production,
            input,
            max_encoded_bytes,
        } => {
            let limits = FileLimits::new(max_encoded_bytes, Limits::default())?;
            let replay = ReplayLimits::new(max_encoded_bytes, Limits::default())?;
            let input = File::open(input).context("open complete changes file")?;
            exchange_file::apply_changes(
                &mut SqliteProduction::open(production)?,
                input,
                limits,
                replay,
            )?
        }
    };
    report(through, json)
}

pub(super) fn publish<T>(path: &Path, write: impl FnOnce(&mut File) -> Result<T>) -> Result<T> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut output =
        tempfile::NamedTempFile::new_in(parent).context("create private exchange output")?;
    let result = write(output.as_file_mut())?;
    output
        .as_file()
        .sync_all()
        .context("flush complete exchange output")?;
    output.persist_noclobber(path).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            anyhow::Error::new(Error::new(
                ErrorKind::AlreadyExists,
                "exchange output already exists",
            ))
        } else {
            anyhow::Error::new(error.error).context("publish complete exchange output")
        }
    })?;
    Ok(result)
}

pub(super) fn document(document: &Document) -> Result<serde_json::Value> {
    Ok(serde_json::from_slice(&document.canonical_bytes()?)?)
}

pub(super) fn report(position: Position, json: bool) -> Result<()> {
    if json {
        super::super::print_json(&serde_json::json!({"head":document(&position.document())?}))?;
    } else {
        println!(
            "production {}, history {}, sequence {}",
            position.scope().production(),
            position.scope().history(),
            position.sequence()
        );
    }
    Ok(())
}

pub(super) fn position(production: &Path, output: &Path, json: bool) -> Result<()> {
    let head = SqliteProduction::open(production)?.exchange_head()?;
    publish(output, |file| {
        file.write_all(&head.document().canonical_bytes()?)
            .context("write scoped exchange position")?;
        Ok(())
    })?;
    report(head, json)
}

pub(super) fn load_position(path: &Path) -> Result<Position> {
    use std::io::Read;
    let limits = Limits::new(4096, 8, 64)?;
    let mut bytes = Vec::new();
    File::open(path)
        .context("open scoped position")?
        .take(4097)
        .read_to_end(&mut bytes)
        .context("read bounded scoped position")?;
    Ok(Position::from_document(&Document::parse(&bytes, limits)?)?)
}
