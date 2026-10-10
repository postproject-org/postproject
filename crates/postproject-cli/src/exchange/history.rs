//! Recovery requires an explicit fresh scoped decision, never an ordinary open.

use std::{io::Write, path::PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;
use postproject_protocol::ProtocolBase;
use postproject_storage_sqlite::{ResynchronizationLimits, SqliteProduction};

use super::files;

#[derive(Debug, Subcommand)]
pub(crate) enum HistoryCommand {
    /// Save a coherent scoped mutation decision, including an incomplete head.
    Decision {
        production: PathBuf,
        output: PathBuf,
    },
    /// Establish a new floor for incomplete authority history, preserving evidence.
    Resynchronize {
        production: PathBuf,
        #[arg(long)]
        base: PathBuf,
        #[arg(long = "max-records", default_value_t = 10_000_000)]
        records: u64,
        #[arg(long = "max-encoded-bytes", default_value_t = 1_073_741_824)]
        encoded_bytes: u64,
    },
}

pub(super) fn execute(command: HistoryCommand, json: bool) -> Result<()> {
    match command {
        HistoryCommand::Decision { production, output } => {
            let source = SqliteProduction::open(production)?;
            let session = source.read_session()?;
            let decision = session.decision_base();
            let view = session.into_read_only();
            let base = ProtocolBase::new(view.exchange_scope()?, decision)?;
            files::publish(&output, |file| {
                file.write_all(&base.document().canonical_bytes()?)
                    .context("write scoped history decision")?;
                Ok(())
            })?;
            if json {
                super::super::print_json(
                    &serde_json::json!({"base": files::document(&base.document())?}),
                )?;
            } else {
                println!(
                    "decision for production {}, history {}, sequence {}",
                    base.scope().production(),
                    base.scope().history(),
                    base.decision().sequence()
                );
            }
        }
        HistoryCommand::Resynchronize {
            production,
            base,
            records,
            encoded_bytes,
        } => {
            let base = ProtocolBase::from_document(&files::load_document(&base)?)?;
            let limits = ResynchronizationLimits::new(records, encoded_bytes)?;
            let floor =
                SqliteProduction::open(production)?.resynchronize_exchange_history(base, limits)?;
            if json {
                super::super::print_json(
                    &serde_json::json!({"floor":files::document(&floor.document())?}),
                )?;
            } else {
                println!(
                    "retained replay floor {} in history {}",
                    floor.sequence(),
                    floor.scope().history()
                );
            }
        }
    }
    Ok(())
}
