//! Atomic capture of all current native mutation families.

pub(super) mod chunks;
mod reader;
mod replay;
mod writer;

pub use reader::RecordReader;
pub use replay::ReplayLimits;
pub(crate) use replay::apply;

use postproject_core::{
    Error, ErrorKind, ProductionId, Result, Revision, RevisionEvent, RevisionEventKind,
};
use postproject_protocol::{
    Document, Extensions, Limits, Position, RecordFeature, RecordManifest, encode_event,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::sqlite_error;

pub(crate) fn position(
    connection: &Connection,
    production: ProductionId,
    sequence: u64,
) -> Result<Option<Position>> {
    let floor = super::floor(connection, production)?;
    if sequence == floor.sequence() {
        return Ok(Some(floor));
    }
    manifest(connection, production, sequence)?
        .map(|manifest| manifest.head())
        .transpose()
        .map_err(|_| invalid())
}

pub(super) fn manifest(
    connection: &Connection,
    production: ProductionId,
    sequence: u64,
) -> Result<Option<RecordManifest>> {
    let floor = super::floor(connection, production)?;
    if sequence <= floor.sequence() {
        return Ok(None);
    }
    let bytes = connection
        .query_row(
            "SELECT manifest FROM exchange_records WHERE sequence = ?1",
            [i64::try_from(sequence).map_err(|_| invalid())?],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(sqlite_error("read committed record manifest"))?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let manifest = Document::parse(
        &bytes,
        Limits::new(262_144, 192, 8192).map_err(|_| invalid())?,
    )
    .and_then(|document| RecordManifest::from_document(&document))
    .map_err(|_| invalid())?;
    let position = manifest.head().map_err(|_| invalid())?;
    if position.scope() != floor.scope() || position.sequence() != sequence {
        return Err(invalid());
    }
    let revision = connection
        .query_row(
            "SELECT id FROM revisions WHERE sequence = ?1",
            [i64::try_from(sequence).map_err(|_| invalid())?],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .map_err(sqlite_error("validate committed record revision"))?;
    if Some(revision.as_slice())
        != position
            .revision()
            .as_ref()
            .map(|id| id.as_bytes().as_slice())
    {
        return Err(invalid());
    }
    Ok(Some(manifest))
}

pub(crate) fn capture_records(
    connection: &Connection,
    production: ProductionId,
    revision: &Revision,
    effects: &[super::CapturedEffect],
    events: &[RevisionEventKind],
    extensions: &Extensions,
) -> Result<()> {
    // A native changing commit must never succeed without complete capture.
    if !events.iter().all(|event| {
        matches!(
            event,
            RevisionEventKind::MetadataAddedOrReplaced { .. }
                | RevisionEventKind::MetadataRemoved { .. }
                | RevisionEventKind::AssetImported { .. }
                | RevisionEventKind::RepresentationAdded { .. }
                | RevisionEventKind::ResourceAdded { .. }
                | RevisionEventKind::ResourceFileFactsObserved { .. }
                | RevisionEventKind::ResourceFingerprintObserved { .. }
                | RevisionEventKind::RepresentationFingerprintObserved { .. }
                | RevisionEventKind::ExternalIdentifierAdded { .. }
                | RevisionEventKind::ExternalIdentifierRemoved { .. }
                | RevisionEventKind::DependencySetRecorded { .. }
                | RevisionEventKind::ActivityCreated { .. }
                | RevisionEventKind::ActivityInputAdded { .. }
                | RevisionEventKind::ActivityOutputAdded { .. }
                | RevisionEventKind::JobRequested { .. }
                | RevisionEventKind::JobClaimed { .. }
                | RevisionEventKind::JobClaimRenewed { .. }
                | RevisionEventKind::JobClaimReleased { .. }
                | RevisionEventKind::JobSucceeded { .. }
                | RevisionEventKind::JobFailed { .. }
                | RevisionEventKind::JobCancelled { .. }
                | RevisionEventKind::RepresentationResourceAdded { .. }
                | RevisionEventKind::LocatorAdded { .. }
                | RevisionEventKind::LocatorRetired { .. }
                | RevisionEventKind::MediaRootAdded { .. }
                | RevisionEventKind::MediaRootEnabledChanged { .. }
                | RevisionEventKind::MediaRootRemoved { .. }
        )
    }) {
        return Err(incomplete());
    }
    let Some(predecessor) = position(connection, production, revision.sequence() - 1)? else {
        return Err(Error::new(
            ErrorKind::Storage,
            "native commit lacks its complete exchange predecessor",
        ));
    };
    let mut expected = events.iter();
    for effect in effects {
        effect.visit_observations(connection, |event| {
            if expected.next() != Some(&event) {
                return Err(incomplete());
            }
            Ok(())
        })?;
    }
    if effects.is_empty() || expected.next().is_some() {
        return Err(incomplete());
    }
    let mut writer = writer::RecordWriter::new(connection, predecessor.scope(), revision.id());
    for effect in effects {
        effect.write_frames(connection, revision.sequence(), |frame| {
            writer.document(frame)
        })?;
    }
    for (position, event) in events.iter().enumerate() {
        let position = u32::try_from(position).map_err(|_| encoding())?;
        let event = RevisionEvent::new(revision.id(), position, event.clone());
        writer.document(&encode_event(&event).map_err(|_| encoding())?)?;
    }
    let chunks = writer.finish()?;
    let manifest = RecordManifest::new(
        predecessor,
        revision.clone(),
        chunks,
        u64::try_from(effects.len()).map_err(|_| encoding())?,
        u64::try_from(events.len()).map_err(|_| encoding())?,
        extensions.clone(),
    )
    .and_then(|manifest| {
        manifest.with_required_features(
            std::iter::once(RecordFeature::RecordChunks)
                .chain(effects.iter().map(super::CapturedEffect::feature)),
        )
    })
    .and_then(|manifest| manifest.document())
    .and_then(|document| document.canonical_bytes())
    .map_err(|_| encoding())?;
    connection
        .execute(
            "INSERT INTO exchange_records (revision_id, sequence, manifest) VALUES (?1, ?2, ?3)",
            params![
                revision.id().as_bytes().as_slice(),
                i64::try_from(revision.sequence()).map_err(|_| encoding())?,
                manifest
            ],
        )
        .map_err(sqlite_error("persist committed record manifest"))?;
    Ok(())
}

fn invalid() -> Error {
    Error::new(ErrorKind::Storage, "invalid stored record manifest")
}
fn incomplete() -> Error {
    Error::new(
        ErrorKind::Internal,
        "authored capture does not cover every observation",
    )
}
fn encoding() -> Error {
    Error::new(
        ErrorKind::Internal,
        "cannot encode complete authored record",
    )
}
