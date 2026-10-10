//! Missing and empty checkpoint sections have distinct completeness semantics.

use postproject_protocol::{CheckpointSection, ChunkSummary, Digest, SectionSummary};

#[test]
fn every_section_has_a_unique_stable_identity_and_explicit_emptiness() {
    let mut names = std::collections::BTreeSet::new();
    let chunk = ChunkSummary::new(1, 100, Digest::from_bytes([7; 32])).unwrap();
    for section in CheckpointSection::ALL {
        assert!(names.insert(section.as_str()));
        let empty = SectionSummary::new(section, 0, None);
        if matches!(
            section,
            CheckpointSection::Production | CheckpointSection::ConflictFloor
        ) {
            assert!(empty.is_err());
        } else {
            let empty = empty.unwrap();
            assert_eq!(empty.section(), section);
            assert_eq!(empty.items(), 0);
            assert_eq!(empty.chunks(), None);
        }
        let summary = SectionSummary::new(section, 1, Some(chunk)).unwrap();
        assert_eq!(summary.section(), section);
        assert_eq!(summary.items(), 1);
        assert_eq!(summary.chunks(), Some(chunk));
    }
    assert_eq!(names.len(), 19);
    assert!(names.contains("archives"));
}

#[test]
fn contradictory_or_impossible_section_totals_reject() {
    let chunk = ChunkSummary::new(1, 10, Digest::from_bytes([7; 32])).unwrap();
    assert!(SectionSummary::new(CheckpointSection::Metadata, 0, Some(chunk)).is_err());
    assert!(SectionSummary::new(CheckpointSection::Metadata, 1, None).is_err());
    assert!(SectionSummary::new(CheckpointSection::Metadata, 2, Some(chunk)).is_err());
    assert!(SectionSummary::new(CheckpointSection::Metadata, u64::MAX, Some(chunk)).is_err());
    assert!(SectionSummary::new(CheckpointSection::Production, 2, Some(chunk)).is_err());
}
