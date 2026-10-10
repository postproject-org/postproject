use std::cell::{Cell, RefCell};

use postproject_core::{DependencySet, DependencySetStatus, Error, ErrorKind, RepresentationId};
use rusqlite::Connection;

use crate::{
    ExchangeError, ExchangeResult,
    dependency_snapshot::{WalkSet, validate_dependency_paths_with},
};

use super::{baseline, invalid, occurrence};

enum Knowledge {
    Known(Option<WalkSet>),
    Unknown,
}

fn available(
    connection: &Connection,
    owner: RepresentationId,
    floor: u64,
) -> ExchangeResult<Knowledge> {
    super::charge(connection, 1)?;
    let state = baseline::ensure(connection, owner, floor)?;
    if state.present == Some(false) {
        return Ok(Knowledge::Known(None));
    }
    if state.present == Some(true) && state.dirty == Some(true) {
        return Ok(Knowledge::Known(Some(WalkSet::NeedsExtraction)));
    }
    let (Some(true), Some(false), Some(recorded), Some(count)) =
        (state.present, state.dirty, state.recorded, state.count)
    else {
        return Ok(Knowledge::Unknown);
    };
    let mut dependencies = Vec::new();
    let mut budget = crate::read_budget::ReadBudget::default();
    super::charge(
        connection,
        crate::stored_u64(count, "authored dependency count")?,
    )?;
    for position in 0..crate::stored_u64(count, "authored dependency count")? {
        let value = occurrence::prior(connection, owner, position)?.ok_or_else(invalid)?;
        let dependency = value.dependency();
        // Match the seven native SQL value sizes, without charging JSON
        // escaping or field names against an existing native read limit.
        let bytes = 32
            + dependency.kind().as_str().len()
            + dependency.authored_reference().len()
            + dependency
                .source_resource_id()
                .map_or(0, |id| id.as_bytes().len())
            + dependency
                .resolved_representation_id()
                .map_or(0, |id| id.as_bytes().len());
        budget.record(bytes)?;
        dependencies.push(value.dependency().clone());
    }
    let set = DependencySet::new(
        owner,
        crate::stored_u64(recorded, "authored dependency revision")?,
        DependencySetStatus::Current,
        dependencies,
    )
    .map_err(|_| invalid())?;
    Ok(Knowledge::Known(Some(WalkSet::Known(set))))
}

pub(in crate::exchange::checkpoint::import) fn validate_paths(
    connection: &Connection,
    input: i64,
    owner: RepresentationId,
    floor: u64,
) -> ExchangeResult<()> {
    let unknown = Cell::new(false);
    let failure = RefCell::new(None);
    let load = |owner| match available(connection, owner, floor) {
        Ok(Knowledge::Known(set)) => Ok(set),
        Ok(Knowledge::Unknown) => {
            unknown.set(true);
            Err(Error::new(
                ErrorKind::Unsupported,
                "pre-floor dependency contents are unavailable",
            ))
        }
        Err(ExchangeError::Store(error)) => Err(error),
        Err(ExchangeError::Protocol(error)) => {
            *failure.borrow_mut() = Some(error);
            Err(Error::new(
                ErrorKind::InvalidArgument,
                "invalid authored dependency evidence",
            ))
        }
    };
    let result = validate_dependency_paths_with(connection, input, owner, &load);
    if let Some(error) = failure.into_inner() {
        return Err(error.into());
    }
    match result {
        Ok(()) => Ok(()),
        // Only our explicit unavailable-prefix sentinel permits partial checks.
        // Every available segment and fingerprint is still checked separately.
        Err(_) if unknown.get() => Ok(()),
        Err(error) if error.kind() == ErrorKind::InvalidArgument => Err(invalid().into()),
        Err(error) => Err(error.into()),
    }
}
