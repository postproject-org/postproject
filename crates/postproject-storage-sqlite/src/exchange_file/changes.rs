use std::io::{Read, Seek};

use postproject_protocol::{
    FailureKind, Position, ProtocolError, RecordChunk, RecordManifest, StoreRole,
};

use super::{FileLimits, FileManifest, FileReader, framing};
use crate::{ExchangeResult, ReplayLimits, SqliteProduction};

/// Applies a sealed contiguous change range, atomically committing each record.
///
/// Returns the advertised source head after the complete range. Exact duplicates
/// are verified. On interruption or failure, earlier complete records remain
/// durable; inspect the mirror head and retry the same file. An incomplete record
/// never becomes visible. No worker capability or local revision is created.
///
/// # Errors
/// Rejects foreign scopes, unavailable predecessors, divergent records, wrong
/// file kinds, incomplete framing and receiver budgets. Authorities reject even
/// an empty range. A failed response does not imply that no record committed.
pub fn apply_changes(
    destination: &mut SqliteProduction,
    source: impl Read + Seek,
    file_limits: FileLimits,
    replay_limits: ReplayLimits,
) -> ExchangeResult<Position> {
    if destination.exchange_role() != StoreRole::PassiveMirror {
        return Err(ProtocolError::new(
            FailureKind::MirrorReadOnly,
            "change streams require a passive mirror",
        )
        .into());
    }
    let mut reader = FileReader::open(source, file_limits)?;
    let FileManifest::Changes(through) = *reader.manifest() else {
        return Err(framing::invalid().into());
    };
    let from = Position::from_document(&reader.next_document()?.ok_or_else(framing::invalid)?)?;
    if from.scope() != through.scope() || from.scope() != destination.exchange_scope()? {
        return Err(ProtocolError::new(
            FailureKind::ScopeMismatch,
            "change range has a foreign source scope",
        )
        .into());
    }
    if from.sequence() > through.sequence() {
        return Err(framing::invalid().into());
    }
    let available = crate::exchange::position(
        &destination.connection,
        destination.production.id(),
        from.sequence(),
    )?;
    if available != Some(from) {
        return Err(ProtocolError::new(
            FailureKind::HistoryGap,
            "mirror lacks the change range predecessor",
        )
        .into());
    }
    let mut previous = from;
    while previous.sequence() < through.sequence() {
        let manifest =
            RecordManifest::from_document(&reader.next_document()?.ok_or_else(framing::invalid)?)?;
        if manifest.predecessor() != previous || manifest.revision().sequence() > through.sequence()
        {
            return Err(ProtocolError::new(
                FailureKind::HistoryGap,
                "change range is not contiguous",
            )
            .into());
        }
        let mut remaining = manifest.chunks().count();
        let chunks = std::iter::from_fn(|| {
            if remaining == 0 {
                return None;
            }
            remaining -= 1;
            Some(reader.next_document().and_then(|document| {
                Ok(RecordChunk::from_document(
                    &document.ok_or_else(framing::invalid)?,
                )?)
            }))
        });
        destination.apply_record(&manifest, chunks, replay_limits)?;
        previous = manifest.head()?;
    }
    if previous != through || reader.next_document()?.is_some() {
        return Err(framing::invalid().into());
    }
    Ok(through)
}
