//! Archives retain exact opaque evidence while rejecting inexact framing.

use postproject_core::RevisionId;
use postproject_protocol::{ArchiveEvidence, ArchiveFamily, Document, FailureKind, Limits};

#[test]
fn exact_archive_bytes_and_coordinates_round_trip_without_interpreting_payloads() {
    let revision = RevisionId::from_bytes([1; 16]);
    for family in [
        ArchiveFamily::Manifest,
        ArchiveFamily::Chunk,
        ArchiveFamily::PartialEffect,
    ] {
        let (position, fragment) = if family == ArchiveFamily::Manifest {
            (0, 0)
        } else {
            (1001, 123)
        };
        let payload = b"\x00\xffnot JSON or executable SQL\x00".to_vec();
        let original = ArchiveEvidence::new(
            family,
            Some(revision),
            42,
            position,
            fragment,
            payload.clone(),
        )
        .unwrap();
        let bytes = original.document().canonical_bytes().unwrap();
        let decoded =
            ArchiveEvidence::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap();
        assert_eq!(decoded, original);
        assert_eq!(decoded.payload(), payload);
    }
    let anchor = ArchiveEvidence::new(ArchiveFamily::Anchor, None, 0, 0, 0, vec![255; 32]).unwrap();
    assert_eq!(
        ArchiveEvidence::from_document(&anchor.document()).unwrap(),
        anchor
    );
    let large = ArchiveEvidence::new(
        ArchiveFamily::PartialEffect,
        Some(revision),
        1,
        0,
        0,
        vec![42; 1_048_576],
    )
    .unwrap();
    assert_eq!(
        ArchiveEvidence::from_document(&large.document()).unwrap(),
        large
    );
}

#[test]
fn unknown_families_inexact_coordinates_and_unbounded_or_noncanonical_bytes_reject() {
    for (document, failure) in [
        (br#"{"kind":"history.archive","family":"sql","revision":null,"sequence":"0","position":"0","fragment":"0","payload":"AA=="}"#.as_slice(), FailureKind::Unsupported),
        (br#"{"kind":"history.archive","family":"chunk","revision":null,"sequence":"0","position":"0","fragment":"0","payload":"AA=="}"#.as_slice(), FailureKind::Malformed),
        (br#"{"kind":"history.archive","family":"chunk","revision":"01010101-0101-0101-0101-010101010101","sequence":"01","position":"0","fragment":"0","payload":"AA=="}"#.as_slice(), FailureKind::Malformed),
        (br#"{"kind":"history.archive","family":"chunk","revision":"01010101-0101-0101-0101-010101010101","sequence":"1","position":"0","fragment":"0","payload":"AA"}"#.as_slice(), FailureKind::Malformed),
        (br#"{"kind":"history.archive","family":"chunk","revision":"01010101-0101-0101-0101-010101010101","sequence":"1","position":"0","fragment":"0","payload":"AB=="}"#.as_slice(), FailureKind::Malformed),
    ] {
        assert_eq!(ArchiveEvidence::from_document(&Document::parse(document, Limits::default()).unwrap()).unwrap_err().kind(), failure);
    }
    for (family, revision, sequence, position, fragment, bytes) in [
        (ArchiveFamily::Anchor, None, 0, 1, 0, vec![0; 32]),
        (ArchiveFamily::Anchor, None, 0, 0, 0, vec![0; 31]),
        (
            ArchiveFamily::Manifest,
            Some(RevisionId::new()),
            1,
            0,
            1,
            vec![0],
        ),
        (
            ArchiveFamily::Chunk,
            Some(RevisionId::new()),
            1,
            u64::MAX,
            0,
            vec![0],
        ),
        (
            ArchiveFamily::PartialEffect,
            Some(RevisionId::new()),
            1,
            0,
            0,
            Vec::new(),
        ),
    ] {
        assert_eq!(
            ArchiveEvidence::new(family, revision, sequence, position, fragment, bytes)
                .unwrap_err()
                .kind(),
            FailureKind::Malformed
        );
    }
    assert_eq!(
        ArchiveEvidence::new(
            ArchiveFamily::Chunk,
            Some(RevisionId::new()),
            1,
            0,
            0,
            vec![0; 1_048_577]
        )
        .unwrap_err()
        .kind(),
        FailureKind::LimitExceeded
    );
}
