//! Exact authored media facts, with checked scalar boundaries and no I/O.

use postproject_core::{
    Asset, AssetId, FileFacts, Resource, ResourceFingerprint, ResourceId, Timestamp,
};
use postproject_protocol::{Document, Limits, ResourceHeader, decode_asset, encode_asset};

#[test]
fn assets_retain_identity_extreme_time_and_exact_optional_provenance() {
    for (time, name, provenance) in [
        (i64::MIN, None, None),
        (i64::MAX, Some(String::new()), Some(String::new())),
        (
            0,
            Some("Exact 名\n\0".into()),
            Some("urn:unknown:EXACT%2f".into()),
        ),
    ] {
        let asset = Asset::new(
            AssetId::new(),
            Timestamp::from_unix_micros(time),
            name,
            provenance,
        );
        let bytes = encode_asset(&asset).canonical_bytes().unwrap();
        let document = Document::parse(&bytes, Limits::default()).unwrap();
        assert_eq!(decode_asset(&document).unwrap(), asset);
        let fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            fields["created_at_micros"].as_str().unwrap(),
            time.to_string()
        );
        assert_eq!(fields["import_source"].as_str(), asset.import_source());
    }
}

#[test]
fn resource_headers_preserve_exact_file_facts_and_separate_fingerprint_collections() {
    for facts in [
        None,
        Some(FileFacts::new(0, None)),
        Some(FileFacts::new(
            u64::MAX,
            Some(Timestamp::from_unix_micros(i64::MIN)),
        )),
        Some(FileFacts::new(
            9_007_199_254_740_993,
            Some(Timestamp::from_unix_micros(i64::MAX)),
        )),
    ] {
        let resource = Resource::new(
            ResourceId::new(),
            vec![ResourceFingerprint::new("UNKNOWN-domain", u16::MAX, vec![0, 255]).unwrap()],
            facts,
        );
        let header = ResourceHeader::from_resource(&resource);
        let bytes = header.document().canonical_bytes().unwrap();
        assert_eq!(
            ResourceHeader::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                .unwrap(),
            header
        );
        assert_eq!(header.id(), resource.id());
        assert_eq!(header.file_facts(), facts);
        assert!(!String::from_utf8(bytes).unwrap().contains("fingerprint"));
    }
}

#[test]
fn malformed_media_headers_reject_unknown_fields_noncanonical_numbers_and_ids() {
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(0), None, None);
    let mut fields: serde_json::Value =
        serde_json::from_slice(&encode_asset(&asset).canonical_bytes().unwrap()).unwrap();
    for key in ["row_id", "schema", "resources"] {
        fields[key] = serde_json::json!("1");
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| decode_asset(&document))
                .is_err()
        );
        fields.as_object_mut().unwrap().remove(key);
    }
    for time in [
        serde_json::json!(0),
        serde_json::json!("00"),
        serde_json::json!("9223372036854775808"),
    ] {
        fields["created_at_micros"] = time;
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| decode_asset(&document))
                .is_err()
        );
    }
    fields["created_at_micros"] = "0".into();
    fields["id"] = asset.id().to_string().replace('-', "").into();
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| decode_asset(&document))
            .is_err()
    );
    let resource = Resource::new(ResourceId::new(), Vec::new(), Some(FileFacts::new(0, None)));
    let mut fields: serde_json::Value = serde_json::from_slice(
        &ResourceHeader::from_resource(&resource)
            .document()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    for size in [
        serde_json::json!(-1),
        serde_json::json!("-1"),
        serde_json::json!("18446744073709551616"),
    ] {
        fields["file_facts"]["size_bytes"] = size;
        assert!(
            Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
                .and_then(|document| ResourceHeader::from_document(&document))
                .is_err()
        );
    }
    fields["file_facts"]["size_bytes"] = "0".into();
    fields["fingerprints"] = serde_json::json!([]);
    assert!(
        Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default())
            .and_then(|document| ResourceHeader::from_document(&document))
            .is_err()
    );
}
