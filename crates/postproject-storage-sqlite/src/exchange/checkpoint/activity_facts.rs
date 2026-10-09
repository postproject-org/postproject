//! Scalar provenance reads leave edges and historical evidence as streams.

use postproject_core::{ActivityId, ActivityKind, AgentIdentity, Timestamp, ToolIdentity};
use postproject_protocol::ActivityHeader;
use rusqlite::Connection;

use crate::{ExchangeResult, StoredActivity, read_budget, sqlite_error, stored_domain_error};

#[cfg(test)]
mod tests;

pub(super) fn header(connection: &Connection, id: ActivityId) -> ExchangeResult<ActivityHeader> {
    let (stored, inputs, outputs) = connection.query_row(
        "SELECT id, kind, started_at_micros, finished_at_micros, tool_name, tool_version, tool_uri, agent_name, agent_identifier_scheme, agent_identifier_value, agent_identifier_qualifier, (SELECT COUNT(*) FROM activity_inputs WHERE activity_id = a.id), (SELECT COUNT(*) FROM activity_outputs WHERE activity_id = a.id) FROM activities a WHERE id = ?1",
        [id.as_bytes().as_slice()],
        read_budget::bounded(|row| Ok((StoredActivity {
            id: row.get(0)?, kind: row.get(1)?, started_at: row.get(2)?, finished_at: row.get(3)?,
            tool_name: row.get(4)?, tool_version: row.get(5)?, tool_uri: row.get(6)?,
            agent_name: row.get(7)?, agent_scheme: row.get(8)?, agent_value: row.get(9)?, agent_qualifier: row.get(10)?,
        }, row.get::<_, i64>(11)?, row.get::<_, i64>(12)?))),
    ).map_err(sqlite_error("read checkpoint activity attribution"))?;
    if crate::id_bytes(stored.id, "checkpoint activity identity")? != id.into_bytes() {
        return Err(super::invalid().into());
    }
    let mut header = ActivityHeader::new(
        id,
        ActivityKind::new(stored.kind).map_err(stored_domain_error("checkpoint activity kind"))?,
        crate::stored_u64(inputs, "checkpoint activity input count")?,
        crate::stored_u64(outputs, "checkpoint activity output count")?,
    )?
    .with_timing(
        stored.started_at.map(Timestamp::from_unix_micros),
        stored.finished_at.map(Timestamp::from_unix_micros),
    )?;
    match (stored.tool_name, stored.tool_version, stored.tool_uri) {
        (Some(name), version, uri) => {
            header = header.with_tool(
                ToolIdentity::new(name, version, uri)
                    .map_err(stored_domain_error("checkpoint activity tool"))?,
            );
        }
        (None, None, None) => {}
        _ => return Err(super::invalid().into()),
    }
    let identifier = match (
        stored.agent_scheme,
        stored.agent_value,
        stored.agent_qualifier,
    ) {
        (Some(scheme), Some(value), qualifier) => {
            Some(crate::decode_external_identifier(scheme, value, qualifier)?)
        }
        (None, None, None) => None,
        _ => return Err(super::invalid().into()),
    };
    if stored.agent_name.is_some() || identifier.is_some() {
        header = header.with_agent(
            AgentIdentity::new(stored.agent_name, identifier)
                .map_err(stored_domain_error("checkpoint activity agent"))?,
        );
    }
    Ok(header)
}
