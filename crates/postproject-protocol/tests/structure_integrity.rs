//! Aggregate semantic checks survive valid framing and canonical re-encoding.

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

fn changed(document: &Document, key: &str, value: serde_json::Value) -> Document {
    let mut fields: serde_json::Value =
        serde_json::from_slice(&document.canonical_bytes().unwrap()).unwrap();
    fields[key] = value;
    Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap()
}

#[test]
fn packages_reject_duplicate_resources_wrong_owners_and_optional_only_members() {
    let source = representation(
        ContentStructure::package(vec![
            ResourceMember::new(
                ResourceId::new(),
                ResourceRole::new("test:essence").unwrap(),
                true,
            ),
            ResourceMember::new(
                ResourceId::new(),
                ResourceRole::new("test:sidecar").unwrap(),
                false,
            ),
        ])
        .unwrap(),
    );
    let documents: Vec<_> = encode_structure(&source)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let header = StructureHeader::from_document(&documents[0]).unwrap();
    let resource =
        serde_json::from_slice::<serde_json::Value>(&documents[1].canonical_bytes().unwrap())
            .unwrap()["resource_id"]
            .clone();
    let invalids = [
        changed(&documents[2], "resource_id", resource),
        changed(
            &documents[2],
            "representation_id",
            RepresentationId::new().to_string().into(),
        ),
        changed(&documents[2], "position", "0".into()),
    ];
    for invalid in invalids {
        let mut assembler = StructureAssembler::new(header);
        assembler.push(&documents[1]).unwrap();
        assert!(assembler.push(&invalid).is_err());
        assert!(assembler.finish().is_err());
    }
    let mut optional_only = StructureAssembler::new(header);
    optional_only
        .push(&changed(&documents[1], "required", false.into()))
        .unwrap();
    optional_only.push(&documents[2]).unwrap();
    assert!(optional_only.finish().is_err());
    let mut fields: serde_json::Value =
        serde_json::from_slice(&documents[0].canonical_bytes().unwrap()).unwrap();
    fields["content"]["shape"] = "ordered_parts".into();
    let header = StructureHeader::from_document(
        &Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap(),
    )
    .unwrap();
    let mut ordered = StructureAssembler::new(header);
    ordered.push(&documents[1]).unwrap();
    assert!(ordered.push(&documents[2]).is_err());
    assert!(ordered.finish().is_err());
}

#[test]
fn sequences_reject_sortedness_duplicates_out_of_domain_and_wrong_frame_kinds() {
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
    for frame in [-4, -2, 1, 6] {
        let mut assembler = StructureAssembler::new(header);
        assembler.push(&documents[1]).unwrap();
        assert!(
            assembler
                .push(&changed(&documents[2], "frame", frame.to_string().into()))
                .is_err()
        );
        assert!(assembler.finish().is_err());
    }
    let mut assembler = StructureAssembler::new(header);
    assert!(
        assembler
            .push(&changed(
                &documents[1],
                "representation_id",
                RepresentationId::new().to_string().into()
            ))
            .is_err()
    );
    let mut assembler = StructureAssembler::new(header);
    assert!(assembler.push(&documents[0]).is_err());
    assert!(assembler.finish().is_err());
    let single = representation(ContentStructure::single_resource(ResourceId::new()));
    let mut assembler =
        StructureAssembler::new(StructureHeader::from_representation(&single).unwrap());
    assert!(assembler.push(&documents[1]).is_err());
    assert!(assembler.finish().is_err());
}
