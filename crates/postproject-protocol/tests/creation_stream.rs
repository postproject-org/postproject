//! Authored creation evidence comes from the prepared values and original revision.

use postproject_core::{
    AssetId, ContentStructure, FileFacts, Locator, LocatorAvailability, LocatorId, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationImport, RepresentationKind,
    Resource, ResourceFingerprint, ResourceId,
};
use postproject_protocol::{
    FingerprintObservation, FingerprintState, RepresentationCreationStart, ResourceCreationStart,
    StructureHeader, decode_locator, encode_representation_creation,
};

#[test]
fn streamed_creation_preserves_initial_evidence_and_declares_every_member() {
    let resource_id = ResourceId::new();
    let representation = Representation::new(
        RepresentationId::new(),
        AssetId::new(),
        RepresentationKind::Original,
        ContentStructure::single_resource(resource_id),
        vec![RepresentationFingerprint::new("repr-domain", 2, vec![1, 255]).unwrap()],
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource_id,
        "unknown:EXACT",
        None,
        LocatorAvailability::Unknown,
    )
    .unwrap();
    let import = RepresentationImport::new(
        representation,
        vec![Resource::new(
            resource_id,
            vec![ResourceFingerprint::new("resource-domain", 3, vec![255, 0]).unwrap()],
            Some(FileFacts::new(123, None)),
        )],
        vec![locator.clone()],
    )
    .unwrap();
    let documents: Vec<_> = encode_representation_creation(&import, 7)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(documents.len(), 6);
    let header = RepresentationCreationStart::from_document(&documents[0]).unwrap();
    assert_eq!(header.fingerprint_count(), 1);
    let representation_fingerprint = FingerprintObservation::from_document(&documents[1]).unwrap();
    assert_eq!(representation_fingerprint.snapshot().value(), &[1, 255]);
    assert_eq!(
        representation_fingerprint
            .snapshot()
            .observed_revision_sequence(),
        Some(7)
    );
    assert_eq!(
        representation_fingerprint.state(),
        FingerprintState::Current
    );
    assert_eq!(
        StructureHeader::from_document(&documents[2])
            .unwrap()
            .representation_id(),
        import.representation().id()
    );
    let resource = ResourceCreationStart::from_document(&documents[3]).unwrap();
    assert_eq!(resource.resource().id(), resource_id);
    assert_eq!(
        resource.resource().file_facts(),
        Some(FileFacts::new(123, None))
    );
    let resource_fingerprint = FingerprintObservation::from_document(&documents[4]).unwrap();
    assert_eq!(resource_fingerprint.snapshot().value(), &[255, 0]);
    assert_eq!(
        resource_fingerprint.snapshot().observed_revision_sequence(),
        Some(7)
    );
    assert_eq!(decode_locator(&documents[5]).unwrap(), locator);
    assert!(encode_representation_creation(&import, 0).is_err());
}
