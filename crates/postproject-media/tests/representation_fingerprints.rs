//! Structure-aware representation-fingerprint integration tests.

use postproject_core::{
    ContentStructure, FrameRange, ImageSequenceDescriptor, RationalRate, Resource,
    ResourceFingerprint, ResourceId, ResourceMember, ResourceRole,
};
use postproject_media::{
    REPRESENTATION_FINGERPRINT_ALGORITHM, REPRESENTATION_FINGERPRINT_VERSION,
    fingerprint_representation,
};

fn resource(id: u8, value: u8) -> Resource {
    Resource::new(
        ResourceId::from_bytes([id; 16]),
        vec![ResourceFingerprint::new("fixture", 1, vec![value]).expect("valid fingerprint")],
        None,
    )
}

fn member(resource: &Resource) -> ResourceMember {
    ResourceMember::new(
        resource.id(),
        ResourceRole::new("org.postproject:essence").expect("valid role"),
        true,
    )
}

#[test]
fn single_resource_fingerprint_is_content_based() {
    let first = resource(1, 7);
    let second = resource(2, 7);

    let first_fingerprint = fingerprint_representation(
        &ContentStructure::single_resource(first.id()),
        std::slice::from_ref(&first),
    )
    .expect("fingerprint first representation");
    let second_fingerprint = fingerprint_representation(
        &ContentStructure::single_resource(second.id()),
        std::slice::from_ref(&second),
    )
    .expect("fingerprint second representation");

    assert_eq!(first_fingerprint, second_fingerprint);
    assert_eq!(
        first_fingerprint.algorithm(),
        REPRESENTATION_FINGERPRINT_ALGORITHM
    );
    assert_eq!(
        first_fingerprint.version(),
        REPRESENTATION_FINGERPRINT_VERSION
    );
}

#[test]
fn ordered_parts_retain_order_but_packages_do_not() {
    let first = resource(1, 1);
    let second = resource(2, 2);
    let resources = [first.clone(), second.clone()];
    let forward = vec![member(&first), member(&second)];
    let reverse = vec![member(&second), member(&first)];

    let forward_ordered = fingerprint_representation(
        &ContentStructure::ordered_parts(forward.clone()).expect("ordered parts"),
        &resources,
    )
    .expect("fingerprint ordered parts");
    let reverse_ordered = fingerprint_representation(
        &ContentStructure::ordered_parts(reverse.clone()).expect("reversed parts"),
        &resources,
    )
    .expect("fingerprint reversed parts");
    assert_ne!(forward_ordered, reverse_ordered);

    let forward_package = fingerprint_representation(
        &ContentStructure::package(forward).expect("package"),
        &resources,
    )
    .expect("fingerprint package");
    let reverse_package = fingerprint_representation(
        &ContentStructure::package(reverse).expect("reversed package"),
        &resources,
    )
    .expect("fingerprint reversed package");
    assert_eq!(forward_package, reverse_package);
}

#[test]
fn image_sequence_uses_its_compact_descriptor() {
    let resource = resource(3, 3);
    let descriptor = |end| {
        ImageSequenceDescriptor::new(
            resource.id(),
            FrameRange::new(1, end, 1).expect("valid frame range"),
            RationalRate::new(24_000, 1_001).expect("valid rate"),
            vec![42],
        )
        .expect("valid descriptor")
    };

    let million_frames = fingerprint_representation(
        &ContentStructure::image_sequence(descriptor(1_000_000)),
        std::slice::from_ref(&resource),
    )
    .expect("fingerprint compact sequence");
    let changed_range = fingerprint_representation(
        &ContentStructure::image_sequence(descriptor(1_000_001)),
        std::slice::from_ref(&resource),
    )
    .expect("fingerprint changed sequence");

    assert_ne!(million_frames, changed_range);
}

#[test]
fn resources_must_exactly_match_and_carry_evidence() {
    let expected = resource(1, 1);
    let extra = resource(2, 2);
    let structure = ContentStructure::single_resource(expected.id());

    assert!(fingerprint_representation(&structure, &[]).is_err());
    assert!(fingerprint_representation(&structure, &[expected.clone(), extra]).is_err());
    assert!(
        fingerprint_representation(
            &structure,
            &[Resource::new(expected.id(), Vec::new(), None)]
        )
        .is_err()
    );
}
