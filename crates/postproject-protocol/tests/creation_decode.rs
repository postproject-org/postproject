//! Streaming relationship checks agree with complete native import construction.

use postproject_core::{
    AssetId, ContentStructure, FrameRange, ImageSequenceDescriptor, Locator, LocatorAvailability,
    LocatorId, RationalRate, Representation, RepresentationFingerprint, RepresentationId,
    RepresentationImport, RepresentationKind, Resource, ResourceFingerprint, ResourceId,
    ResourceMember, ResourceRole, SequenceNaming,
};
use postproject_protocol::{
    CreationDecoder, CreationFact, RepresentationCreationStart, encode_representation_creation,
};

#[test]
fn decoded_creation_facts_reconstruct_the_same_checked_native_aggregates() {
    let first = ResourceId::new();
    let second = ResourceId::new();
    let member = |id, required| {
        ResourceMember::new(id, ResourceRole::new("unknown:member").unwrap(), required)
    };
    let contents = [
        ContentStructure::single_resource(first),
        ContentStructure::image_sequence(
            ImageSequenceDescriptor::new(
                first,
                FrameRange::new(-10, 10, 2).unwrap(),
                RationalRate::new(24, 1).unwrap(),
                vec![-2, 4],
            )
            .unwrap(),
        ),
        ContentStructure::ordered_parts(vec![member(second, true), member(first, true)]).unwrap(),
        ContentStructure::package(vec![member(first, true), member(second, false)]).unwrap(),
    ];
    for content in contents {
        let source = fixture(content);
        assert_eq!(restore(&source), source);
    }
}

fn fixture(content: ContentStructure) -> RepresentationImport {
    let resources: Vec<_> = content
        .resource_ids()
        .into_iter()
        .map(|id| {
            Resource::new(
                id,
                vec![ResourceFingerprint::new("resource-domain", 1, vec![0, 255]).unwrap()],
                None,
            )
        })
        .collect();
    let locators = resources
        .iter()
        .map(|resource| {
            let mut locator = Locator::new(
                LocatorId::new(),
                resource.id(),
                format!("unknown:{}", resource.id()),
                None,
                LocatorAvailability::Unknown,
            )
            .unwrap();
            if content.image_sequence_descriptor().is_some() {
                locator =
                    locator.with_sequence_naming(SequenceNaming::new("shot.", ".exr", 4).unwrap());
            }
            locator
        })
        .collect();
    RepresentationImport::new(
        Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Original,
            content,
            vec![RepresentationFingerprint::new("repr-domain", 2, vec![255, 1]).unwrap()],
        ),
        resources,
        locators,
    )
    .unwrap()
}

fn restore(source: &RepresentationImport) -> RepresentationImport {
    let mut documents = encode_representation_creation(source, 7).unwrap();
    let header =
        RepresentationCreationStart::from_document(&documents.next().unwrap().unwrap()).unwrap();
    let mut decoder = CreationDecoder::new(header, 7).unwrap();
    let mut content = None;
    let mut fingerprints = Vec::new();
    let mut resources = Vec::new();
    let mut locators = Vec::new();
    for document in documents {
        match decoder.push(&document.unwrap()).unwrap() {
            Some(CreationFact::RepresentationFingerprint(fact)) => {
                let snapshot = fact.snapshot();
                fingerprints.push(
                    RepresentationFingerprint::new(
                        snapshot.algorithm(),
                        snapshot.version(),
                        snapshot.value().to_vec(),
                    )
                    .unwrap(),
                );
            }
            Some(CreationFact::Structure(structure)) => content = Some(structure),
            Some(CreationFact::Resource(resource)) => resources.push(Resource::new(
                resource.id(),
                Vec::new(),
                resource.file_facts(),
            )),
            Some(CreationFact::ResourceFingerprint(fact)) => {
                let snapshot = fact.snapshot();
                let last = resources.last_mut().unwrap();
                assert_eq!(
                    fact.target(),
                    postproject_core::ObjectRef::Resource(last.id())
                );
                *last = last.with_observed_fingerprint(
                    &ResourceFingerprint::new(
                        snapshot.algorithm(),
                        snapshot.version(),
                        snapshot.value().to_vec(),
                    )
                    .unwrap(),
                );
            }
            Some(CreationFact::Locator(locator)) => locators.push(locator),
            None => {}
        }
    }
    decoder.finish().unwrap();
    let representation = header.representation();
    RepresentationImport::new(
        Representation::new(
            representation.id(),
            representation.asset_id(),
            representation.kind(),
            content.unwrap(),
            fingerprints,
        ),
        resources,
        locators,
    )
    .unwrap()
}
