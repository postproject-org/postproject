use postproject_core::{
    Asset, AssetId, ContentStructure, FileFacts, FrameRange, ImageSequenceDescriptor, Locator,
    LocatorAvailability, LocatorId, OriginalMediaImport, RationalRate, Representation,
    RepresentationId, RepresentationKind, Resource, ResourceId, ResourceMember, ResourceRole,
    SequenceNaming, Timestamp,
};

use crate::SqliteProduction;

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
        edit.commit().unwrap();
        drop(edit);
        assert_eq!(super::asset(&source.connection, asset.id()).unwrap(), asset);
        assert_eq!(
            super::representation(&source.connection, representation.id()).unwrap(),
            postproject_protocol::RepresentationHeader::from_representation(&representation)
        );
        assert_eq!(
            super::content(&source.connection, representation.id()).unwrap(),
            *representation.content_structure()
        );
        for resource in import.resources() {
            assert_eq!(
                super::resource(&source.connection, resource.id()).unwrap(),
                postproject_protocol::ResourceHeader::from_resource(resource)
            );
        }
        source.connection.execute("DELETE FROM representation_resources WHERE representation_id = ?1 AND position = 0", [representation.id().as_bytes().as_slice()]).unwrap();
        assert!(super::content(&source.connection, representation.id()).is_err());
    }
}
