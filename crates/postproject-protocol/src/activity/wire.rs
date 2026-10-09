use super::ActivityHeader;
use crate::{
    Document, Result,
    fields::{checked, exact, nullable, object, text, unsupported},
    identifier::{decode_identifier, encode_identifier},
};
use postproject_core::{ActivityKind, AgentIdentity, Timestamp, ToolIdentity};
use serde_json::{Value, json};

pub(super) fn encode(header: &ActivityHeader) -> Document {
    Document {
        value: json!({
            "kind":"activity.header", "id":header.id().to_string(), "activity_kind":header.kind().as_str(),
            "started_at_micros":header.started_at().map(|time| time.as_unix_micros().to_string()),
            "finished_at_micros":header.finished_at().map(|time| time.as_unix_micros().to_string()),
            "input_count":header.input_count().to_string(), "output_count":header.output_count().to_string(),
            "tool":header.tool().map(|tool| json!({"name":tool.name(), "version":tool.version(), "uri":tool.uri()})),
            "agent":header.agent().map(|agent| json!({"name":agent.name(), "identifier":agent.identifier().map(encode_identifier)})),
        }),
    }
}

pub(super) fn decode(document: &Document) -> Result<ActivityHeader> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "id",
            "activity_kind",
            "started_at_micros",
            "finished_at_micros",
            "input_count",
            "output_count",
            "tool",
            "agent",
        ],
    )?;
    if document.kind()? != "activity.header" {
        return Err(unsupported());
    }
    let mut header = ActivityHeader::new(
        exact(&fields["id"])?,
        checked(ActivityKind::new(text(&fields["activity_kind"])?))?,
        exact(&fields["input_count"])?,
        exact(&fields["output_count"])?,
    )?
    .with_timing(
        nullable(&fields["started_at_micros"], timestamp)?,
        nullable(&fields["finished_at_micros"], timestamp)?,
    )?;
    if let Some(tool) = nullable(&fields["tool"], decode_tool)? {
        header = header.with_tool(tool);
    }
    if let Some(agent) = nullable(&fields["agent"], decode_agent)? {
        header = header.with_agent(agent);
    }
    Ok(header)
}

fn timestamp(value: &Value) -> Result<Timestamp> {
    Ok(Timestamp::from_unix_micros(exact(value)?))
}
fn owned_text(value: &Value) -> Result<String> {
    Ok(text(value)?.to_owned())
}
fn decode_tool(value: &Value) -> Result<ToolIdentity> {
    let fields = object(value, &["name", "version", "uri"])?;
    checked(ToolIdentity::new(
        text(&fields["name"])?,
        nullable(&fields["version"], owned_text)?,
        nullable(&fields["uri"], owned_text)?,
    ))
}
fn decode_agent(value: &Value) -> Result<AgentIdentity> {
    let fields = object(value, &["name", "identifier"])?;
    checked(AgentIdentity::new(
        nullable(&fields["name"], owned_text)?,
        nullable(&fields["identifier"], decode_identifier)?,
    ))
}
