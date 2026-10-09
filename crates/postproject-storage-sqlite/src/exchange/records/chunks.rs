//! Internal fragments are independent of public protocol chunk bounds.

use postproject_core::{Result, RevisionId};
use postproject_protocol::{
    Document, FailureKind, Limits, MAX_RECORD_CHUNK_BYTES, ProtocolError, RecordChunk,
};
use rusqlite::{Connection, params};

use crate::{ExchangeResult, sqlite_error};

const FRAGMENT_BYTES: usize = 1024 * 1024;

pub(super) fn persist(connection: &Connection, chunk: &RecordChunk) -> Result<()> {
    let bytes = chunk
        .document()
        .and_then(|document| document.canonical_bytes())
        .map_err(|_| super::encoding())?;
    let mut insert = connection.prepare("INSERT INTO exchange_record_chunks (revision_id, position, fragment_position, document) VALUES (?1, ?2, ?3, ?4)")
        .map_err(sqlite_error("prepare committed chunk fragments"))?;
    for (fragment, bytes) in bytes.chunks(FRAGMENT_BYTES).enumerate() {
        insert
            .execute(params![
                chunk.revision().as_bytes().as_slice(),
                i64::try_from(chunk.index()).map_err(|_| super::encoding())?,
                i64::try_from(fragment).map_err(|_| super::encoding())?,
                bytes
            ])
            .map_err(sqlite_error("persist committed record chunk fragment"))?;
    }
    Ok(())
}

pub(super) fn load(
    connection: &Connection,
    revision: RevisionId,
    index: u64,
) -> ExchangeResult<Option<RecordChunk>> {
    let mut statement = connection.prepare("SELECT fragment_position, document FROM exchange_record_chunks WHERE revision_id = ?1 AND position = ?2 ORDER BY fragment_position")
        .map_err(sqlite_error("prepare committed chunk fragments"))?;
    let mut rows = statement
        .query(params![
            revision.as_bytes().as_slice(),
            i64::try_from(index).map_err(|_| super::invalid())?
        ])
        .map_err(sqlite_error("query committed chunk fragments"))?;
    let mut expected = 0_i64;
    let mut bytes = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read committed chunk fragment"))?
    {
        let position: i64 = row
            .get(0)
            .map_err(sqlite_error("read chunk fragment position"))?;
        let fragment: Vec<u8> = row
            .get(1)
            .map_err(sqlite_error("read chunk fragment bytes"))?;
        if position != expected || fragment.is_empty() || fragment.len() > FRAGMENT_BYTES {
            return Err(ProtocolError::new(
                FailureKind::Integrity,
                "invalid ordered chunk fragments",
            )
            .into());
        }
        if bytes
            .len()
            .checked_add(fragment.len())
            .is_none_or(|total| total > MAX_RECORD_CHUNK_BYTES)
        {
            return Err(ProtocolError::new(
                FailureKind::LimitExceeded,
                "stored chunk exceeds protocol limit",
            )
            .into());
        }
        bytes.extend_from_slice(&fragment);
        expected = expected.checked_add(1).ok_or_else(super::invalid)?;
    }
    if expected == 0 {
        return Ok(None);
    }
    Ok(Some(RecordChunk::from_document(&Document::parse(
        &bytes,
        Limits::new(MAX_RECORD_CHUNK_BYTES, 192, 8192)?,
    )?)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteProduction;
    use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};
    use postproject_protocol::{Extensions, MAX_RECORD_CHUNK_PAYLOAD};

    #[test]
    fn maximum_public_chunk_round_trips_through_the_native_sqlite_value_bound() {
        let directory = tempfile::tempdir().unwrap();
        let mut production =
            SqliteProduction::create(directory.path().join("chunks.pproj"), None).unwrap();
        let target = ObjectRef::Production(production.production().id());
        let property = MetadataProperty::new(
            VocabularyId::new("urn:chunks:test").unwrap(),
            PropertyId::new("value").unwrap(),
        );
        let mut edit = production.begin_transaction().unwrap();
        edit.add_metadata_value(target, &property, &MetadataValue::i64(1))
            .unwrap();
        let revision = edit.commit().unwrap().revision().unwrap().id();
        drop(edit);
        production
            .connection
            .execute("DELETE FROM exchange_record_chunks", [])
            .unwrap();
        let chunk = RecordChunk::new(
            production.exchange_scope().unwrap(),
            revision,
            0,
            None,
            vec![255; MAX_RECORD_CHUNK_PAYLOAD],
            Extensions::default(),
        )
        .unwrap();
        // Transport storage does not imply these arbitrary bytes are domain
        // effects; the original record manifest remains unchanged.
        persist(&production.connection, &chunk).unwrap();
        assert_eq!(
            load(&production.connection, revision, 0).unwrap().unwrap(),
            chunk
        );
        let size: i64 = production
            .connection
            .query_row(
                "SELECT max(length(document)) FROM exchange_record_chunks",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(size, i64::try_from(FRAGMENT_BYTES).unwrap());
        production
            .connection
            .execute(
                "DELETE FROM exchange_record_chunks WHERE fragment_position = 1",
                [],
            )
            .unwrap();
        assert!(load(&production.connection, revision, 0).is_err());
    }
}
