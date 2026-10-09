use postproject_core::{FingerprintSnapshot, ObjectRef, RepresentationId, ResourceId};
use postproject_protocol::{
    CheckpointChunk, FingerprintObservation, FingerprintRecomputation, FingerprintState,
};

use crate::{ExchangeResult, SqliteProduction, id_bytes, sqlite_error, stored_u64};

use super::super::{invalid, writer::SectionWriter};

pub(in crate::exchange::checkpoint) fn fingerprints<
    Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>,
>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
) -> ExchangeResult<()> {
    for (table, owner, representation) in [
        ("representation_fingerprints", "representation_id", true),
        ("resource_fingerprints", "resource_id", false),
    ] {
        observations(view, writer, table, owner, representation, false)?;
        let history = if representation {
            "representation_fingerprint_history"
        } else {
            "resource_fingerprint_history"
        };
        observations(view, writer, history, owner, representation, true)?;
    }
    let mut statement = view.connection.prepare("SELECT representation_id, changed_resource_id, marked_revision_sequence FROM representation_fingerprint_recomputations ORDER BY representation_id")
        .map_err(sqlite_error("prepare checkpoint recomputation evidence"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint recomputation evidence"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint recomputation evidence"))?
    {
        let marker = FingerprintRecomputation::new(
            RepresentationId::from_bytes(id_bytes(
                row.get(0)
                    .map_err(sqlite_error("read recomputation owner"))?,
                "recomputation owner",
            )?),
            ResourceId::from_bytes(id_bytes(
                row.get(1)
                    .map_err(sqlite_error("read recomputation resource"))?,
                "recomputation resource",
            )?),
            stored_u64(
                row.get(2)
                    .map_err(sqlite_error("read recomputation boundary"))?,
                "recomputation boundary",
            )?,
        )?;
        writer.document(&marker.document(), true)?;
    }
    Ok(())
}

fn observations<Sink: FnMut(CheckpointChunk) -> ExchangeResult<()>>(
    view: &SqliteProduction,
    writer: &mut SectionWriter<'_, Sink>,
    table: &str,
    owner: &str,
    representation: bool,
    history: bool,
) -> ExchangeResult<()> {
    let state = if history {
        format!(
            "superseded_revision_sequence, row_number() OVER (PARTITION BY {owner}, algorithm, algorithm_version ORDER BY id) - 1"
        )
    } else {
        "NULL, NULL".into()
    };
    let order = if history { ", id" } else { "" };
    let mut statement = view.connection.prepare(&format!("SELECT {owner}, algorithm, algorithm_version, value, observed_revision_sequence, {state} FROM {table} ORDER BY {owner}, algorithm, algorithm_version{order}"))
        .map_err(sqlite_error("prepare checkpoint fingerprint evidence"))?;
    let mut rows = statement
        .query([])
        .map_err(sqlite_error("query checkpoint fingerprint evidence"))?;
    while let Some(row) = rows
        .next()
        .map_err(sqlite_error("read checkpoint fingerprint evidence"))?
    {
        let id = id_bytes(
            row.get(0).map_err(sqlite_error("read fingerprint owner"))?,
            "fingerprint owner",
        )?;
        let target = if representation {
            ObjectRef::Representation(RepresentationId::from_bytes(id))
        } else {
            ObjectRef::Resource(ResourceId::from_bytes(id))
        };
        let algorithm: String = row
            .get(1)
            .map_err(sqlite_error("read fingerprint algorithm"))?;
        let version: i64 = row
            .get(2)
            .map_err(sqlite_error("read fingerprint version"))?;
        let observed: Option<i64> = row
            .get(4)
            .map_err(sqlite_error("read fingerprint observation boundary"))?;
        let snapshot = FingerprintSnapshot::new(
            algorithm,
            u16::try_from(version).map_err(|_| invalid())?,
            row.get(3).map_err(sqlite_error("read fingerprint bytes"))?,
            observed
                .map(|sequence| stored_u64(sequence, "fingerprint observation boundary"))
                .transpose()?,
        )?;
        let state = if history {
            FingerprintState::Superseded {
                position: stored_u64(
                    row.get(6)
                        .map_err(sqlite_error("read fingerprint history position"))?,
                    "fingerprint history position",
                )?,
                revision_sequence: stored_u64(
                    row.get(5)
                        .map_err(sqlite_error("read fingerprint supersession boundary"))?,
                    "fingerprint supersession boundary",
                )?,
            }
        } else {
            FingerprintState::Current
        };
        writer.document(
            &FingerprintObservation::new(target, snapshot, state)?.document()?,
            true,
        )?;
    }
    Ok(())
}
