use postproject_core::{
    Asset, AssetId, ContentStructure, ExternalIdentifier, FileFacts, IdentifierScheme, Locator,
    LocatorAvailability, LocatorId, MetadataProperty, MetadataValue, ObjectRef,
    OriginalMediaImport, PropertyId, Representation, RepresentationFingerprint, RepresentationId,
    RepresentationKind, Resource, ResourceFingerprint, ResourceId, Timestamp, VocabularyId,
};
use postproject_protocol::{FrameDecoder, Limits};

use super::super::RetainedEffects;
use crate::{
    SqliteProduction,
    exchange::checkpoint::import::{
        fingerprint_state, guard_state, identifier_state, locator_state, media_state,
        metadata_state, recomputation_state, root_state,
    },
};

#[test]
fn complete_native_media_records_explain_later_state_without_reconstructing_creation() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    populate(&mut source);
    let connection = &source.connection;
    metadata_state::create(connection).unwrap();
    root_state::create(connection).unwrap();
    guard_state::create(connection).unwrap();
    media_state::create(connection).unwrap();
    locator_state::create(connection).unwrap();
    identifier_state::create(connection).unwrap();
    fingerprint_state::create(connection).unwrap();
    recomputation_state::create(connection).unwrap();
    for sequence in 1..=2 {
        let revision = source.changes_since(sequence - 1, 1).unwrap().remove(0);
        for event in source.events_for_revision(revision.id()).unwrap() {
            guard_state::observed(connection, &event, sequence).unwrap();
        }
        let mut reader = source.record_reader(sequence).unwrap();
        let mut effects = RetainedEffects::new(reader.manifest(), 0);
        let mut decoder = FrameDecoder::new(Limits::default());
        while let Some(chunk) = reader.next_chunk().unwrap() {
            let mut offset = 0;
            while offset < chunk.payload().len() {
                let (consumed, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
                offset += consumed;
                if let Some(document) = document {
                    effects.document(connection, &document).unwrap();
                }
            }
        }
        decoder.finish().unwrap();
        effects.finish().unwrap();
    }
    media_state::finish(connection, true).unwrap();
    locator_state::finish(connection, true).unwrap();
    identifier_state::finish(connection, true).unwrap();
    fingerprint_state::finish(connection, 0).unwrap();
    recomputation_state::finish(connection, 0).unwrap();
    metadata_state::finish(connection, true).unwrap();
    root_state::finish(connection, true).unwrap();
    guard_state::finish(connection, 0).unwrap();
    let leftovers: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name LIKE 'checkpoint_%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(leftovers, 0);
}

fn populate(source: &mut SqliteProduction) {
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(-1), None, None);
    let resource = Resource::new(
        ResourceId::new(),
        vec![ResourceFingerprint::new("content", 1, vec![1]).unwrap()],
        Some(FileFacts::new(123, None)),
    );
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        ContentStructure::single_resource(resource.id()),
        vec![RepresentationFingerprint::new("aggregate", 1, vec![11]).unwrap()],
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource.id(),
        "file:///missing.mov",
        None,
        LocatorAvailability::Offline,
    )
    .unwrap();
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("unknown:CASE").unwrap(),
        "Exact 名",
        Some("Qual".into()),
    )
    .unwrap();
    let import =
        OriginalMediaImport::new(asset, representation, vec![resource], vec![locator]).unwrap();
    let resource = import.resources()[0].id();
    let representation = import.representation().id();
    let target = ObjectRef::Asset(import.asset().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.add_external_identifier(target, &identifier).unwrap();
    let property = MetadataProperty::new(
        VocabularyId::new("unknown:CASE").unwrap(),
        PropertyId::new("Exact").unwrap(),
    );
    edit.add_metadata_value(
        ObjectRef::Resource(resource),
        &property,
        &MetadataValue::string("Exact 名").unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_resource_fingerprint(
        resource,
        &ResourceFingerprint::new("content", 1, vec![2]).unwrap(),
    )
    .unwrap();
    edit.record_representation_fingerprint(
        representation,
        &RepresentationFingerprint::new("aggregate", 1, vec![12]).unwrap(),
    )
    .unwrap();
    edit.record_resource_fingerprint(
        resource,
        &ResourceFingerprint::new("content", 1, vec![3]).unwrap(),
    )
    .unwrap();
    edit.record_resource_file_facts(
        resource,
        FileFacts::new(456, Some(Timestamp::from_unix_micros(-99))),
    )
    .unwrap();
    edit.remove_external_identifier(target, &identifier)
        .unwrap();
    edit.retire_locator(import.locators()[0].id()).unwrap();
    edit.add_locator(
        &Locator::new(
            import.locators()[0].id(),
            resource,
            "file:///replacement.mov",
            None,
            LocatorAvailability::Unknown,
        )
        .unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
}
