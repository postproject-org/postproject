use postproject_core::{
    AgentIdentity, JobClaim, JobCompletion, JobFailure, JobState, Timestamp, ToolIdentity,
};
use serde_json::{Value, json};

use crate::{
    Result,
    fields::{checked, exact, malformed, nullable, object, text, unsupported},
    identifier::{decode_identifier, encode_identifier},
};

pub(super) fn encode(state: &JobState) -> Result<Value> {
    Ok(match state {
        JobState::Requested => json!({"kind":"requested"}),
        JobState::Cancelled => json!({"kind":"cancelled"}),
        JobState::Succeeded(completion) => {
            json!({"kind":"succeeded", "activity_id":completion.activity_id().to_string(), "representation_id":completion.representation_id().to_string()})
        }
        JobState::Failed(failure) => json!({"kind":"failed", "diagnostic":failure.diagnostic()}),
        JobState::Claimed(claim) => {
            json!({"kind":"claimed", "expires_at_micros":claim.expires_at().as_unix_micros().to_string(), "tool":{"name":claim.tool().name(), "version":claim.tool().version(), "uri":claim.tool().uri()}, "agent":claim.agent().map(|agent|json!({"name":agent.name(), "identifier":agent.identifier().map(encode_identifier)}))})
        }
        _ => return Err(unsupported()),
    })
}

pub(super) fn decode(value: &Value) -> Result<JobState> {
    Ok(match text(value.get("kind").ok_or_else(malformed)?)? {
        "requested" | "cancelled" => {
            let fields = object(value, &["kind"])?;
            if text(&fields["kind"])? == "requested" {
                JobState::Requested
            } else {
                JobState::Cancelled
            }
        }
        "succeeded" => {
            let fields = object(value, &["kind", "activity_id", "representation_id"])?;
            JobState::Succeeded(JobCompletion::new(
                exact(&fields["activity_id"])?,
                exact(&fields["representation_id"])?,
            ))
        }
        "failed" => {
            let fields = object(value, &["kind", "diagnostic"])?;
            JobState::Failed(checked(JobFailure::new(text(&fields["diagnostic"])?))?)
        }
        "claimed" => {
            let fields = object(value, &["kind", "expires_at_micros", "tool", "agent"])?;
            let tool = object(&fields["tool"], &["name", "version", "uri"])?;
            let tool = checked(ToolIdentity::new(
                text(&tool["name"])?,
                nullable(&tool["version"], owned_text)?,
                nullable(&tool["uri"], owned_text)?,
            ))?;
            let agent = nullable(&fields["agent"], |value| {
                let fields = object(value, &["name", "identifier"])?;
                checked(AgentIdentity::new(
                    nullable(&fields["name"], owned_text)?,
                    nullable(&fields["identifier"], decode_identifier)?,
                ))
            })?;
            JobState::Claimed(JobClaim::new(
                tool,
                agent,
                Timestamp::from_unix_micros(exact(&fields["expires_at_micros"])?),
            ))
        }
        _ => return Err(unsupported()),
    })
}

fn owned_text(value: &Value) -> Result<String> {
    Ok(text(value)?.to_owned())
}
