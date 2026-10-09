use postproject_core::{Error, RepresentationId};
use postproject_protocol::{
    ActivityPathHeader, ActivityPathSegment, ActivityPathStatus, DependencyOccurrence, Document,
};
use rusqlite::Connection;

use crate::{decode_dependency, id_bytes, sqlite_error, stored_u64};

pub(super) fn write<E: From<Error>>(
    connection: &Connection,
    input: i64,
    write: &mut impl FnMut(&Document) -> Result<(), E>,
) -> Result<(), E> {
    let mut statement = connection.prepare("SELECT p.id, p.position, p.status, p.subject_representation_id, (SELECT COUNT(*) FROM activity_input_dependency_path_edges WHERE path_id = p.id), (SELECT COUNT(*) FROM activity_input_dependency_fingerprint_snapshots WHERE path_id = p.id) FROM activity_input_dependency_paths p WHERE activity_input_id = ?1 ORDER BY position")
        .map_err(sqlite_error("prepare authored dependency paths"))?;
    let rows = statement
        .query_map([input], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(sqlite_error("query authored dependency paths"))?;
    for (position, row) in rows.enumerate() {
        let (id, authored_position, status, subject, segments, fingerprints) =
            row.map_err(sqlite_error("read authored dependency path"))?;
        let status = match status {
            0 => ActivityPathStatus::Recorded,
            1 => ActivityPathStatus::NeedsExtraction,
            2 => ActivityPathStatus::Unresolved,
            3 => ActivityPathStatus::DepthTruncated,
            4 => ActivityPathStatus::RepresentationsTruncated,
            _ => return Err(super::invalid().into()),
        };
        let header = ActivityPathHeader::new(
            stored_u64(authored_position, "authored dependency path position")?,
            status,
            RepresentationId::from_bytes(id_bytes(subject, "authored dependency subject")?),
            stored_u64(segments, "authored path depth")?,
            stored_u64(fingerprints, "authored dependency fingerprint count")?,
        )
        .map_err(|_| super::invalid())?;
        if u64::try_from(position).map_err(|_| super::invalid())? != header.position() {
            return Err(super::invalid().into());
        }
        write(&header.document())?;
        write_segments(connection, id, write)?;
        super::fingerprints::write(
            connection,
            "activity_input_dependency_fingerprint_snapshots",
            "path_id",
            id,
            write,
        )?;
    }
    Ok(())
}

fn write_segments<E: From<Error>>(
    connection: &Connection,
    path: i64,
    write: &mut impl FnMut(&Document) -> Result<(), E>,
) -> Result<(), E> {
    let mut statement = connection.prepare("SELECT position, source_representation_id, dependency_position, source_resource_id, kind, target_kind, target_id, resolved_representation_id, authored_reference FROM activity_input_dependency_path_edges WHERE path_id = ?1 ORDER BY position")
        .map_err(sqlite_error("prepare authored dependency segments"))?;
    let mut rows = statement
        .query([path])
        .map_err(sqlite_error("query authored dependency segments"))?;
    let mut position = 0;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read authored dependency segment"))?
    {
        let authored_position: i64 = row
            .get(0)
            .map_err(sqlite_error("read authored path position"))?;
        let source: Vec<u8> = row
            .get(1)
            .map_err(sqlite_error("read authored segment source"))?;
        let dependency_position: i64 = row
            .get(2)
            .map_err(sqlite_error("read authored dependency position"))?;
        let stored = (|| -> rusqlite::Result<_> {
            Ok((
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })()
        .map_err(sqlite_error("read authored dependency facts"))?;
        let dependency = decode_dependency(
            stored.0, stored.1, stored.2, stored.3, stored.4, 1, stored.5,
        )?;
        if stored_u64(authored_position, "authored path position")? != position {
            return Err(super::invalid().into());
        }
        let occurrence = DependencyOccurrence::new(
            RepresentationId::from_bytes(id_bytes(source, "authored segment source")?),
            stored_u64(dependency_position, "authored dependency position")?,
            dependency,
        )
        .map_err(|_| super::invalid())?;
        write(
            &ActivityPathSegment::new(position, occurrence)
                .and_then(|segment| segment.document())
                .map_err(|_| super::invalid())?,
        )?;
        position += 1;
    }
    Ok(())
}
