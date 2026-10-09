use super::mutation_error;
use postproject_core::{Dependency, DependencyTarget, Error, ErrorKind, RepresentationId, Result};
use rusqlite::{Transaction, params};

pub(crate) fn occurrence(
    transaction: &Transaction<'_>,
    representation_id: RepresentationId,
    position: i64,
    dependency: &Dependency,
) -> Result<()> {
    let (target_kind, target_id) = match dependency.target() {
        DependencyTarget::Asset(id) => (1_i64, id.into_bytes()),
        DependencyTarget::Representation(id) => (2_i64, id.into_bytes()),
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "dependency target kind is not supported by this schema",
            ));
        }
    };
    transaction
        .execute(
            "INSERT INTO dependencies (
                    source_representation_id, position, source_resource_id, kind,
                    target_kind, target_id, resolved_representation_id, required,
                    authored_reference
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                representation_id.as_bytes().as_slice(),
                position,
                dependency
                    .source_resource_id()
                    .map(|id| id.into_bytes().to_vec()),
                dependency.kind().as_str(),
                target_kind,
                target_id.as_slice(),
                dependency
                    .resolved_representation_id()
                    .map(|id| id.into_bytes().to_vec()),
                dependency.is_required(),
                dependency.authored_reference(),
            ],
        )
        .map_err(mutation_error("persist dependency edge"))?;
    Ok(())
}
