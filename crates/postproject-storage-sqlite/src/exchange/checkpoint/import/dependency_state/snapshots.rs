use postproject_core::RepresentationId;
use postproject_protocol::{ActivityPathSegment, ActivityPathStatus};
use rusqlite::Connection;

use crate::ExchangeResult;

use super::{baseline, invalid, occurrence, store};

pub(in crate::exchange::checkpoint::import) fn segment(
    connection: &Connection,
    segment: &ActivityPathSegment,
    floor: u64,
) -> ExchangeResult<()> {
    let value = segment.occurrence();
    let owner = value.source_representation_id();
    occurrence::require_references(connection, value, floor)?;
    let mut state = baseline::ensure(connection, owner, floor)?;
    if state.present == Some(false) || state.dirty == Some(true) {
        return Err(invalid().into());
    }
    if let Some(count) = state.count {
        if value.position() >= crate::stored_u64(count, "authored dependency count")?
            || occurrence::prior(connection, owner, value.position())?.as_ref() != Some(value)
        {
            return Err(invalid().into());
        }
    }
    // A captured required segment proves a Current set existed, even when an
    // older replacement's optional rows are no longer retained.
    state.present = Some(true);
    state.dirty = Some(false);
    store(connection, owner, state)
}

pub(in crate::exchange::checkpoint::import) fn subject(
    connection: &Connection,
    owner: RepresentationId,
    status: ActivityPathStatus,
    floor: u64,
) -> ExchangeResult<()> {
    let mut state = baseline::ensure(connection, owner, floor)?;
    match status {
        ActivityPathStatus::NeedsExtraction => {
            if state.present == Some(false) || state.dirty == Some(false) {
                return Err(invalid().into());
            }
            state.present = Some(true);
            state.dirty = Some(true);
        }
        ActivityPathStatus::Recorded | ActivityPathStatus::DepthTruncated => {
            if state.dirty == Some(true) {
                if state.present == Some(true) {
                    return Err(invalid().into());
                }
                state.present = Some(false);
            } else if state.present == Some(true) {
                state.dirty = Some(false);
            }
            if status == ActivityPathStatus::DepthTruncated {
                if state.present == Some(false) {
                    return Err(invalid().into());
                }
                if let Some(count) = state.count {
                    super::charge(
                        connection,
                        crate::stored_u64(count, "authored dependency count")?,
                    )?;
                    let mut required = false;
                    for position in 0..crate::stored_u64(count, "authored dependency count")? {
                        required |= occurrence::prior(connection, owner, position)?
                            .ok_or_else(invalid)?
                            .dependency()
                            .is_required();
                    }
                    if !required {
                        return Err(invalid().into());
                    }
                }
            }
        }
        ActivityPathStatus::Unresolved | ActivityPathStatus::RepresentationsTruncated => {
            return Ok(());
        }
    }
    store(connection, owner, state)
}
