//! Original import framing retains assigned asset facts and complete media.

use postproject_core::{
    Asset, AssetId, ContentStructure, Locator, LocatorAvailability, LocatorId, OriginalMediaImport,
    Representation, RepresentationId, RepresentationKind, Resource, ResourceId, Timestamp,
};
use postproject_protocol::{
    CreationDecoder, Document, FailureKind, Limits, RepresentationCreationStart,
    decode_original_creation_start, encode_original_creation,
};

#[test]
fn original_asset_and_complete_representation_share_the_native_ownership() {
    let asset = Asset::new(
        AssetId::new(),
        Timestamp::from_unix_micros(-7),
        Some("Exact 名".into()),
        Some("unknown:ORIGIN".into()),
    );
    let resource = ResourceId::new();
    let import = OriginalMediaImport::new(
        asset.clone(),
        Representation::new(
            RepresentationId::new(),
            asset.id(),
            RepresentationKind::Original,
            ContentStructure::single_resource(resource),
            Vec::new(),
        ),
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "unknown:LOCATOR",
                None,
                LocatorAvailability::Unknown,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let mut frames = encode_original_creation(&import, 3).unwrap();
    let document = frames.next().unwrap().unwrap();
    assert_eq!(document.kind().unwrap(), "original.creation");
    assert_eq!(decode_original_creation_start(&document).unwrap(), asset);
    let start =
        RepresentationCreationStart::from_document(&frames.next().unwrap().unwrap()).unwrap();
    assert_eq!(start.representation().asset_id(), asset.id());
    assert_eq!(start.representation().kind(), RepresentationKind::Original);
    let mut decoder = CreationDecoder::new(start, 3).unwrap();
    assert!(!decoder.is_complete());
    for frame in frames {
        assert!(!decoder.is_complete());
        decoder.push(&frame.unwrap()).unwrap();
    }
    assert!(decoder.is_complete());
    decoder.finish().unwrap();
    assert!(encode_original_creation(&import, 0).is_err());
}

#[test]
fn original_start_and_dispatch_reject_malformed_or_unknown_fields() {
    for (bytes, kind) in [
        (
            &br#"{"kind":"original.creation","asset":{},"extra":null}"#[..],
            FailureKind::Malformed,
        ),
        (
            &br#"{"kind":"future.creation","asset":{}}"#[..],
            FailureKind::Unsupported,
        ),
    ] {
        let document = Document::parse(bytes, Limits::default()).unwrap();
        assert_eq!(
            decode_original_creation_start(&document)
                .unwrap_err()
                .kind(),
            kind
        );
    }
    for bytes in [&b"{}"[..], &br#"{"kind":null}"#[..]] {
        assert!(
            Document::parse(bytes, Limits::default())
                .unwrap()
                .kind()
                .is_err()
        );
    }
}
