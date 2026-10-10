//! Private file delivery is separate from retained public outcomes.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    str::FromStr,
};

use anyhow::{Context, Result};
use clap::Args;
use postproject_core::JobId;
use postproject_protocol::{Command, MAX_PROPOSAL_COMMANDS, Outcome, OutcomeStatus, Proposal};
use postproject_storage_sqlite::SqliteProduction;

use crate::job_transport::{self, CommittedOperationError, TokenOutput};

#[derive(Debug, Args)]
pub(crate) struct WorkerArgs {
    /// Private scoped token file; repeat for distinct jobs, or use '-' once.
    #[arg(long = "lease-token-file")]
    tokens: Vec<PathBuf>,
    /// New private credential destination as `JOB_UUID=PATH`; repeat for claims.
    #[arg(long = "claim-output")]
    outputs: Vec<ClaimOutput>,
}

#[derive(Clone, Debug)]
struct ClaimOutput {
    job: JobId,
    path: PathBuf,
}

impl FromStr for ClaimOutput {
    type Err = &'static str;
    fn from_str(text: &str) -> std::result::Result<Self, Self::Err> {
        let (job, path) = text
            .split_once('=')
            .ok_or("claim output needs JOB_UUID=PATH")?;
        if path.is_empty() || path == "-" {
            return Err("claim output needs a new private file path");
        }
        Ok(Self {
            job: job.parse().map_err(|_| "invalid claim job identity")?,
            path: PathBuf::from(path),
        })
    }
}

pub(super) fn submit(
    source: &mut SqliteProduction,
    proposal: &Proposal,
    args: WorkerArgs,
) -> Result<Outcome> {
    if args.tokens.len() > MAX_PROPOSAL_COMMANDS
        || args.outputs.len() > MAX_PROPOSAL_COMMANDS
        || args
            .tokens
            .iter()
            .filter(|path| path.as_os_str() == "-")
            .count()
            > 1
    {
        return Err(crate::errors::invalid("private submission context exceeds its limit").into());
    }
    let tokens = args
        .tokens
        .iter()
        .map(|path| job_transport::read_scoped_token(path))
        .collect::<Result<Vec<_>>>()?;
    let tokens = tokens
        .iter()
        .map(|input| input.token.as_str())
        .collect::<Vec<_>>();
    let retained = source
        .submission_outcome(proposal.scope(), proposal.client(), proposal.request())?
        .is_some();
    // Recovery must not reserve/recreate credential files, even after expiry.
    // Storage still compares the exact intent and private binding on every retry.
    let mut outputs = if retained {
        BTreeMap::new()
    } else {
        reserve(proposal, args.outputs)?
    };
    let result = source.submit_proposal_with_capabilities(proposal, &[], &tokens)?;
    let (outcome, leases) = result.into_parts();
    let delivery = || -> Result<()> {
        for lease in leases {
            let output = outputs.remove(&lease.job_id()).ok_or_else(|| {
                crate::errors::invalid("new claim has no private delivery destination")
            })?;
            output
                .deliver(&lease.export_token()?)
                .context("deliver newly committed claim")?;
        }
        Ok(())
    }();
    if let Err(error) = delivery {
        let receipts = if let OutcomeStatus::Accepted(receipt) = outcome.status() {
            std::slice::from_ref(receipt)
        } else {
            &[]
        };
        return Err(CommittedOperationError::attach(error, receipts));
    }
    Ok(outcome)
}

fn reserve(
    proposal: &Proposal,
    destinations: Vec<ClaimOutput>,
) -> Result<BTreeMap<JobId, TokenOutput>> {
    let claims = proposal
        .commands()
        .iter()
        .filter_map(|command| {
            if let Command::ClaimJob { job_id, .. } = command {
                Some(*job_id)
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    let mut paths = BTreeSet::new();
    let mut outputs = BTreeMap::new();
    for destination in destinations {
        if !claims.contains(&destination.job)
            || outputs.contains_key(&destination.job)
            || !paths.insert(destination.path.clone())
        {
            return Err(crate::errors::invalid(
                "claim outputs must name distinct requested claims and file paths",
            )
            .into());
        }
        let output =
            TokenOutput::reserve(&destination.path).context("reserve private claim destination")?;
        outputs.insert(destination.job, output);
    }
    if outputs.len() != claims.len() {
        return Err(
            crate::errors::invalid("each new claim requires a private claim-output file").into(),
        );
    }
    Ok(outputs)
}
