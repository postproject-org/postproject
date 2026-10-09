//! Complete streamed aggregates retain order, roles and compact sequence facts.

use postproject_core::{
    AssetId, ContentStructure, FrameRange, ImageSequenceDescriptor, RationalRate, Representation,
    RepresentationId, RepresentationKind, ResourceId, ResourceMember, ResourceRole,
};
use postproject_protocol::{
    Document, Limits, StructureAssembler, StructureHeader, encode_structure,
};

fn representation(content: ContentStructure) -> Representation {
    Representation::new(
        RepresentationId::new(),
        AssetId::new(),
        RepresentationKind::Original,
        content,
        Vec::new(),
    )
}

#[test]
fn streamed_structures_restore_compact_sequences_order_and_optional_sidecars() {
    let required = ResourceMember::new(
        ResourceId::new(),
        ResourceRole::new("test:essence").unwrap(),
        true,
    );
    let optional = ResourceMember::new(
        ResourceId::new(),
        ResourceRole::new("vendor:unknown-sidecar").unwrap(),
        false,
    );
    let contents = [
        ContentStructure::single_resource(ResourceId::new()),
        ContentStructure::image_sequence(
            ImageSequenceDescriptor::new(
                ResourceId::new(),
                FrameRange::new(i64::MIN, i64::MAX, 1).unwrap(),
                RationalRate::new(30_000, 1001).unwrap(),
                vec![i64::MIN, -2, i64::MAX],
            )
            .unwrap(),
        ),
        ContentStructure::ordered_parts(vec![
            required.clone(),
            ResourceMember::new(optional.resource_id(), optional.role().clone(), true),
        ])
        .unwrap(),
        ContentStructure::package(vec![optional, required]).unwrap(),
    ];
    for content in contents {
        let representation = representation(content);
        let mut documents = encode_structure(&representation).unwrap();
        let header = StructureHeader::from_document(&documents.next().unwrap().unwrap()).unwrap();
        let mut assembler = StructureAssembler::new(header);
        let mut count = 1;
        for document in documents {
            let bytes = document.unwrap().canonical_bytes().unwrap();
            assert!(bytes.len() < 600);
            assembler
                .push(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap();
            count += 1;
        }
        assert_eq!(count, 1 + header.member_count() + header.exception_count());
        assert_eq!(
            assembler.finish().unwrap(),
            *representation.content_structure()
        );
    }
}

#[test]
fn incomplete_or_failed_aggregates_never_return_a_partial_structure() {
    let source = representation(ContentStructure::image_sequence(
        ImageSequenceDescriptor::new(
            ResourceId::new(),
            FrameRange::new(-4, 4, 2).unwrap(),
            RationalRate::new(24, 1).unwrap(),
            vec![-2, 2],
        )
        .unwrap(),
    ));
    let documents: Vec<_> = encode_structure(&source)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let header = StructureHeader::from_document(&documents[0]).unwrap();
    let mut incomplete = StructureAssembler::new(header);
    incomplete.push(&documents[1]).unwrap();
    assert!(incomplete.finish().is_err());
    let mut failed = StructureAssembler::new(header);
    assert!(failed.push(&documents[2]).is_err());
    assert!(failed.push(&documents[1]).is_err());
    assert!(failed.finish().is_err());
    let mut surplus = StructureAssembler::new(header);
    surplus.push(&documents[1]).unwrap();
    surplus.push(&documents[2]).unwrap();
    assert!(surplus.push(&documents[2]).is_err());
    assert!(surplus.finish().is_err());
}
