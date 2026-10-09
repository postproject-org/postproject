//! Per-location sequence naming and original observations survive transport.

use postproject_core::{
    Locator, LocatorAvailability, LocatorId, ResourceId, SequenceNaming, Timestamp,
};
use postproject_protocol::{Document, Limits, decode_locator, encode_locator};

#[test]
fn locators_retain_original_observations_and_per_copy_naming_without_io() {
    for (availability, time, naming, root) in [
        (LocatorAvailability::Unknown, None, None, None),
        (
            LocatorAvailability::Offline,
            Some(i64::MIN),
            Some(SequenceNaming::new("未知.", ".exr", 32).unwrap()),
            Some("Rushes 名"),
        ),
        (
            LocatorAvailability::Online,
            Some(i64::MAX),
            Some(SequenceNaming::new("", ".png", 0).unwrap()),
            None,
        ),
    ] {
        let mut locator = Locator::new(
            LocatorId::new(),
            ResourceId::new(),
            "unknown:EXACT%2f-value",
            time.map(Timestamp::from_unix_micros),
            availability,
        )
        .unwrap();
        if let Some(naming) = naming {
            locator = locator.with_sequence_naming(naming);
        }
        if let Some(root) = root {
            locator = locator.with_media_root(root).unwrap();
        }
        let bytes = encode_locator(&locator).unwrap().canonical_bytes().unwrap();
        let restored =
            decode_locator(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap();
        assert_eq!(restored, locator);
        assert_eq!(restored.uri(), "unknown:EXACT%2f-value");
        assert_eq!(restored.last_seen().map(Timestamp::as_unix_micros), time);
    }
}

#[test]
fn malformed_locators_reject_invalid_naming_uris_roots_and_microseconds() {
    let locator = Locator::new(
        LocatorId::new(),
        ResourceId::new(),
        "file:///missing/media",
        None,
        LocatorAvailability::Unknown,
    )
    .unwrap()
    .with_sequence_naming(SequenceNaming::new("shot.", ".exr", 4).unwrap());
    let source: serde_json::Value =
        serde_json::from_slice(&encode_locator(&locator).unwrap().canonical_bytes().unwrap())
            .unwrap();
    for (path, value) in [
        ("/uri", serde_json::json!("relative/path")),
        ("/media_root", serde_json::json!("root/path")),
        ("/availability", serde_json::json!("unknown-future")),
        (
            "/last_seen_micros",
            serde_json::json!("9223372036854775808"),
        ),
        ("/last_seen_micros", serde_json::json!("-0")),
        ("/sequence_naming/padding", serde_json::json!("33")),
        ("/sequence_naming/prefix", serde_json::json!("path/name")),
        ("/sequence_naming/suffix", serde_json::json!("\u{0}")),
    ] {
        let mut fields = source.clone();
        *fields.pointer_mut(path).unwrap() = value;
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| decode_locator(&document))
                .is_err()
        );
    }
    let mut fields = source;
    fields["verified_now"] = true.into();
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| decode_locator(&document))
            .is_err()
    );
}
