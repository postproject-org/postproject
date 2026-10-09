use postproject_core::{
    Error, ErrorKind, FingerprintSnapshot, ObjectRef, RepresentationId, ResourceId, Result,
};
use postproject_protocol::{FingerprintChangeStart, FingerprintRecomputation};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::{exchange::CapturedFingerprint, id_bytes, sqlite_error, stored_u64};

pub(super) fn prepare(
    transaction: &Transaction<'_>,
    target: ObjectRef,
    current: FingerprintSnapshot,
    previous: Option<&(Vec<u8>, Option<i64>)>,
) -> Result<CapturedFingerprint> {
    let previous = previous
        .map(|(bytes, sequence)| {
            FingerprintSnapshot::new(
                current.algorithm(),
                current.version(),
                bytes.clone(),
                sequence
                    .map(|sequence| stored_u64(sequence, "fingerprint observation"))
                    .transpose()?,
            )
        })
        .transpose()?;
    let changed = previous
        .as_ref()
        .is_none_or(|previous| previous.value() != current.value());
    let (table, column, id) = match &target {
        ObjectRef::Resource(id) => ("resource_fingerprint_history", "resource_id", id.as_bytes()),
        ObjectRef::Representation(id) => (
            "representation_fingerprint_history",
            "representation_id",
            id.as_bytes(),
        ),
        _ => return Err(invalid()),
    };
    let archived_position = if previous.is_some() && changed {
        Some(stored_u64(transaction.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1 AND algorithm = ?2 AND algorithm_version = ?3"),
            params![id.as_slice(), current.algorithm(), current.version()], |row|row.get::<_, i64>(0),
        ).map_err(sqlite_error("capture fingerprint archive position"))?, "fingerprint archive position")?)
    } else {
        None
    };
    let (markers, cleared_marker, dependency_invalidated) = match target {
        ObjectRef::Resource(id) => (owners(transaction, id, current.observed_revision_sequence().ok_or_else(invalid)?)?, None, false),
        ObjectRef::Representation(id) => (Vec::new(), dirty_marker(transaction, id)?,
            transaction.query_row("SELECT EXISTS(SELECT 1 FROM dependency_sets WHERE source_representation_id = ?1 AND needs_extraction = 0)", [id.as_bytes().as_slice()], |row|row.get(0))
                .map_err(sqlite_error("capture dependency invalidation"))?),
        _ => return Err(invalid()),
    };
    let current = if changed {
        current
    } else {
        previous.clone().ok_or_else(invalid)?
    };
    let start = FingerprintChangeStart::new(
        target,
        previous,
        current,
        archived_position,
        u64::try_from(markers.len()).map_err(|_| invalid())?,
        cleared_marker,
        dependency_invalidated,
    )
    .map_err(|_| invalid())?;
    Ok(CapturedFingerprint { start, markers })
}

fn owners(
    transaction: &Transaction<'_>,
    resource: ResourceId,
    sequence: u64,
) -> Result<Vec<FingerprintRecomputation>> {
    let mut statement = transaction.prepare("SELECT representation_id FROM representation_resources WHERE resource_id = ?1 ORDER BY representation_id")
        .map_err(sqlite_error("prepare fingerprint owner capture"))?;
    statement
        .query_map([resource.as_bytes().as_slice()], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .map_err(sqlite_error("query fingerprint owners"))?
        .map(|row| {
            let representation = RepresentationId::from_bytes(id_bytes(
                row.map_err(sqlite_error("read fingerprint owner"))?,
                "fingerprint owner",
            )?);
            FingerprintRecomputation::new(representation, resource, sequence).map_err(|_| invalid())
        })
        .collect()
}

pub(crate) fn dirty_marker(
    transaction: &Transaction<'_>,
    representation: RepresentationId,
) -> Result<Option<FingerprintRecomputation>> {
    transaction.query_row("SELECT changed_resource_id, marked_revision_sequence FROM representation_fingerprint_recomputations WHERE representation_id = ?1",
        [representation.as_bytes().as_slice()], |row|Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)))
        .optional().map_err(sqlite_error("capture cleared fingerprint marker"))?
        .map(|(resource, sequence)| {
            FingerprintRecomputation::new(representation, ResourceId::from_bytes(id_bytes(resource, "changed fingerprint resource")?), stored_u64(sequence, "fingerprint marker")?)
                .map_err(|_|invalid())
        }).transpose()
}

fn invalid() -> Error {
    Error::new(
        ErrorKind::Storage,
        "invalid original fingerprint transition",
    )
}
