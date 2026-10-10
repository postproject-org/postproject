//! Coherent bounded checkpoint transport; domain coverage grows in checked slices.

mod activity_facts;
mod dependency_facts;
mod import;
mod media_facts;
mod sections;
mod writer;

pub use import::CheckpointLimits;
pub(crate) use import::import;
pub(crate) use import::recover;

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
    let unsupported: bool = view
        .connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM jobs UNION ALL SELECT 1 FROM dependency_sets)",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error("check checkpoint domain coverage"))?;
    if unsupported {
        return Err(ProtocolError::new(
            FailureKind::Unsupported,
            "checkpoint dependency and job evidence is not supported yet",
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
            CheckpointSection::Assets
            | CheckpointSection::Resources
            | CheckpointSection::Representations
            | CheckpointSection::Structures => sections::media(&view, &mut writer, section)?,
            CheckpointSection::Locators => sections::locators(&view, &mut writer)?,
            CheckpointSection::Identifiers => sections::identifiers(&view, &mut writer)?,
            CheckpointSection::Fingerprints => sections::fingerprints(&view, &mut writer)?,
            CheckpointSection::Metadata => sections::metadata(&view, &mut writer)?,
            CheckpointSection::Activities => sections::activities(&view, &mut writer)?,
            CheckpointSection::Dependencies => {
                sections::dependencies(&view, &mut writer, head.sequence())?;
            }
            CheckpointSection::Roots => sections::roots(&view, &mut writer)?,
            CheckpointSection::Revisions => sections::revisions(&view, &mut writer)?,
            CheckpointSection::Events => sections::events(&view, &mut writer)?,
            CheckpointSection::ConflictVersions => sections::versions(&view, &mut writer)?,
            CheckpointSection::ConflictFloor => sections::conflict_floor(&view, &mut writer)?,
            CheckpointSection::Records => sections::records(&view, floor, head, &mut writer)?,
            CheckpointSection::Jobs => (),
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
