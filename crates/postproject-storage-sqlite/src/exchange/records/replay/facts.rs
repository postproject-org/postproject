//! Original observations and semantic versions shared by structural effect apply.

use postproject_core::{RevisionEventKind, SemanticConflictKey};
use postproject_protocol::RecordManifest;
use rusqlite::{Transaction, params};

use crate::{
    ExchangeResult, sqlite_error,
    transaction::{encode_conflict_key, persist_revision_event},
};

pub(super) fn observation(
    transaction: &Transaction<'_>,
    manifest: &RecordManifest,
    position: u64,
    event: &RevisionEventKind,
) -> ExchangeResult<()> {
    if position >= manifest.event_count() {
        return Err(super::effects::invalid().into());
    }
    let position = u32::try_from(position).map_err(|_| super::effects::invalid())?;
    persist_revision_event(
        transaction,
        manifest.revision().id(),
        i64::from(position),
        event,
    )?;
    Ok(())
}

pub(super) fn changed(
    transaction: &Transaction<'_>,
    manifest: &RecordManifest,
    key: &SemanticConflictKey,
) -> ExchangeResult<()> {
    let key = encode_conflict_key(key)?;
    let revision = manifest.revision();
    transaction.execute("INSERT INTO conflict_versions (conflict_key, last_changed_revision_id, last_changed_revision_sequence) VALUES (?1, ?2, ?3) ON CONFLICT(conflict_key) DO UPDATE SET last_changed_revision_id = excluded.last_changed_revision_id, last_changed_revision_sequence = excluded.last_changed_revision_sequence", params![key, revision.id().as_bytes().as_slice(), i64::try_from(revision.sequence()).map_err(|_| super::effects::invalid())?])
        .map_err(sqlite_error("persist replayed semantic version"))?;
    Ok(())
}

pub(super) fn structural<T>(result: postproject_core::Result<T>) -> ExchangeResult<T> {
    result.map_err(|error| match error.kind() {
        postproject_core::ErrorKind::Storage | postproject_core::ErrorKind::Internal => {
            error.into()
        }
        postproject_core::ErrorKind::Unsupported => postproject_protocol::ProtocolError::new(
            postproject_protocol::FailureKind::Unsupported,
            "portable fact is unsupported by this receiver",
        )
        .into(),
        _ => super::effects::invalid().into(),
    })
}
