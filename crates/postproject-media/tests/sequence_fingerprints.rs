//! Sampled image-sequence fingerprint integration tests.

use std::fs;

use postproject_core::{
    FrameRange, ImageSequenceDescriptor, RationalRate, ResourceId, SequenceNaming,
};
use postproject_media::{
    SEQUENCE_FINGERPRINT_ALGORITHM, SEQUENCE_FINGERPRINT_VERSION, fingerprint_image_sequence,
};

fn descriptor(missing: Vec<i64>) -> ImageSequenceDescriptor {
    ImageSequenceDescriptor::new(
        ResourceId::new(),
        FrameRange::new(1, 5, 1).expect("valid range"),
        RationalRate::new(24, 1).expect("valid rate"),
        missing,
    )
    .expect("valid descriptor")
}

fn naming(prefix: &str) -> SequenceNaming {
    SequenceNaming::new(prefix, ".exr", 4).expect("valid naming")
}

fn write_frames(directory: &std::path::Path, prefix: &str) {
    for frame in 1..=5 {
        fs::write(
            directory.join(format!("{prefix}{frame:04}.exr")),
            format!("frame {frame}"),
        )
        .expect("write frame");
    }
}

#[test]
fn sample_coverage_is_explicit_and_content_sensitive() {
    let directory = tempfile::tempdir().expect("create directory");
    write_frames(directory.path(), "plate.");
    let descriptor = descriptor(Vec::new());

    let initial = fingerprint_image_sequence(directory.path(), &naming("plate."), &descriptor)
        .expect("fingerprint sequence");
    assert_eq!(initial.sampled_frames(), &[1, 3, 5]);
    assert_eq!(
        initial.fingerprint().algorithm(),
        SEQUENCE_FINGERPRINT_ALGORITHM
    );
    assert_eq!(initial.fingerprint().version(), 2);
    assert_eq!(
        initial.fingerprint().version(),
        SEQUENCE_FINGERPRINT_VERSION
    );

    fs::write(directory.path().join("plate.0003.exr"), b"changed").expect("change sampled frame");
    let changed = fingerprint_image_sequence(directory.path(), &naming("plate."), &descriptor)
        .expect("fingerprint changed sequence");
    assert_ne!(initial.fingerprint(), changed.fingerprint());
}

#[test]
fn recorded_gaps_are_excluded_before_sampling() {
    let directory = tempfile::tempdir().expect("create directory");
    write_frames(directory.path(), "plate.");

    let report =
        fingerprint_image_sequence(directory.path(), &naming("plate."), &descriptor(vec![3]))
            .expect("fingerprint sparse sequence");

    assert_eq!(report.sampled_frames(), &[1, 2, 5]);
}

#[test]
fn relocation_does_not_change_collection_identity() {
    let first = tempfile::tempdir().expect("create first directory");
    let second = tempfile::tempdir().expect("create second directory");
    write_frames(first.path(), "plate.");
    write_frames(second.path(), "plate.");
    let descriptor = descriptor(Vec::new());

    let first = fingerprint_image_sequence(first.path(), &naming("plate."), &descriptor)
        .expect("fingerprint first");
    let second = fingerprint_image_sequence(second.path(), &naming("plate."), &descriptor)
        .expect("fingerprint second");

    assert_eq!(first.fingerprint(), second.fingerprint());
}

#[test]
fn renaming_does_not_change_collection_identity() {
    let first = tempfile::tempdir().expect("create first directory");
    let renamed = tempfile::tempdir().expect("create second directory");
    write_frames(first.path(), "plate.");
    write_frames(renamed.path(), "plate-graded_");
    let descriptor = descriptor(Vec::new());

    let first = fingerprint_image_sequence(first.path(), &naming("plate."), &descriptor)
        .expect("fingerprint first");
    let renamed = fingerprint_image_sequence(renamed.path(), &naming("plate-graded_"), &descriptor)
        .expect("fingerprint renamed");

    assert_eq!(first.fingerprint(), renamed.fingerprint());
}
