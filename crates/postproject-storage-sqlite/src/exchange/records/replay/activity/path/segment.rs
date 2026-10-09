use postproject_core::DependencyTarget;
use postproject_protocol::ActivityPathSegment;
use rusqlite::{OptionalExtension, Transaction, params};

use super::super::super::effects::invalid;
use crate::{ExchangeResult, decode_dependency, sqlite_error};

pub(super) fn persist(
    transaction: &Transaction<'_>,
    path: i64,
    segment: &ActivityPathSegment,
) -> ExchangeResult<()> {
    let occurrence = segment.occurrence();
    let dependency = occurrence.dependency();
    let source = occurrence.source_representation_id();
    let position = i64::try_from(occurrence.position()).map_err(|_| invalid())?;
    let prior = transaction.query_row("SELECT source_resource_id, kind, target_kind, target_id, resolved_representation_id, required, authored_reference FROM dependencies WHERE source_representation_id = ?1 AND position = ?2 AND EXISTS(SELECT 1 FROM dependency_sets WHERE source_representation_id = ?1 AND needs_extraction = 0)", params![source.as_bytes().as_slice(), position], |row|Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))).optional().map_err(sqlite_error("validate original authored dependency segment"))?;
    let prior = prior
        .map(
            |(resource, kind, target_kind, target, resolved, required, authored)| {
                decode_dependency(
                    resource,
                    kind,
                    target_kind,
                    target,
                    resolved,
                    required,
                    authored,
                )
            },
        )
        .transpose()?;
    if prior.as_ref() != Some(dependency) {
        return Err(invalid().into());
    }
    let (target_kind, target) = match dependency.target() {
        DependencyTarget::Asset(id) => (1, id.into_bytes()),
        DependencyTarget::Representation(id) => (2, id.into_bytes()),
        _ => return Err(invalid().into()),
    };
    transaction.execute("INSERT INTO activity_input_dependency_path_edges (path_id, position, source_representation_id, dependency_position, source_resource_id, kind, target_kind, target_id, resolved_representation_id, authored_reference) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", params![path, i64::try_from(segment.position()).map_err(|_|invalid())?, source.as_bytes().as_slice(), position, dependency.source_resource_id().map(|id|id.into_bytes().to_vec()), dependency.kind().as_str(), target_kind, target.as_slice(), dependency.resolved_representation_id().map(|id|id.into_bytes().to_vec()), dependency.authored_reference()]).map_err(sqlite_error("stage original authored dependency segment"))?;
    Ok(())
}
