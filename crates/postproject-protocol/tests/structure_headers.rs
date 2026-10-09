//! Compact headers reject impossible domain facts before continuation allocation.

use postproject_core::{
    AssetId, ContentStructure, FrameRange, ImageSequenceDescriptor, RationalRate, Representation,
    RepresentationId, RepresentationKind, ResourceId, ResourceMember, ResourceRole,
};
use postproject_protocol::{Document, FailureKind, Limits, StructureHeader};

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
fn all_shapes_have_bounded_headers_with_exact_continuation_totals() {
    let member = ResourceMember::new(
        ResourceId::new(),
        ResourceRole::new("unknown:essence").unwrap(),
        true,
    );
    let contents = [
        ContentStructure::single_resource(ResourceId::new()),
        ContentStructure::image_sequence(
            ImageSequenceDescriptor::new(
                ResourceId::new(),
                FrameRange::new(i64::MIN, i64::MAX, 1).unwrap(),
                RationalRate::new(u32::MAX, 1).unwrap(),
                vec![i64::MIN, i64::MAX],
            )
            .unwrap(),
        ),
        ContentStructure::ordered_parts(vec![member.clone()]).unwrap(),
        ContentStructure::package(vec![member]).unwrap(),
    ];
    for content in contents {
        let representation = representation(content);
        let header = StructureHeader::from_structure(
            representation.id(),
            representation.content_structure(),
        )
        .unwrap();
        assert_eq!(
            header,
            StructureHeader::from_representation(&representation).unwrap()
        );
        let bytes = header.document().canonical_bytes().unwrap();
        assert!(bytes.len() < 600);
        let restored =
            StructureHeader::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap();
        assert_eq!(restored, header);
        assert_eq!(restored.representation_id(), representation.id());
        assert_eq!(restored.kind(), representation.content_structure().kind());
        assert_eq!(
            restored.member_count(),
            representation
                .content_structure()
                .members()
                .map_or(0, <[_]>::len)
        );
        assert_eq!(
            restored.exception_count(),
            representation
                .content_structure()
                .image_sequence_descriptor()
                .map_or(0, |sequence| sequence.known_missing_frames().len())
        );
    }
}

#[test]
fn invalid_scalar_domains_and_oversized_continuations_reject_before_allocation() {
    let source = representation(ContentStructure::image_sequence(
        ImageSequenceDescriptor::new(
            ResourceId::new(),
            FrameRange::new(-10, 10, 2).unwrap(),
            RationalRate::new(24, 1).unwrap(),
            Vec::new(),
        )
        .unwrap(),
    ));
    let fields: serde_json::Value = serde_json::from_slice(
        &StructureHeader::from_representation(&source)
            .unwrap()
            .document()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    for (path, value, kind) in [
        ("/content/frames/step", "0", FailureKind::Malformed),
        ("/content/frames/end", "9", FailureKind::Malformed),
        ("/content/rate/denominator", "0", FailureKind::Malformed),
        (
            "/content/exception_count",
            "100001",
            FailureKind::LimitExceeded,
        ),
        (
            "/content/exception_count",
            "18446744073709551615",
            FailureKind::LimitExceeded,
        ),
        ("/content/frames/start", "-01", FailureKind::Malformed),
        ("/content/shape", "unknown", FailureKind::Unsupported),
    ] {
        let mut invalid = fields.clone();
        *invalid.pointer_mut(path).unwrap() = value.into();
        let error = Document::parse(&serde_json::to_vec(&invalid).unwrap(), Limits::default())
            .and_then(|document| StructureHeader::from_document(&document))
            .unwrap_err();
        assert_eq!(error.kind(), kind);
    }
    let mut invalid = fields;
    invalid["content"]["known_missing_frames"] = serde_json::json!([]);
    assert!(
        Document::parse(&serde_json::to_vec(&invalid).unwrap(), Limits::default())
            .and_then(|document| StructureHeader::from_document(&document))
            .is_err()
    );
}
