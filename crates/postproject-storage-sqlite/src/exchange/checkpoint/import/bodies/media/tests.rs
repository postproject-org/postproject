use postproject_core::{
    Asset, AssetId, ContentStructure, FileFacts, FrameRange, ImageSequenceDescriptor, Locator,
    LocatorAvailability, LocatorId, OriginalMediaImport, RationalRate, Representation,
    RepresentationId, RepresentationKind, Resource, ResourceId, ResourceMember, ResourceRole,
    SequenceNaming, Timestamp,
};

use crate::{SqliteProduction, exchange::checkpoint::media_facts};
use postproject_core::{ExternalIdentifier, IdentifierScheme, ObjectRef};

#[test]
fn scalar_and_structural_reads_preserve_all_shapes_without_loading_other_collections() {
    let member = |id, required| {
        ResourceMember::new(id, ResourceRole::new("unknown:role").unwrap(), required)
    };
    let first = ResourceId::new();
    let second = ResourceId::new();
    for structure in [
        ContentStructure::single_resource(first),
        ContentStructure::image_sequence(
            ImageSequenceDescriptor::new(
                first,
                FrameRange::new(-10, 10, 2).unwrap(),
                RationalRate::new(24_000, 1_001).unwrap(),
                vec![-2, 4],
            )
            .unwrap(),
        ),
        ContentStructure::ordered_parts(vec![member(second, true), member(first, true)]).unwrap(),
        ContentStructure::package(vec![member(first, true), member(second, false)]).unwrap(),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut source =
            SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
        let asset = Asset::new(
            AssetId::new(),
            Timestamp::from_unix_micros(-1),
            Some("Exact 名".into()),
            None,
        );
        let representation = Representation::new(
            RepresentationId::new(),
            asset.id(),
            RepresentationKind::Original,
            structure,
            Vec::new(),
        );
        let resources: Vec<_> = representation
            .content_structure()
            .resource_ids()
            .into_iter()
            .map(|id| {
                Resource::new(
                    id,
                    Vec::new(),
                    Some(FileFacts::new(123, Some(Timestamp::from_unix_micros(-99)))),
                )
            })
            .collect();
        let locators = resources
            .iter()
            .map(|resource| {
                let locator = Locator::new(
                    LocatorId::new(),
                    resource.id(),
                    "file:///does-not-exist",
                    None,
                    LocatorAvailability::Offline,
                )
                .unwrap();
                if representation
                    .content_structure()
                    .image_sequence_descriptor()
                    .is_some()
                {
                    locator.with_sequence_naming(SequenceNaming::new("shot.", ".exr", 4).unwrap())
                } else {
                    locator
                }
            })
            .collect();
        let import =
            OriginalMediaImport::new(asset.clone(), representation.clone(), resources, locators)
                .unwrap();
        let mut edit = source.begin_transaction().unwrap();
        edit.import_original(&import).unwrap();
        edit.add_external_identifier(ObjectRef::Asset(asset.id()), &attachment())
            .unwrap();
        edit.commit().unwrap();
        drop(edit);
        assert_eq!(
            media_facts::asset(&source.connection, asset.id()).unwrap(),
            asset
        );
        assert_eq!(
            media_facts::representation(&source.connection, representation.id()).unwrap(),
            postproject_protocol::RepresentationHeader::from_representation(&representation)
        );
        assert_eq!(
            media_facts::content(&source.connection, representation.id()).unwrap(),
            *representation.content_structure()
        );
        for resource in import.resources() {
            assert_eq!(
                media_facts::resource(&source.connection, resource.id()).unwrap(),
                postproject_protocol::ResourceHeader::from_resource(resource)
            );
        }
        assert_staging_round_trip(&source, &import);
        source.connection.execute("DELETE FROM representation_resources WHERE representation_id = ?1 AND position = 0", [representation.id().as_bytes().as_slice()]).unwrap();
        assert!(media_facts::content(&source.connection, representation.id()).is_err());
    }
}

fn assert_checkpoint_round_trip(source: &SqliteProduction, import: &OriginalMediaImport) {
    let directory = tempfile::tempdir().unwrap();
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let mirror = SqliteProduction::import_checkpoint(
        directory.path().join("mirror.pproj"),
        &manifest,
        chunks.into_iter().map(Ok),
        crate::CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        source.asset(import.asset().id()).unwrap(),
        mirror.asset(import.asset().id()).unwrap()
    );
    let representation = import.representation().id();
    assert_eq!(
        source.representation(representation).unwrap(),
        mirror.representation(representation).unwrap()
    );
    assert_eq!(
        source.resources(representation).unwrap(),
        mirror.resources(representation).unwrap()
    );
    for resource in import.resources() {
        assert_eq!(
            source.locators(resource.id()).unwrap(),
            mirror.locators(resource.id()).unwrap()
        );
    }
    let target = ObjectRef::Asset(import.asset().id());
    assert_eq!(
        source.external_identifiers(target).unwrap(),
        mirror.external_identifiers(target).unwrap()
    );
    assert_eq!(
        source.changes_since(0, 10).unwrap(),
        mirror.changes_since(0, 10).unwrap()
    );
    assert_eq!(
        source.exchange_head().unwrap(),
        mirror.exchange_head().unwrap()
    );
}

fn assert_staging_round_trip(source: &SqliteProduction, import: &OriginalMediaImport) {
    use super::super::Bodies;
    use crate::exchange::checkpoint::{sections, writer::SectionWriter};
    use postproject_protocol::{CheckpointId, CheckpointSection};
    let directory = tempfile::tempdir().unwrap();
    let mut destination =
        SqliteProduction::create(directory.path().join("staged.pproj"), None).unwrap();
    let manifest = destination.export_checkpoint(|_| Ok(())).unwrap();
    let transaction = destination.connection.transaction().unwrap();
    transaction
        .execute_batch("PRAGMA defer_foreign_keys = ON;")
        .unwrap();
    let mut bodies = Bodies::new(&transaction, &manifest, crate::CheckpointLimits::default());
    for section in [
        CheckpointSection::Assets,
        CheckpointSection::Resources,
        CheckpointSection::Representations,
        CheckpointSection::Structures,
        CheckpointSection::Locators,
        CheckpointSection::Identifiers,
    ] {
        let mut chunks = Vec::new();
        let mut sink = |chunk| {
            chunks.push(chunk);
            Ok(())
        };
        let mut writer = SectionWriter::new(
            source.exchange_head().unwrap().scope(),
            CheckpointId::new(),
            section,
            &mut sink,
        );
        match section {
            CheckpointSection::Locators => sections::locators(source, &mut writer).unwrap(),
            CheckpointSection::Identifiers => sections::identifiers(source, &mut writer).unwrap(),
            _ => sections::media(source, &mut writer, section).unwrap(),
        }
        let summary = writer.finish().unwrap();
        assert_eq!(
            summary.items(),
            if matches!(
                section,
                CheckpointSection::Resources | CheckpointSection::Locators
            ) {
                import.resources().len() as u64
            } else {
                1
            }
        );
        let starts = stage_section(&mut bodies, section, chunks);
        if section == CheckpointSection::Structures {
            assert_eq!(starts, summary.items());
        }
    }
    assert_eq!(
        media_facts::asset(&transaction, import.asset().id()).unwrap(),
        *import.asset()
    );
    assert_eq!(
        media_facts::content(&transaction, import.representation().id()).unwrap(),
        *import.representation().content_structure()
    );
    assert_staged_access(&transaction, import);
    drop(bodies);
    transaction.rollback().unwrap();
    assert_eq!(destination.assets().unwrap(), Vec::new());
    assert_checkpoint_round_trip(source, import);
}

fn stage_section(
    bodies: &mut super::super::Bodies<'_, '_>,
    section: postproject_protocol::CheckpointSection,
    chunks: Vec<postproject_protocol::CheckpointChunk>,
) -> u64 {
    use postproject_protocol::{CheckpointSection, FrameDecoder, Limits};
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut starts = 0;
    for chunk in chunks {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            if let Some(document) = document {
                match section {
                    CheckpointSection::Locators => bodies.locator(&document).unwrap(),
                    CheckpointSection::Identifiers => bodies.identifier(&document).unwrap(),
                    CheckpointSection::Assets => bodies.asset(&document).unwrap(),
                    CheckpointSection::Resources => bodies.resource(&document).unwrap(),
                    CheckpointSection::Representations => {
                        bodies.representation(&document).unwrap();
                    }
                    CheckpointSection::Structures => {
                        starts += u64::from(bodies.structure(&document).unwrap());
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    decoder.finish().unwrap();
    starts
}

fn assert_staged_access(connection: &rusqlite::Connection, import: &OriginalMediaImport) {
    for expected in import.locators() {
        let stored = connection.query_row("SELECT l.id, l.uri, l.last_seen_micros, l.availability, l.media_root_name, n.prefix, n.suffix, n.padding FROM locators l LEFT JOIN locator_sequence_namings n ON n.locator_id = l.id WHERE l.id = ?1", [expected.id().as_bytes().as_slice()], crate::StoredLocator::read).unwrap();
        assert_eq!(
            stored.into_locator(expected.resource_id()).unwrap(),
            *expected
        );
    }
    let attachment = connection
        .query_row(
            "SELECT scheme, value, qualifier FROM external_identifiers",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        attachment,
        (
            "unknown:CASE".into(),
            "Exact 名".into(),
            Some("Qual".into())
        )
    );
}

fn attachment() -> ExternalIdentifier {
    ExternalIdentifier::new(
        IdentifierScheme::new("unknown:CASE").unwrap(),
        "Exact 名",
        Some("Qual".into()),
    )
    .unwrap()
}
