use postproject_core::{DependencyTarget, ObjectRef, RepresentationId};
use postproject_protocol::{DependencyOccurrence, Document, Limits};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{ExchangeResult, sqlite_error, transaction::validate_dependency_references};

use super::{invalid, state};

pub(in crate::exchange::checkpoint::import) fn require_references(
    connection: &Connection,
    occurrence: &DependencyOccurrence,
    floor: u64,
) -> ExchangeResult<()> {
    let dependency = occurrence.dependency();
    validate_dependency_references(
        connection,
        occurrence.source_representation_id(),
        dependency,
    )
    .map_err(|_| invalid())?;
    let require = |target| super::super::media_state::require(connection, target, floor);
    require(ObjectRef::Representation(
        occurrence.source_representation_id(),
    ))?;
    if let Some(resource) = dependency.source_resource_id() {
        require(ObjectRef::Resource(resource))?;
    }
    match dependency.target() {
        DependencyTarget::Asset(asset) => require(ObjectRef::Asset(asset))?,
        DependencyTarget::Representation(representation) => {
            require(ObjectRef::Representation(representation))?;
        }
        _ => return Err(invalid().into()),
    }
    if let Some(resolved) = dependency.resolved_representation_id() {
        require(ObjectRef::Representation(resolved))?;
    }
    Ok(())
}

pub(super) fn prior(
    connection: &Connection,
    owner: RepresentationId,
    position: u64,
) -> ExchangeResult<Option<DependencyOccurrence>> {
    let bytes: Option<Vec<u8>> = connection.query_row("SELECT document FROM checkpoint_dependency_occurrences WHERE owner = ?1 AND position = ?2", params![owner.as_bytes().as_slice(), i64::try_from(position).map_err(|_| invalid())?], |row| row.get(0))
        .optional().map_err(sqlite_error("read authored dependency occurrence"))?;
    if let Some(bytes) = bytes {
        return Ok(Some(DependencyOccurrence::from_document(
            &Document::parse(&bytes, Limits::default())?,
        )?));
    }
    if state(connection, owner)?.is_some_and(|state| state.baseline) {
        current(connection, owner, position)
    } else {
        Ok(None)
    }
}

pub(super) fn current(
    connection: &Connection,
    owner: RepresentationId,
    position: u64,
) -> ExchangeResult<Option<DependencyOccurrence>> {
    let value = connection.query_row("SELECT source_resource_id, kind, target_kind, target_id, resolved_representation_id, required, authored_reference FROM dependencies WHERE source_representation_id = ?1 AND position = ?2", params![owner.as_bytes().as_slice(), i64::try_from(position).map_err(|_| invalid())?], crate::read_budget::bounded(|row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))))
        .optional().map_err(sqlite_error("read current dependency occurrence"))?;
    value
        .map(
            |(resource, kind, target_kind, target, resolved, required, authored)| {
                let dependency = crate::decode_dependency(
                    resource,
                    kind,
                    target_kind,
                    target,
                    resolved,
                    required,
                    authored,
                )?;
                Ok(DependencyOccurrence::new(owner, position, dependency)?)
            },
        )
        .transpose()
}

pub(super) fn finish(connection: &Connection) -> ExchangeResult<()> {
    let mut statement = connection.prepare("SELECT owner, position, document FROM checkpoint_dependency_occurrences ORDER BY owner, position")
        .map_err(sqlite_error("prepare explained dependency occurrences"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query explained dependency occurrences"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read explained dependency occurrence"))?
    {
        let owner = RepresentationId::from_bytes(crate::id_bytes(
            row.get(0).map_err(sqlite_error("read dependency owner"))?,
            "dependency owner",
        )?);
        let position = crate::stored_u64(
            row.get(1)
                .map_err(sqlite_error("read dependency position"))?,
            "dependency position",
        )?;
        let bytes: Vec<u8> = row
            .get(2)
            .map_err(sqlite_error("read dependency document"))?;
        let expected =
            DependencyOccurrence::from_document(&Document::parse(&bytes, Limits::default())?)?;
        if current(connection, owner, position)?.as_ref() != Some(&expected) {
            return Err(invalid().into());
        }
    }
    Ok(())
}
