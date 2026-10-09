//! Configured roots retain exact portable facts and checked domain boundaries.

use postproject_core::{MediaRoot, MediaRootId};
use postproject_protocol::{Document, Limits, decode_root, encode_root};

#[test]
fn root_facts_retain_configuration_and_distinguish_absent_from_empty_labels() {
    for (priority, enabled, label, legacy) in [
        (i32::MIN, false, None, None),
        (
            i32::MAX,
            true,
            Some(String::new()),
            Some("file:///legacy/a%20b".into()),
        ),
        (
            0,
            false,
            Some("Name 名\n\0".into()),
            Some("unknown:exact-value".into()),
        ),
    ] {
        let root = MediaRoot::new(
            MediaRootId::new(),
            "Rushes 名",
            label,
            legacy,
            priority,
            enabled,
        )
        .unwrap();
        let document = encode_root(&root);
        assert_eq!(
            decode_root(
                &Document::parse(&document.canonical_bytes().unwrap(), Limits::default()).unwrap()
            )
            .unwrap(),
            root
        );
        let value: serde_json::Value =
            serde_json::from_slice(&document.canonical_bytes().unwrap()).unwrap();
        assert_eq!(value["priority"].as_str().unwrap(), priority.to_string());
        assert_eq!(value["enabled"].as_bool(), Some(enabled));
        assert!(value.get("directory_mapping").is_none());
    }
}

#[test]
fn malformed_roots_reject_local_mapping_fields_and_invalid_checked_values() {
    let root = MediaRoot::new(MediaRootId::new(), "Rushes", None, None, 0, true).unwrap();
    let source: serde_json::Value =
        serde_json::from_slice(&encode_root(&root).canonical_bytes().unwrap()).unwrap();
    for (key, value) in [
        ("name", serde_json::json!("path/name")),
        ("name", serde_json::json!(" padded ")),
        ("name", serde_json::json!("")),
        ("legacy_uri", serde_json::json!("relative/path")),
        ("priority", serde_json::json!("2147483648")),
        ("priority", serde_json::json!("-00")),
        ("enabled", serde_json::json!("true")),
        ("directory_mapping", serde_json::json!("/local/path")),
        ("kind", serde_json::json!("unknown")),
    ] {
        let mut fields = source.clone();
        fields[key] = value;
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| decode_root(&document))
                .is_err()
        );
    }
}
