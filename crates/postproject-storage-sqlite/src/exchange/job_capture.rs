//! Job state is copied at staging; immutable request inputs stream at commit.

use postproject_core::{Error, ErrorKind, JobId, JobKind, RequestedJobOutput, Result};
use postproject_protocol::{Document, JobHeader, JobInput};
use rusqlite::{Connection, OptionalExtension};

use crate::{
    decode_job_state, decode_representation_kind, id_bytes, sqlite_error, stored_job_row,
    stored_u64,
};

pub(crate) fn header(connection: &Connection, job: JobId) -> Result<JobHeader> {
    let stored = connection
        .query_row(
            "SELECT id, kind, output_asset_id, output_representation_kind,
        target_root, state, claim_id, claim_tool_name, claim_tool_version, claim_tool_uri,
        claim_agent_name, claim_agent_scheme, claim_agent_value, claim_agent_qualifier,
        claim_expires_at_micros, completion_activity_id, completion_representation_id,
        failure_diagnostic, claim_inert FROM jobs WHERE id = ?1",
            [job.as_bytes().as_slice()],
            stored_job_row,
        )
        .optional()
        .map_err(sqlite_error("load staged job facts"))?
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "job does not exist"))?;
    let state = decode_job_state(connection, &stored)?;
    let inputs: i64 = connection
        .query_row(
            "SELECT count(*) FROM job_inputs WHERE job_id = ?1",
            [job.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("count job inputs"))?;
    JobHeader::new(
        job,
        JobKind::new(stored.kind)?,
        stored_u64(inputs, "job inputs")?,
        RequestedJobOutput::new(
            postproject_core::AssetId::from_bytes(id_bytes(
                stored.output_asset_id,
                "job output asset",
            )?),
            decode_representation_kind(stored.output_representation_kind)?,
            stored.target_root,
        )?,
        state,
    )
    .map_err(|_| invalid())
}

pub(crate) fn write<E: From<Error>>(
    connection: &Connection,
    header: &JobHeader,
    mut write: impl FnMut(&Document) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    write(&header.document().map_err(|_| invalid())?)?;
    let mut statement = connection.prepare("SELECT position, representation_id FROM job_inputs WHERE job_id = ?1 ORDER BY position")
        .map_err(sqlite_error("prepare captured job inputs"))?;
    let mut rows = statement
        .query([header.id().as_bytes().as_slice()])
        .map_err(sqlite_error("query captured job inputs"))?;
    let mut count = 0_u64;
    let mut previous = None;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read captured job input"))?
    {
        let position = stored_u64(
            row.get(0).map_err(sqlite_error("job input position"))?,
            "job input position",
        )?;
        let representation = postproject_core::RepresentationId::from_bytes(id_bytes(
            row.get(1).map_err(sqlite_error("job input identity"))?,
            "job input",
        )?);
        if position != count
            || count >= header.input_count()
            || previous.is_some_and(|id| id >= representation)
        {
            return Err(invalid().into());
        }
        write(
            &JobInput::new(header.id(), count, representation)
                .map_err(|_| invalid())?
                .document(),
        )?;
        count += 1;
        previous = Some(representation);
    }
    if count != header.input_count() {
        return Err(invalid().into());
    }
    Ok(())
}

fn invalid() -> Error {
    Error::new(ErrorKind::Storage, "invalid staged job facts")
}
