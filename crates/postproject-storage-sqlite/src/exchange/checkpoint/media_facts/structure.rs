use postproject_core::{
    ContentStructure, FrameRange, ImageSequenceDescriptor, MAX_CONTENT_MEMBERS,
    MAX_SEQUENCE_EXCEPTIONS, RationalRate, RepresentationId, ResourceId, ResourceMember,
    ResourceRole,
};
use rusqlite::Connection;

use crate::{ExchangeResult, sqlite_error};

use super::super::invalid;

pub(in crate::exchange::checkpoint) fn content(
    connection: &Connection,
    owner: RepresentationId,
) -> ExchangeResult<ContentStructure> {
    let kind: i64 = connection
        .query_row(
            "SELECT structure_kind FROM representations WHERE id = ?1",
            [owner.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("read checkpoint content kind"))?;
    let mut statement = connection.prepare("SELECT resource_id, position, role, required FROM representation_resources WHERE representation_id = ?1 ORDER BY position")
        .map_err(sqlite_error("prepare checkpoint membership"))?;
    let mut rows = statement
        .query([owner.as_bytes().as_slice()])
        .map_err(sqlite_error("query checkpoint membership"))?;
    let mut members = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint membership"))?
    {
        let position: i64 = row
            .get(1)
            .map_err(sqlite_error("read membership position"))?;
        if usize::try_from(position).ok() != Some(members.len())
            || members.len() >= MAX_CONTENT_MEMBERS
        {
            return Err(invalid().into());
        }
        members.push((
            ResourceId::from_bytes(crate::id_bytes(
                row.get(0)
                    .map_err(sqlite_error("read membership resource"))?,
                "member",
            )?),
            row.get::<_, Option<String>>(2)
                .map_err(sqlite_error("read membership role"))?,
            row.get::<_, bool>(3)
                .map_err(sqlite_error("read membership requirement"))?,
        ));
    }
    let single = || match members.as_slice() {
        [(id, None, true)] => Ok(*id),
        _ => Err(invalid()),
    };
    let sequence_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM image_sequences WHERE representation_id = ?1",
            [owner.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check sequence discriminator"))?;
    if sequence_count != i64::from(kind == 1) {
        return Err(invalid().into());
    }
    match kind {
        0 => Ok(ContentStructure::single_resource(single()?)),
        1 => sequence(connection, owner, single()?),
        2 | 3 => {
            let members = members
                .into_iter()
                .map(|(id, role, required)| {
                    Ok(ResourceMember::new(
                        id,
                        ResourceRole::new(role.ok_or_else(invalid)?)?,
                        required,
                    ))
                })
                .collect::<ExchangeResult<Vec<_>>>()?;
            Ok(if kind == 2 {
                ContentStructure::ordered_parts(members)?
            } else {
                ContentStructure::package(members)?
            })
        }
        _ => Err(invalid().into()),
    }
}

fn sequence(
    connection: &Connection,
    owner: RepresentationId,
    resource: ResourceId,
) -> ExchangeResult<ContentStructure> {
    let fields = connection.query_row("SELECT resource_id, start_frame, end_frame, frame_step, rate_numerator, rate_denominator FROM image_sequences WHERE representation_id = ?1", [owner.as_bytes().as_slice()], |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, u32>(3)?, row.get::<_, u32>(4)?, row.get::<_, u32>(5)?)))
    .map_err(sqlite_error("read checkpoint sequence"))?;
    if crate::id_bytes(fields.0, "sequence resource")? != resource.into_bytes() {
        return Err(invalid().into());
    }
    let mut statement = connection.prepare("SELECT frame FROM image_sequence_missing_frames WHERE representation_id = ?1 ORDER BY frame")
    .map_err(sqlite_error("prepare checkpoint exceptions"))?;
    let mut rows = statement
        .query([owner.as_bytes().as_slice()])
        .map_err(sqlite_error("query checkpoint exceptions"))?;
    let mut missing = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint exception"))?
    {
        if missing.len() >= MAX_SEQUENCE_EXCEPTIONS {
            return Err(invalid().into());
        }
        missing.push(row.get(0).map_err(sqlite_error("read missing frame"))?);
    }
    Ok(ContentStructure::image_sequence(
        ImageSequenceDescriptor::new(
            resource,
            FrameRange::new(fields.1, fields.2, fields.3)?,
            RationalRate::new(fields.4, fields.5)?,
            missing,
        )?,
    ))
}
