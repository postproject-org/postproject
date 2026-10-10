//! Trusted local files and durable metadata submission outcomes.

mod checkpoint;
mod files;

use std::{
    fmt,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use postproject_protocol::{
    ClientId, Document, HistoryId, Limits, Outcome, OutcomeStatus, Proposal, RequestId, Scope,
    StoreRole,
};
use postproject_storage_sqlite::SqliteProduction;

#[derive(Debug, Args)]
pub(crate) struct ExchangeArgs {
    #[command(subcommand)]
    command: ExchangeCommand,
}

#[derive(Debug, Subcommand)]
enum ExchangeCommand {
    /// Inspect source scope, passive role and retained replay floor.
    Inspect { production: PathBuf },
    /// Save the exact current source continuation for a later changes export.
    Position {
        production: PathBuf,
        output: PathBuf,
    },
    /// Export or apply complete contiguous records from a scoped position.
    Changes {
        #[command(subcommand)]
        command: files::ChangesCommand,
    },
    /// Export, validate or import a complete portable checkpoint.
    Checkpoint {
        #[command(subcommand)]
        command: checkpoint::CheckpointCommand,
    },
    /// Submit a bounded portable proposal; retry the same file/identity.
    Submit {
        production: PathBuf,
        proposal: PathBuf,
    },
    /// Recover a retained public outcome without reapplying the request.
    Outcome {
        production: PathBuf,
        #[arg(long)]
        history: HistoryId,
        #[arg(long)]
        client: ClientId,
        #[arg(long)]
        request: RequestId,
    },
}

#[derive(Debug)]
pub(crate) struct RejectedOutcome(pub(crate) Outcome);

impl fmt::Display for RejectedOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "submission {} rejected; its terminal outcome is retained",
            self.0.request()
        )
    }
}

impl std::error::Error for RejectedOutcome {}

fn public_outcome(outcome: &Outcome) -> Result<serde_json::Value> {
    let bytes = outcome.document()?.canonical_bytes()?;
    // This is serialization of already checked canonical protocol data, not a
    // second input decoder. Exact integer strings remain strings in the CLI.
    serde_json::from_slice(&bytes).context("encode public submission outcome")
}

pub(crate) fn print_rejection(error: &RejectedOutcome) -> Result<()> {
    super::print_json(&serde_json::json!({
        "outcome":public_outcome(&error.0)?,
        "error":{"code":super::errors::category(&anyhow::Error::new(RejectedOutcome(error.0.clone()))).0,"message":"submission rejected"}
    }))
}

fn load_proposal(path: &Path) -> Result<Proposal> {
    let limits = Limits::default();
    let file = File::open(path).context("open proposal file")?;
    let mut bytes = Vec::new();
    file.take(u64::try_from(limits.max_bytes())? + 1)
        .read_to_end(&mut bytes)
        .context("read bounded proposal")?;
    Ok(Proposal::from_document(&Document::parse(&bytes, limits)?)?)
}

fn inspect(production: &Path, json: bool) -> Result<()> {
    let production = SqliteProduction::open(production)?;
    let view = production.read_session()?.into_read_only();
    let scope = view.exchange_scope()?;
    let floor = view.exchange_floor()?;
    let role = match view.exchange_role() {
        StoreRole::Authority => "authority",
        StoreRole::PassiveMirror => "passive_mirror",
    };
    let revision = view.latest_revision()?;
    let head = match view.exchange_head() {
        Ok(head) => Some(head),
        Err(postproject_storage_sqlite::ExchangeError::Protocol(error))
            if error.kind() == postproject_protocol::FailureKind::HistoryGap =>
        {
            None
        }
        Err(error) => return Err(error.into()),
    };
    if json {
        super::print_json(&serde_json::json!({
            "scope":{"production":scope.production().to_string(),"history":scope.history().to_string()},
            "role":role,"protocol_version":"1","supported_features":["checkpoint-archives.v1", "checkpoints.v1", "dependencies.v1", "jobs.v1", "media.v1", "metadata.v1", "outcomes.v1", "provenance.v1", "record-chunks.v1"],
            "limits":{"proposal_bytes":Limits::default().max_bytes().to_string(),"proposal_commands":postproject_protocol::MAX_PROPOSAL_COMMANDS.to_string()},
            "floor":{"revision":floor.revision().map(|id| id.to_string()),"sequence":floor.sequence().to_string(),"digest":floor.digest().to_string()},
            "head":head.map(|head| files::document(&head.document())).transpose()?,
            "replay_status":if head.is_some() {"available"} else {"history_gap"},
            "observed_head":{"revision":revision.as_ref().map(|revision| revision.id().to_string()),"sequence":revision.as_ref().map_or(0, postproject_core::Revision::sequence).to_string()}
        }))?;
    } else {
        println!(
            "{role}: production {}, history {}, replay floor {}",
            scope.production(),
            scope.history(),
            floor.sequence()
        );
    }
    Ok(())
}

pub(crate) fn execute(args: ExchangeArgs, json: bool, has_external_base: bool) -> Result<()> {
    if has_external_base {
        return Err(super::errors::invalid("exchange commands use the proposal's scoped base; global decision options are not accepted").into());
    }
    match args.command {
        ExchangeCommand::Inspect { production } => inspect(&production, json),
        ExchangeCommand::Position { production, output } => {
            files::position(&production, &output, json)
        }
        ExchangeCommand::Changes { command } => files::changes(command, json),
        ExchangeCommand::Checkpoint { command } => checkpoint::execute(command, json),
        ExchangeCommand::Submit {
            production,
            proposal,
        } => {
            let proposal = load_proposal(&proposal)?;
            let outcome = SqliteProduction::open(production)?.submit_proposal(&proposal)?;
            if matches!(outcome.status(), OutcomeStatus::Rejected(_)) {
                return Err(RejectedOutcome(outcome).into());
            }
            if json {
                super::print_json(&serde_json::json!({"outcome":public_outcome(&outcome)?}))?;
            } else {
                println!("submission {} accepted", outcome.request());
            }
            Ok(())
        }
        ExchangeCommand::Outcome {
            production,
            history,
            client,
            request,
        } => {
            let production = SqliteProduction::open(production)?;
            let scope = Scope::new(production.production().id(), history);
            let outcome = production.submission_outcome(scope, client, request)?;
            if json {
                super::print_json(
                    &serde_json::json!({"outcome":outcome.as_ref().map(public_outcome).transpose()?}),
                )?;
            } else {
                println!(
                    "submission {request}: {}",
                    if outcome.is_some() {
                        "retained"
                    } else {
                        "unknown"
                    }
                );
            }
            Ok(())
        }
    }
}
