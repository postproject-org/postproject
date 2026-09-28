//! Compound-media recognition coverage.

use std::{fs, path::Path};

use postproject_core::{ContentStructureKind, RationalRate};
use postproject_media::{
    AVCHD_CLIP_INFO_ROLE, AVCHD_ESSENCE_ROLE, AVCHD_NAVIGATION_ROLE, AVCHD_PLAYLIST_ROLE,
    MediaRecognizer, RecognizedMedia, SPAN_PART_ROLE, prepare_recognized_original_media,
};

fn recognizer() -> MediaRecognizer {
    MediaRecognizer::new(RationalRate::new(24, 1).expect("valid rate"))
}

#[test]
fn recognizes_sparse_numbered_image_sequences() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    for name in ["plate.1001.exr", "plate.1002.exr", "plate.1004.exr"] {
        fs::write(temporary.path().join(name), name).expect("write frame");
    }

    let recognized = recognizer()
        .recognize(temporary.path())
        .expect("recognize directory");
    let [
        RecognizedMedia::ImageSequence {
            naming,
            frames,
            missing_frames,
            ..
        },
    ] = recognized.as_slice()
    else {
        panic!("expected one image sequence");
    };
    assert_eq!(naming.prefix(), "plate.");
    assert_eq!(naming.suffix(), ".exr");
    assert_eq!(naming.padding(), 4);
    assert_eq!((frames.start(), frames.end()), (1001, 1004));
    assert_eq!(missing_frames, &[1003]);

    let prepared = prepare_recognized_original_media(&recognized[0], None, None)
        .expect("prepare recognized sequence");
    assert_eq!(
        prepared.representation().content_structure().kind(),
        ContentStructureKind::ImageSequence
    );
}

#[test]
fn recognizes_ordered_recording_spans() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    for name in ["interview_001.mxf", "interview_002.mxf"] {
        fs::write(temporary.path().join(name), name).expect("write span");
    }

    let recognized = recognizer()
        .recognize(temporary.path())
        .expect("recognize spans");
    let [RecognizedMedia::OrderedParts(members)] = recognized.as_slice() else {
        panic!("expected ordered parts");
    };
    assert_eq!(members.len(), 2);
    assert!(members.iter().all(|member| member.role() == SPAN_PART_ROLE));
    assert!(
        members
            .iter()
            .all(postproject_media::RecognizedMember::is_required)
    );
}

#[test]
fn recognizes_checked_in_avchd_package_roles() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/avchd-card");
    let recognized = recognizer().recognize(&fixture).expect("recognize AVCHD");
    let [RecognizedMedia::Package(members)] = recognized.as_slice() else {
        panic!("expected AVCHD package");
    };
    let roles = members
        .iter()
        .map(postproject_media::RecognizedMember::role)
        .collect::<Vec<_>>();
    assert!(roles.contains(&AVCHD_ESSENCE_ROLE));
    assert!(roles.contains(&AVCHD_CLIP_INFO_ROLE));
    assert!(roles.contains(&AVCHD_PLAYLIST_ROLE));
    assert!(roles.contains(&AVCHD_NAVIGATION_ROLE));
    assert!(
        members
            .iter()
            .any(postproject_media::RecognizedMember::is_required)
    );

    let prepared = prepare_recognized_original_media(&recognized[0], None, None)
        .expect("prepare recognized package");
    assert_eq!(
        prepared.representation().content_structure().kind(),
        ContentStructureKind::Package
    );
    assert_eq!(prepared.resources().len(), 4);
}

#[test]
fn returns_each_unrelated_numbered_group_without_guessing() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    for name in [
        "beauty.0001.dpx",
        "beauty.0002.dpx",
        "matte.0001.tiff",
        "matte.0002.tiff",
    ] {
        fs::write(temporary.path().join(name), name).expect("write frame");
    }

    let recognized = recognizer()
        .recognize(temporary.path())
        .expect("recognize groups");
    assert_eq!(recognized.len(), 2);
    assert!(
        recognized
            .iter()
            .all(|candidate| matches!(candidate, RecognizedMedia::ImageSequence { .. }))
    );
}
