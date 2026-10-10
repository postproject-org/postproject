//! Only a fully checked private transaction can reach the destination name.

mod activity_state;
mod bodies;
mod completion;
mod dependency_state;
mod fingerprint_state;
mod guard_state;
mod identifier_state;
mod limits;
mod locator_state;
mod media_state;
mod metadata_state;
mod recomputation_state;
mod recovery;
mod root_state;
mod staging;

#[cfg(test)]
mod tests;

pub use limits::CheckpointLimits;
pub(crate) use recovery::recover;

use std::path::Path;

use postproject_protocol::{
    CheckpointChunk, CheckpointManifest, Extensions, FailureKind, FrameDecoder, ProtocolError,
};

use crate::{ExchangeResult, SqliteProduction, sqlite_error};

pub(crate) fn import(
    destination: &Path,
    manifest: &CheckpointManifest,
    chunks: impl IntoIterator<Item = ExchangeResult<CheckpointChunk>>,
    limits: CheckpointLimits,
) -> ExchangeResult<SqliteProduction> {
    if manifest.extensions() != &Extensions::default() {
        return Err(unsupported_extensions().into());
    }
    let mut remaining = limits
        .encoded_bytes
        .checked_sub(
            u64::try_from(manifest.document()?.canonical_bytes()?.len())
                .map_err(|_| limits::budget())?,
        )
        .ok_or_else(limits::budget)?;
    let (stage, mut connection) = staging::Staging::new(destination, manifest, limits)?;
    let transaction = connection
        .transaction()
        .map_err(sqlite_error("begin private checkpoint import"))?;
    transaction
        .execute_batch("PRAGMA defer_foreign_keys = ON;")
        .map_err(sqlite_error("defer private checkpoint cross-references"))?;
    metadata_state::create(&transaction)?;
    root_state::create(&transaction)?;
    guard_state::create(&transaction)?;
    media_state::create(&transaction)?;
    locator_state::create(&transaction)?;
    identifier_state::create(&transaction)?;
    recomputation_state::create(&transaction)?;
    dependency_state::create(&transaction, limits.frames)?;
    let mut bodies = bodies::Bodies::new(&transaction, manifest, limits);
    let mut chunks = chunks.into_iter();
    for summary in manifest.sections() {
        if summary.section() == postproject_protocol::CheckpointSection::Records {
            fingerprint_state::create(&transaction)?;
            activity_state::create(&transaction, limits.frames, || {
                stage.check_disk(limits.disk_bytes)
            })?;
        }
        let Some(declared) = summary.chunks() else {
            continue;
        };
        let mut chain = manifest
            .section_chain(summary.section())
            .ok_or_else(super::invalid)?;
        let mut decoder = FrameDecoder::new(limits.document);
        let mut items = 0_u64;
        for _ in 0..declared.count() {
            let chunk = chunks.next().ok_or_else(super::invalid)??;
            if chunk.extensions() != &Extensions::default() {
                return Err(unsupported_extensions().into());
            }
            remaining = remaining
                .checked_sub(
                    u64::try_from(chunk.document()?.canonical_bytes()?.len())
                        .map_err(|_| limits::budget())?,
                )
                .ok_or_else(limits::budget)?;
            chain.push(&chunk)?;
            let mut offset = 0;
            while offset < chunk.payload().len() {
                let (consumed, document) = decoder.consume(&chunk.payload()[offset..])?;
                offset += consumed;
                if let Some(document) = document {
                    if bodies.document(summary.section(), &document)? {
                        items = items.checked_add(1).ok_or_else(super::invalid)?;
                        if items > summary.items() {
                            return Err(super::invalid().into());
                        }
                    }
                }
            }
            stage.check_disk(limits.disk_bytes)?;
        }
        decoder.finish()?;
        manifest.verify_section(summary.section(), chain)?;
        if items != summary.items() {
            return Err(super::invalid().into());
        }
    }
    if let Some(extra) = chunks.next() {
        extra?;
        return Err(super::invalid().into());
    }
    bodies.finish()?;
    activity_state::finish(&transaction, manifest.floor().sequence())?;
    dependency_state::finish(&transaction, manifest.floor().sequence())?;
    media_state::finish(&transaction, manifest.floor().sequence() == 0)?;
    locator_state::finish(&transaction, manifest.floor().sequence() == 0)?;
    identifier_state::finish(&transaction, manifest.floor().sequence() == 0)?;
    fingerprint_state::finish(&transaction, manifest.floor().sequence())?;
    recomputation_state::finish(&transaction, manifest.floor().sequence())?;
    metadata_state::finish(&transaction, manifest.floor().sequence() == 0)?;
    root_state::finish(&transaction, manifest.floor().sequence() == 0)?;
    guard_state::finish(&transaction, manifest.floor().sequence())?;
    transaction
        .commit()
        .map_err(sqlite_error("commit complete private checkpoint"))?;
    stage.check_disk(limits.disk_bytes)?;
    let path = stage.promote(connection)?;
    Ok(SqliteProduction::open(path)?)
}

fn unsupported_extensions() -> ProtocolError {
    ProtocolError::new(
        FailureKind::Unsupported,
        "checkpoint extension preservation is not implemented yet",
    )
}
