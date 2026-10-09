//! Coherent bounded checkpoint transport; metadata domain slice only for now.

mod import;
mod sections;
mod writer;

pub use import::CheckpointLimits;
pub(crate) use import::import;

use postproject_protocol::{
    CheckpointChunk, CheckpointId, CheckpointManifest, CheckpointSection, Extensions, FailureKind,
    ProductionHeader, ProtocolError, SectionSummary,
};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

pub(crate) fn export(
    source: &SqliteProduction,
    mut sink: impl FnMut(CheckpointChunk) -> ExchangeResult<()>,
) -> ExchangeResult<CheckpointManifest> {
    let session = source.read_session()?;
    let sequence = session.decision_base().sequence();
    let view = session.into_read_only();
    let floor = super::floor(&view.connection, view.production.id())?;
    let head =
        super::position(&view.connection, view.production.id(), sequence)?.ok_or_else(|| {
            ProtocolError::new(
                FailureKind::HistoryGap,
                "checkpoint head lacks complete capture",
            )
        })?;
    // Do not silently omit unsupported current objects in this vertical slice.
    let unsupported: bool = view.connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM assets UNION ALL SELECT 1 FROM resources UNION ALL SELECT 1 FROM representations UNION ALL SELECT 1 FROM media_roots UNION ALL SELECT 1 FROM activities UNION ALL SELECT 1 FROM jobs UNION ALL SELECT 1 FROM external_identifiers)",
        [], |row| row.get(0),
    ).map_err(sqlite_error("check checkpoint domain coverage"))?;
    if unsupported {
        return Err(ProtocolError::new(
            FailureKind::Unsupported,
            "checkpoint currently supports production metadata only",
        )
        .into());
    }
    let partial_history: bool = view.connection.query_row("SELECT EXISTS(SELECT 1 FROM exchange_effect_fragments e JOIN revisions r ON r.id = e.revision_id WHERE r.sequence <= ?1)", [i64::try_from(floor.sequence()).map_err(|_| invalid())?], |row| row.get(0))
        .map_err(sqlite_error("check earlier development effect evidence"))?;
    if partial_history {
        return Err(ProtocolError::new(
            FailureKind::Unsupported,
            "checkpoint cannot yet transport pre-floor development effect evidence",
        )
        .into());
    }
    let id = CheckpointId::new();
    let mut summaries = Vec::with_capacity(CheckpointSection::ALL.len());
    for section in CheckpointSection::ALL {
        let mut writer = writer::SectionWriter::new(head.scope(), id, section, &mut sink);
        match section {
            CheckpointSection::Production => writer.document(
                &ProductionHeader::from_production(&view.production).document(),
                true,
            )?,
            CheckpointSection::Metadata => sections::metadata(&view, &mut writer)?,
            CheckpointSection::Revisions => sections::revisions(&view, &mut writer)?,
            CheckpointSection::Events => sections::events(&view, &mut writer)?,
            CheckpointSection::ConflictVersions => sections::versions(&view, &mut writer)?,
            CheckpointSection::ConflictFloor => sections::conflict_floor(&view, &mut writer)?,
            CheckpointSection::Records => sections::records(&view, floor, head, &mut writer)?,
            _ => (),
        }
        summaries.push(writer.finish()?);
    }
    let summaries: [SectionSummary; 18] = summaries.try_into().map_err(|_| invalid())?;
    Ok(CheckpointManifest::new(
        id,
        head,
        floor,
        summaries,
        Extensions::default(),
    )?)
}

fn invalid() -> ProtocolError {
    ProtocolError::new(
        FailureKind::Integrity,
        "checkpoint source facts are inconsistent",
    )
}
