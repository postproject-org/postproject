//! Stream immutable staging-time evidence, never current input observations.

mod fingerprints;
mod paths;

use postproject_core::{
    ActivityRole, Error, ErrorKind, RepresentationId, Result, RevisionEventKind,
};
use postproject_protocol::{ActivityEdgeHeader, ActivityEdgeSide, ActivityHeader, Document};
use rusqlite::{Connection, params};

use crate::{id_bytes, sqlite_error, stored_domain_error, stored_u64};

pub(super) fn write<E: From<Error>>(
    connection: &Connection,
    header: &ActivityHeader,
    write: &mut impl FnMut(&Document) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    write(&header.document())?;
    edges(connection, header, |id, edge| {
        write(&edge.document())?;
        let (table, column) = match edge.side() {
            ActivityEdgeSide::Input => {
                ("activity_input_fingerprint_snapshots", "activity_input_id")
            }
            ActivityEdgeSide::Output => (
                "activity_output_fingerprint_snapshots",
                "activity_output_id",
            ),
        };
        fingerprints::write(connection, table, column, id, write)?;
        if edge.has_dependency_snapshot() {
            paths::write(connection, id, write)?;
        }
        Ok(())
    })
}

pub(super) fn observations(
    connection: &Connection,
    header: &ActivityHeader,
    visit: &mut impl FnMut(RevisionEventKind) -> Result<()>,
) -> Result<()> {
    visit(RevisionEventKind::ActivityCreated {
        activity_id: header.id(),
        kind: header.kind().clone(),
    })?;
    edges(connection, header, |_, edge| {
        let event = match edge.side() {
            ActivityEdgeSide::Input => RevisionEventKind::ActivityInputAdded {
                activity_id: header.id(),
                representation_id: edge.representation_id(),
                role: edge.role().cloned(),
            },
            ActivityEdgeSide::Output => RevisionEventKind::ActivityOutputAdded {
                activity_id: header.id(),
                representation_id: edge.representation_id(),
                role: edge.role().cloned(),
            },
        };
        visit(event)
    })
}

fn edges<E: From<Error>>(
    connection: &Connection,
    header: &ActivityHeader,
    mut visit: impl FnMut(i64, ActivityEdgeHeader) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    for (side, table, fingerprints, column, count) in [
        (
            ActivityEdgeSide::Input,
            "activity_inputs",
            "activity_input_fingerprint_snapshots",
            "activity_input_id",
            header.input_count(),
        ),
        (
            ActivityEdgeSide::Output,
            "activity_outputs",
            "activity_output_fingerprint_snapshots",
            "activity_output_id",
            header.output_count(),
        ),
    ] {
        let paths = if side == ActivityEdgeSide::Input {
            "EXISTS(SELECT 1 FROM activity_input_dependency_snapshots WHERE activity_input_id = e.id), (SELECT COUNT(*) FROM activity_input_dependency_paths WHERE activity_input_id = e.id)"
        } else {
            "0, 0"
        };
        let sql = format!(
            "SELECT e.id, e.representation_id, e.role, e.snapshot_revision_sequence, (SELECT COUNT(*) FROM {fingerprints} WHERE {column} = e.id), {paths} FROM {table} e WHERE activity_id = ?1 ORDER BY representation_id, role, id"
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(sqlite_error("prepare authored activity edges"))?;
        let mut rows = statement
            .query(params![header.id().as_bytes().as_slice()])
            .map_err(sqlite_error("query authored activity edges"))?;
        let mut position = 0;
        while let Some(row) = rows
            .next()
            .map_err(sqlite_error("read authored activity edge"))?
        {
            let id: i64 = row
                .get(0)
                .map_err(sqlite_error("read authored edge identity"))?;
            let representation: Vec<u8> = row
                .get(1)
                .map_err(sqlite_error("read authored edge representation"))?;
            let role: Option<String> = row
                .get(2)
                .map_err(sqlite_error("read authored edge role"))?;
            let sequence: Option<i64> = row
                .get(3)
                .map_err(sqlite_error("read authored edge boundary"))?;
            let fingerprint_count: i64 = row
                .get(4)
                .map_err(sqlite_error("read authored edge fingerprint count"))?;
            let marker: bool = row
                .get(5)
                .map_err(sqlite_error("read authored edge dependency marker"))?;
            let path_count: i64 = row
                .get(6)
                .map_err(sqlite_error("read authored edge path count"))?;
            let mut edge = ActivityEdgeHeader::new(
                header.id(),
                side,
                position,
                RepresentationId::from_bytes(id_bytes(
                    representation,
                    "authored edge representation",
                )?),
                role.map(ActivityRole::new)
                    .transpose()
                    .map_err(stored_domain_error("authored edge role"))?,
            )
            .map_err(|_| invalid())?;
            if let Some(sequence) = sequence {
                edge = edge
                    .with_snapshot(
                        stored_u64(sequence, "authored edge boundary")?,
                        stored_u64(fingerprint_count, "authored edge fingerprint count")?,
                    )
                    .map_err(|_| invalid())?;
            } else if fingerprint_count != 0 {
                return Err(invalid().into());
            }
            if marker {
                edge = edge
                    .with_dependency_snapshot(stored_u64(path_count, "authored edge path count")?)
                    .map_err(|_| invalid())?;
            } else if path_count != 0 {
                return Err(invalid().into());
            }
            visit(id, edge)?;
            position += 1;
        }
        if position != count {
            return Err(invalid().into());
        }
    }
    Ok(())
}

fn invalid() -> Error {
    Error::new(ErrorKind::Storage, "invalid immutable activity capture")
}
