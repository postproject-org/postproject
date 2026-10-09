use postproject_core::{Error, ErrorKind, RepresentationId, Result};
use rusqlite::{OptionalExtension, params};

use super::{CaptureContext, PathEdge, snapshot_error};
use crate::{decode_dependency, id_bytes};

pub(super) fn path(
    context: &CaptureContext<'_, '_>,
    status: i64,
    subject: RepresentationId,
    path: &[PathEdge],
) -> Result<()> {
    let actual = context.transaction.query_row("SELECT id, status, subject_representation_id, (SELECT COUNT(*) FROM activity_input_dependency_path_edges WHERE path_id = p.id) FROM activity_input_dependency_paths p WHERE activity_input_id = ?1 AND position = ?2", params![context.activity_input_id, context.next_path_position], |row|Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Vec<u8>>(2)?, row.get::<_, i64>(3)?))).optional().map_err(snapshot_error("validate original dependency path"))?;
    let Some((id, actual_status, actual_subject, actual_depth)) = actual else {
        return Err(invalid());
    };
    if actual_status != status
        || actual_subject != subject.as_bytes()
        || usize::try_from(actual_depth).ok() != Some(path.len())
    {
        return Err(invalid());
    }
    for (position, expected) in path.iter().enumerate() {
        let actual = context.transaction.query_row("SELECT source_representation_id, dependency_position, source_resource_id, kind, target_kind, target_id, resolved_representation_id, authored_reference FROM activity_input_dependency_path_edges WHERE path_id = ?1 AND position = ?2", params![id, i64::try_from(position).map_err(|_|invalid())?], |row|Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?))).optional().map_err(snapshot_error("validate complete dependency segment"))?;
        let Some((
            source,
            authored_position,
            resource,
            kind,
            target_kind,
            target,
            resolved,
            authored,
        )) = actual
        else {
            return Err(invalid());
        };
        let actual_dependency =
            decode_dependency(resource, kind, target_kind, target, resolved, 1, authored)?;
        if id_bytes(source, "original path source")?
            != expected.source_representation_id.into_bytes()
            || usize::try_from(authored_position).ok() != Some(expected.dependency_position)
            || actual_dependency != expected.dependency
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn invalid() -> Error {
    Error::new(
        ErrorKind::InvalidArgument,
        "provided dependency paths contradict the authored prefix",
    )
}
