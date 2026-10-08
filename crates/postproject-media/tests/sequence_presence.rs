//! Presence checks preserve compact ranges and refuse incomplete inventories.

use std::fs;

use postproject_core::{
    ContentStructure, ErrorKind, FrameRange, ImageSequenceDescriptor, Locator, LocatorAvailability,
    LocatorId, RationalRate, Resource, ResourceId, ResourceResolutionState, SequenceNaming,
};
use postproject_media::{MediaResolver, ResolverOptions, canonical_file_uri};

fn knowledge(
    directory: &std::path::Path,
    start: i64,
    end: i64,
) -> (Resource, ContentStructure, Locator, SequenceNaming) {
    let id = ResourceId::new();
    let naming = SequenceNaming::new("frame.", ".exr", 4).expect("valid naming");
    let descriptor = ImageSequenceDescriptor::new(
        id,
        FrameRange::new(start, end, 1).expect("valid frames"),
        RationalRate::new(24, 1).expect("valid rate"),
        Vec::new(),
    )
    .expect("valid descriptor");
    let locator = Locator::new(
        LocatorId::new(),
        id,
        canonical_file_uri(directory).expect("directory URI"),
        None,
        LocatorAvailability::Unknown,
    )
    .expect("valid locator")
    .with_sequence_naming(naming.clone());
    (
        Resource::new(id, Vec::new(), None),
        ContentStructure::image_sequence(descriptor),
        locator,
        naming,
    )
}

#[test]
fn full_integer_frame_domain_does_not_require_probing_every_declared_frame() {
    let directory = tempfile::tempdir().expect("create directory");
    let (resource, structure, locator, naming) = knowledge(directory.path(), i64::MIN, i64::MAX);
    let resolver = MediaResolver::default();
    let offline = resolver
        .resolve_resource(
            &resource,
            &structure,
            std::slice::from_ref(&locator),
            &[],
            &[],
        )
        .expect("empty known directory remains ordinary offline knowledge");
    assert_eq!(offline.state(), ResourceResolutionState::Offline);
    fs::write(
        directory.path().join(naming.filename(i64::MIN)),
        b"one frame",
    )
    .expect("write one declared frame");
    let error = resolver
        .resolve_resource(&resource, &structure, &[locator], &[], &[])
        .expect_err("missing-frame count exceeds bounded observations");
    assert_eq!(error.kind(), ErrorKind::Unsupported);
}

#[test]
fn incomplete_directory_inventory_cannot_fabricate_missing_frame_evidence() {
    let directory = tempfile::tempdir().expect("create directory");
    let (resource, structure, locator, naming) = knowledge(directory.path(), 1, 2);
    for frame in 1..=2 {
        fs::write(directory.path().join(naming.filename(frame)), b"frame").expect("write frame");
    }
    fs::write(directory.path().join("unrelated.txt"), b"extra").expect("write unrelated entry");
    let resolver = MediaResolver::new(ResolverOptions {
        max_entries_per_directory: 2,
        ..ResolverOptions::default()
    })
    .expect("valid small budget");
    let error = resolver
        .resolve_resource(
            &resource,
            &structure,
            std::slice::from_ref(&locator),
            &[],
            &[],
        )
        .expect_err("complete known sequence cannot be classified from partial listing");
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    let complete = MediaResolver::default()
        .resolve_resource(&resource, &structure, &[locator], &[], &[])
        .expect("complete inventory fits default budget");
    assert_eq!(
        complete.state(),
        ResourceResolutionState::OnlineAtKnownLocator
    );
    assert!(complete.missing_frames().is_empty());
}
