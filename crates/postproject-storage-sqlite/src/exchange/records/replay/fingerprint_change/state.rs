use postproject_core::{FingerprintSnapshot, ObjectRef};
use postproject_protocol::{FingerprintChangeStart, RecordManifest};
use rusqlite::{OptionalExtension, Transaction, params};

use super::super::{effects::invalid, facts::structural};
use crate::{
    ExchangeResult, encode_metadata_target, sqlite_error, stored_u64,
    transaction::{
        ensure_metadata_target_exists, fingerprint_capture::dirty_marker, mutation_error,
    },
};

pub(super) fn apply(
    transaction: &Transaction<'_>,
    manifest: &RecordManifest,
    start: &FingerprintChangeStart,
) -> ExchangeResult<()> {
    let target = start.target();
    let (kind, id) = encode_metadata_target(&target)?;
    structural(ensure_metadata_target_exists(transaction, kind, id))?;
    let (table, column) = match start.target() {
        ObjectRef::Resource(_) => ("resource_fingerprints", "resource_id"),
        ObjectRef::Representation(_) => ("representation_fingerprints", "representation_id"),
        _ => return Err(invalid().into()),
    };
    let current = start.current();
    let previous = transaction.query_row(&format!("SELECT value, observed_revision_sequence FROM {table} WHERE {column} = ?1 AND algorithm = ?2 AND algorithm_version = ?3"),
        params![id.as_slice(), current.algorithm(), current.version()], |row|Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<i64>>(1)?)))
        .optional().map_err(sqlite_error("read prior replay fingerprint"))?
        .map(|(bytes, sequence)| FingerprintSnapshot::new(current.algorithm(), current.version(), bytes,
            sequence.map(|sequence|stored_u64(sequence, "original fingerprint boundary")).transpose()?))
        .transpose()?;
    if previous.as_ref() != start.previous() {
        return Err(invalid().into());
    }
    let sequence = manifest.revision().sequence();
    let changed = previous
        .as_ref()
        .is_none_or(|previous| previous.value() != current.value());
    if (changed && current.observed_revision_sequence() != Some(sequence))
        || previous
            .as_ref()
            .and_then(FingerprintSnapshot::observed_revision_sequence)
            .is_some_and(|observed| observed > sequence)
    {
        return Err(invalid().into());
    }
    archive(transaction, start, table, column, id, sequence)?;
    if changed {
        structural(transaction.execute(&format!("INSERT INTO {table} ({column}, algorithm, algorithm_version, value, observed_revision_sequence) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT({column}, algorithm, algorithm_version) DO UPDATE SET value = excluded.value, observed_revision_sequence = excluded.observed_revision_sequence"),
            params![id.as_slice(), current.algorithm(), current.version(), current.value(), i64::try_from(sequence).map_err(|_|invalid())?])
            .map_err(mutation_error("stage original current fingerprint")))?;
    }
    if let ObjectRef::Representation(representation) = start.target() {
        let marker = dirty_marker(transaction, representation)?;
        if marker != start.cleared_marker()
            || marker.is_some_and(|marker| marker.revision_sequence() > sequence)
        {
            return Err(invalid().into());
        }
        let invalidates: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM dependency_sets WHERE source_representation_id = ?1 AND needs_extraction = 0)",
            [representation.as_bytes().as_slice()], |row|row.get(0)).map_err(sqlite_error("validate original dependency invalidation"))?;
        if invalidates != start.dependency_invalidated() {
            return Err(invalid().into());
        }
        structural(transaction.execute("DELETE FROM representation_fingerprint_recomputations WHERE representation_id = ?1", [representation.as_bytes().as_slice()])
            .map_err(mutation_error("stage original recomputation clear")))?;
        structural(transaction.execute("UPDATE dependency_sets SET needs_extraction = 1 WHERE source_representation_id = ?1 AND needs_extraction = 0", [representation.as_bytes().as_slice()])
            .map_err(mutation_error("stage original dependency invalidation")))?;
    }
    Ok(())
}

fn archive(
    transaction: &Transaction<'_>,
    start: &FingerprintChangeStart,
    table: &str,
    column: &str,
    id: &[u8; 16],
    sequence: u64,
) -> ExchangeResult<()> {
    let Some(position) = start.archived_position() else {
        return Ok(());
    };
    let previous = start.previous().ok_or_else(invalid)?;
    let table = match table {
        "resource_fingerprints" => "resource_fingerprint_history",
        "representation_fingerprints" => "representation_fingerprint_history",
        _ => return Err(invalid().into()),
    };
    let count: i64 = transaction.query_row(&format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1 AND algorithm = ?2 AND algorithm_version = ?3"),
        params![id.as_slice(), previous.algorithm(), previous.version()], |row|row.get(0)).map_err(sqlite_error("validate original archive order"))?;
    if u64::try_from(count).ok() != Some(position) {
        return Err(invalid().into());
    }
    structural(transaction.execute(&format!("INSERT INTO {table} ({column}, algorithm, algorithm_version, value, observed_revision_sequence, superseded_revision_sequence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"),
        params![id.as_slice(), previous.algorithm(), previous.version(), previous.value(), previous.observed_revision_sequence().map(i64::try_from).transpose().map_err(|_|invalid())?, i64::try_from(sequence).map_err(|_|invalid())?])
        .map_err(mutation_error("stage original superseded fingerprint")))?;
    Ok(())
}
