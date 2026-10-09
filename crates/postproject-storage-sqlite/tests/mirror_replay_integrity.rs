//! Valid hashes do not waive scope, continuity, structural or event validation.

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionEvent, RevisionEventKind,
    VocabularyId,
};
use postproject_protocol::{
    ChunkSummary, Extensions, FailureKind, MetadataEffect, RecordChunk, RecordManifest,
    encode_event,
};
use postproject_storage_sqlite::{ExchangeError, ReplayLimits, SqliteProduction};

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:integrity:test").unwrap(),
        PropertyId::new("value").unwrap(),
    )
}

fn forged(
    source: &SqliteProduction,
    position: u64,
    wrong_event: bool,
) -> (RecordManifest, RecordChunk) {
    let manifest = source.record_reader(1).unwrap().manifest().clone();
    let target = ObjectRef::Production(source.production().id());
    let effect =
        MetadataEffect::appended(target, property(), position, MetadataValue::i64(4)).unwrap();
    let event = if wrong_event {
        RevisionEventKind::MetadataRemoved {
            target,
            property: property(),
        }
    } else {
        RevisionEventKind::MetadataAddedOrReplaced {
            target,
            property: property(),
        }
    };
    let event = encode_event(&RevisionEvent::new(manifest.revision().id(), 0, event)).unwrap();
    let mut payload = Vec::new();
    for document in effect
        .frames()
        .map(Result::unwrap)
        .chain(std::iter::once(event))
    {
        let bytes = document.canonical_bytes().unwrap();
        payload.extend_from_slice(&u64::try_from(bytes.len()).unwrap().to_be_bytes());
        payload.extend_from_slice(&bytes);
    }
    let size = u64::try_from(payload.len()).unwrap();
    let chunk = RecordChunk::new(
        manifest.predecessor().scope(),
        manifest.revision().id(),
        0,
        None,
        payload,
        Extensions::default(),
    )
    .unwrap();
    let manifest = RecordManifest::new(
        manifest.predecessor(),
        manifest.revision().clone(),
        ChunkSummary::new(1, size, chunk.digest().unwrap()).unwrap(),
        1,
        1,
        Extensions::default(),
    )
    .unwrap();
    (manifest, chunk)
}

fn append(source: &mut SqliteProduction) {
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(4))
        .unwrap();
    edit.commit().unwrap();
}

#[test]
fn rehashed_wrong_positions_or_events_roll_back_all_staged_state() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let anchor = mirror.exchange_head().unwrap();
    append(&mut source);
    for (position, wrong_event) in [(1, false), (0, true)] {
        let (manifest, chunk) = forged(&source, position, wrong_event);
        assert!(
            matches!(mirror.apply_record(&manifest, [Ok(chunk)], ReplayLimits::default()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::Integrity)
        );
        assert_eq!(mirror.exchange_head().unwrap(), anchor);
        assert_eq!(
            mirror.changes_since(0, 10).unwrap(),
            Vec::<postproject_core::Revision>::new()
        );
        assert_eq!(
            mirror
                .metadata_values(ObjectRef::Production(source.production().id()), &property())
                .unwrap(),
            Vec::<MetadataValue>::new()
        );
    }
}

#[test]
fn foreign_scopes_gaps_authority_apply_and_altered_duplicates_reject() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let other = SqliteProduction::create(directory.path().join("other.pproj"), None).unwrap();
    let mut wrong = SqliteProduction::create_genesis_mirror(
        directory.path().join("wrong.pproj"),
        other.production(),
        other.exchange_floor().unwrap(),
    )
    .unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    append(&mut source);
    append(&mut source);
    let first = source.record_reader(1).unwrap().manifest().clone();
    let second = source.record_reader(2).unwrap().manifest().clone();
    assert!(
        matches!(wrong.apply_record(&first, [], ReplayLimits::default()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::ScopeMismatch)
    );
    assert!(
        matches!(mirror.apply_record(&second, [], ReplayLimits::default()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::HistoryGap)
    );
    assert!(
        matches!(source.apply_record(&first, [], ReplayLimits::default()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::Unsupported)
    );
    let mut reader = source.record_reader(1).unwrap();
    let chunk = reader.next_chunk().unwrap().unwrap();
    assert!(
        mirror
            .apply_record(&first, [Ok(chunk.clone())], ReplayLimits::default())
            .unwrap()
    );
    assert!(
        matches!(mirror.apply_record(&first, [Ok(chunk.clone()), Ok(chunk)], ReplayLimits::default()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::Integrity)
    );
    let (altered, chunk) = forged(&source, 1, false);
    assert!(
        matches!(mirror.apply_record(&altered, [Ok(chunk)], ReplayLimits::default()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::Divergence)
    );
    assert_eq!(mirror.exchange_head().unwrap(), first.head().unwrap());
}
