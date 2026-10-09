//! Work intent carries durations and public attribution, never worker secrets.

use std::time::Duration;

use postproject_core::{
    AgentIdentity, Job, JobFailure, JobState, MAX_JOB_INPUTS, ToolIdentity,
    validate_job_lease_duration,
};
use serde_json::{Value, json};

use super::{Command, activity, prepared};
use crate::{
    Document, JobHeader, Result,
    fields::{array, checked, exact, malformed, nullable, object, text, unsupported},
    identifier::{decode_identifier, encode_identifier},
};

pub(super) fn encode(command: &Command) -> Result<Value> {
    Ok(match command {
        Command::RequestJob(job) => {
            if !matches!(job.state(), JobState::Requested) {
                return Err(malformed());
            }
            json!({"kind":"job.request", "header":JobHeader::from_job(job)?.document()?.value,
                "inputs":job.inputs().iter().map(ToString::to_string).collect::<Vec<_>>()})
        }
        Command::ClaimJob {
            job_id,
            tool,
            agent,
            duration,
        } => json!({"kind":"job.claim",
            "id":job_id.to_string(), "duration_micros":checked(validate_job_lease_duration(*duration))?.to_string(),
            "tool":{"name":tool.name(), "version":tool.version(), "uri":tool.uri()},
            "agent":agent.as_ref().map(|agent| json!({"name":agent.name(), "identifier":agent.identifier().map(encode_identifier)}))}),
        Command::RenewJob { job_id, duration } => {
            json!({"kind":"job.renew", "id":job_id.to_string(),
            "duration_micros":checked(validate_job_lease_duration(*duration))?.to_string()})
        }
        Command::ReleaseJob(id) => json!({"kind":"job.release", "id":id.to_string()}),
        Command::FailJob { job_id, failure } => json!({"kind":"job.fail", "id":job_id.to_string(),
            "diagnostic":failure.diagnostic()}),
        Command::CompleteJob {
            job_id,
            output,
            activity: intent,
        } => json!({"kind":"job.complete", "id":job_id.to_string(),
            "output":prepared::encode_media(output.representation(), output.resources(), output.locators())?,
            "activity":activity::encode(intent)?}),
        Command::CancelJob(id) => json!({"kind":"job.cancel", "id":id.to_string()}),
        _ => return Err(unsupported()),
    })
}

pub(super) fn decode(kind: &str, value: &Value) -> Result<Command> {
    Ok(match kind {
        "job.request" => {
            let fields = object(value, &["kind", "header", "inputs"])?;
            let header = JobHeader::from_document(&Document {
                value: fields["header"].clone(),
            })?;
            if !matches!(header.state(), JobState::Requested) {
                return Err(malformed());
            }
            let inputs = array(&fields["inputs"], MAX_JOB_INPUTS)?
                .iter()
                .map(exact)
                .collect::<Result<Vec<_>>>()?;
            if u64::try_from(inputs.len()).map_err(|_| malformed())? != header.input_count()
                || inputs.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(malformed());
            }
            Command::RequestJob(checked(Job::new(
                header.id(),
                header.kind().clone(),
                inputs,
                header.requested_output().clone(),
            ))?)
        }
        "job.claim" => {
            let fields = object(value, &["kind", "id", "duration_micros", "tool", "agent"])?;
            Command::ClaimJob {
                job_id: exact(&fields["id"])?,
                duration: duration(&fields["duration_micros"])?,
                tool: tool(&fields["tool"])?,
                agent: nullable(&fields["agent"], agent)?,
            }
        }
        "job.renew" => {
            let fields = object(value, &["kind", "id", "duration_micros"])?;
            Command::RenewJob {
                job_id: exact(&fields["id"])?,
                duration: duration(&fields["duration_micros"])?,
            }
        }
        "job.release" | "job.cancel" => {
            let fields = object(value, &["kind", "id"])?;
            let id = exact(&fields["id"])?;
            if kind == "job.release" {
                Command::ReleaseJob(id)
            } else {
                Command::CancelJob(id)
            }
        }
        "job.fail" => {
            let fields = object(value, &["kind", "id", "diagnostic"])?;
            Command::FailJob {
                job_id: exact(&fields["id"])?,
                failure: checked(JobFailure::new(text(&fields["diagnostic"])?))?,
            }
        }
        "job.complete" => {
            let fields = object(value, &["kind", "id", "output", "activity"])?;
            let intent = &fields["activity"];
            if text(intent.get("kind").ok_or_else(malformed)?)? != "activity.create" {
                return Err(unsupported());
            }
            Command::CompleteJob {
                job_id: exact(&fields["id"])?,
                output: Box::new(prepared::decode_media(&fields["output"])?),
                activity: Box::new(activity::decode(intent)?),
            }
        }
        _ => return Err(unsupported()),
    })
}

fn duration(value: &Value) -> Result<Duration> {
    let duration = Duration::from_micros(exact(value)?);
    checked(validate_job_lease_duration(duration))?;
    Ok(duration)
}

fn owned_text(value: &Value) -> Result<String> {
    Ok(text(value)?.to_owned())
}

fn tool(value: &Value) -> Result<ToolIdentity> {
    let fields = object(value, &["name", "version", "uri"])?;
    checked(ToolIdentity::new(
        text(&fields["name"])?,
        nullable(&fields["version"], owned_text)?,
        nullable(&fields["uri"], owned_text)?,
    ))
}

fn agent(value: &Value) -> Result<AgentIdentity> {
    let fields = object(value, &["name", "identifier"])?;
    checked(AgentIdentity::new(
        nullable(&fields["name"], owned_text)?,
        nullable(&fields["identifier"], decode_identifier)?,
    ))
}
