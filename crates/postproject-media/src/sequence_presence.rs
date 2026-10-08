//! Bounded observations of existing sequence members.

use std::{collections::BTreeSet, fs, path::Path};

use postproject_core::{
    Error, ErrorKind, ImageSequenceDescriptor, MAX_SEQUENCE_EXCEPTIONS, Result, SequenceNaming,
};

use crate::{resolution_budget::ResolutionBudget, resolver::ResolverOptions};

pub(crate) fn observe_sequence_directory(
    path: &Path,
    naming: &SequenceNaming,
    descriptor: &ImageSequenceDescriptor,
    options: &ResolverOptions,
    budget: &mut ResolutionBudget,
) -> Result<Option<Vec<i64>>> {
    let entries = fs::read_dir(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("list sequence directory {}: {error}", path.display()),
        )
    })?;
    let mut present = BTreeSet::new();
    for (index, entry) in entries.enumerate() {
        check_cancelled(options)?;
        if index >= options.max_entries_per_directory {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "sequence directory exceeds the entry budget; incomplete listings cannot establish missing frames",
            ));
        }
        let entry = entry.map_err(|error| {
            Error::new(ErrorKind::Io, format!("read sequence directory: {error}"))
        })?;
        let name = entry.file_name();
        budget.record(
            1,
            name.as_encoded_bytes()
                .len()
                .saturating_add(size_of::<i64>()),
        )?;
        let Some(name) = name.to_str() else { continue };
        let Some(number) = name
            .strip_prefix(naming.prefix())
            .and_then(|rest| rest.strip_suffix(naming.suffix()))
        else {
            continue;
        };
        let Ok(frame) = number.parse::<i64>() else {
            continue;
        };
        if descriptor.frames().contains(frame)
            && !descriptor.is_known_missing(frame)
            && naming.filename(frame) == name
            && entry.path().is_file()
        {
            present.insert(frame);
        }
    }
    if present.is_empty() {
        return Ok(None);
    }
    let expected =
        descriptor.frames().frame_count() - descriptor.known_missing_frames().len() as u128;
    let missing = expected - present.len() as u128;
    if missing > MAX_SEQUENCE_EXCEPTIONS as u128 {
        return Err(Error::new(
            ErrorKind::Unsupported,
            "sequence missing-frame observations exceed 100000 entries",
        ));
    }
    // With bounded observed, declared-missing and newly missing sets, this
    // loop is bounded even when the descriptor spans the full i64 domain.
    let mut missing_frames = Vec::new();
    let frames = descriptor.frames();
    let mut frame = frames.start();
    loop {
        check_cancelled(options)?;
        if !descriptor.is_known_missing(frame) && !present.contains(&frame) {
            missing_frames.push(frame);
        }
        if frame == frames.end() {
            break;
        }
        frame += i64::from(frames.step());
    }
    Ok(Some(missing_frames))
}

fn check_cancelled(options: &ResolverOptions) -> Result<()> {
    options
        .cancellation
        .as_ref()
        .map_or(Ok(()), postproject_core::CancellationToken::check)
}
