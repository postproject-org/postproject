//! Declared media totals are checked before receiving aggregate collections.

use postproject_core::{
    AssetId, ContentStructure, Locator, LocatorAvailability, LocatorId, Representation,
    RepresentationId, RepresentationImport, RepresentationKind, Resource, ResourceFingerprint,
    ResourceId,
};
use postproject_protocol::{Document, Limits, RepresentationCreationStart, ResourceCreationStart};

#[test]
fn creation_headers_retain_ownership_and_exact_bounded_collection_totals() {
    let resource_id = ResourceId::new();
    let resource = Resource::new(
        resource_id,
        vec![ResourceFingerprint::new("unknown", 0, vec![1]).unwrap()],
        None,
    );
    let import = RepresentationImport::new(
        Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Proxy,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        ),
        vec![resource.clone()],
        vec![
            Locator::new(
                LocatorId::new(),
                resource_id,
                "unknown:location",
                None,
                LocatorAvailability::Unknown,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let header = RepresentationCreationStart::from_import(&import).unwrap();
    let restored = RepresentationCreationStart::from_document(
        &Document::parse(
            &header.document().unwrap().canonical_bytes().unwrap(),
            Limits::default(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(restored, header);
    assert_eq!(restored.resource_count(), 1);
    assert_eq!(restored.locator_count(), 1);
    assert_eq!(restored.fingerprint_count(), 0);
    let resource = ResourceCreationStart::from_resource(&resource).unwrap();
    assert_eq!(
        ResourceCreationStart::from_document(
            &Document::parse(
                &resource.document().canonical_bytes().unwrap(),
                Limits::default()
            )
            .unwrap()
        )
        .unwrap(),
        resource
    );
    assert_eq!(resource.fingerprint_count(), 1);
}

#[test]
fn forged_creation_headers_reject_impossible_or_excessive_totals() {
    let resource_id = ResourceId::new();
    let import = RepresentationImport::new(
        Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Original,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        ),
        vec![Resource::new(resource_id, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource_id,
                "unknown:location",
                None,
                LocatorAvailability::Unknown,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let source: serde_json::Value = serde_json::from_slice(
        &RepresentationCreationStart::from_import(&import)
            .unwrap()
            .document()
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    for (key, value) in [
        ("resource_count", "0"),
        ("resource_count", "100001"),
        ("locator_count", "0"),
        ("locator_count", "18446744073709551615"),
        ("fingerprint_count", "18446744073709551615"),
        ("fingerprint_count", "01"),
    ] {
        let mut fields = source.clone();
        fields[key] = value.into();
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| RepresentationCreationStart::from_document(&document))
                .is_err()
        );
    }
    let mut fields = source;
    fields["resources"] = serde_json::json!([]);
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| RepresentationCreationStart::from_document(&document))
            .is_err()
    );
}
