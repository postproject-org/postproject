//! Exact root/locator changes retain their historical observation and guard.

use postproject_core::{
    FileFacts, Locator, LocatorAvailability, LocatorId, MediaRoot, MediaRootId, ResourceId,
    SequenceNaming, Timestamp,
};
use postproject_protocol::{Document, FailureKind, Limits, MediaChange};

#[test]
fn every_change_retains_exact_facts_observation_and_semantic_key() {
    let root = MediaRoot::new(
        MediaRootId::new(),
        "source:名",
        Some("Exact".into()),
        Some("file:///legacy/path".into()),
        i32::MIN,
        false,
    )
    .unwrap();
    let locator = Locator::new(
        LocatorId::new(),
        ResourceId::new(),
        "unknown:EXACT",
        Some(Timestamp::from_unix_micros(i64::MIN)),
        LocatorAvailability::Offline,
    )
    .unwrap()
    .with_sequence_naming(SequenceNaming::new("shot.", ".exr", 4).unwrap());
    let changes = [
        MediaChange::ResourceFileFacts {
            resource_id: locator.resource_id(),
            facts: FileFacts::new(u64::MAX, Some(Timestamp::from_unix_micros(i64::MIN))),
        },
        MediaChange::RootAdded(root.clone()),
        MediaChange::RootEnabled {
            root_id: root.id(),
            enabled: true,
        },
        MediaChange::RootRemoved(root.id()),
        MediaChange::RootRemovedWithFacts(root.clone()),
        MediaChange::LocatorAdded(locator.clone()),
        MediaChange::LocatorRetired {
            locator_id: locator.id(),
            resource_id: locator.resource_id(),
        },
    ];
    for change in changes {
        let bytes = change.document().unwrap().canonical_bytes().unwrap();
        let decoded =
            MediaChange::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap();
        assert_eq!(decoded, change);
        assert_eq!(decoded.observation(), change.observation());
        assert_eq!(decoded.conflict_key(), change.conflict_key());
        let altered = String::from_utf8(bytes)
            .unwrap()
            .replacen('{', "{\"unexpected\":null,", 1);
        assert_eq!(
            MediaChange::from_document(
                &Document::parse(altered.as_bytes(), Limits::default()).unwrap()
            )
            .unwrap_err()
            .kind(),
            FailureKind::Malformed
        );
    }
}

#[test]
fn unknown_kinds_nonboolean_state_and_noncanonical_ids_reject() {
    for (bytes, kind) in [
        (&br#"{"kind":"future.change"}"#[..], FailureKind::Unsupported),
        (&br#"{"kind":"root.enabled","id":"00000000-0000-0000-0000-000000000001","enabled":"true"}"#[..], FailureKind::Malformed),
        (&br#"{"kind":"root.removed","id":"00000000-0000-0000-0000-00000000000A"}"#[..], FailureKind::Malformed),
    ] {
        assert_eq!(MediaChange::from_document(&Document::parse(bytes, Limits::default()).unwrap()).unwrap_err().kind(), kind);
    }
}
