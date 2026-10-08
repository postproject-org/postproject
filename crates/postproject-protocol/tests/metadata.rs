//! Every tagged kind uses core validation and retains exact domain meaning.

use postproject_core::{
    ActivityId, AssetId, DecimalValue, JobId, MetadataField, MetadataValue, ObjectRef,
    ProductionId, PropertyId, RationalValue, RepresentationId, ResourceId, Timestamp,
};
use postproject_protocol::{Document, FailureKind, Limits, decode_metadata, encode_metadata};

fn round_trip(value: &MetadataValue) {
    let encoded = encode_metadata(value).unwrap().canonical_bytes().unwrap();
    let parsed = Document::parse(&encoded, Limits::default()).unwrap();
    assert_eq!(decode_metadata(&parsed).unwrap(), *value);
}

#[test]
fn every_metadata_kind_round_trips_without_float_or_order_loss() {
    let values = vec![
        MetadataValue::string("exact 😀\n").unwrap(),
        MetadataValue::language_string("Grüße", "de-DE").unwrap(),
        MetadataValue::i64(i64::MIN),
        MetadataValue::u64(u64::MAX),
        MetadataValue::decimal(DecimalValue::new(i128::MIN, 1024).unwrap()),
        MetadataValue::boolean(false),
        MetadataValue::timestamp(Timestamp::from_unix_micros(i64::MIN)),
        MetadataValue::uri("https://EXAMPLE.com/a%2fb").unwrap(),
        MetadataValue::bytes(vec![0, 1, 255]).unwrap(),
        MetadataValue::rational(RationalValue::new(i64::MIN, u64::MAX).unwrap()),
        MetadataValue::list(vec![]).unwrap(),
        MetadataValue::structure(vec![]).unwrap(),
    ];
    for value in &values {
        round_trip(value);
    }
    round_trip(&MetadataValue::list(values.clone()).unwrap());
    let fields = values
        .into_iter()
        .map(|value| MetadataField::new(PropertyId::new("repeated").unwrap(), value))
        .collect();
    round_trip(&MetadataValue::structure(fields).unwrap());
    for reference in [
        ObjectRef::Production(ProductionId::new()),
        ObjectRef::Asset(AssetId::new()),
        ObjectRef::Representation(RepresentationId::new()),
        ObjectRef::Resource(ResourceId::new()),
        ObjectRef::Activity(ActivityId::new()),
        ObjectRef::Job(JobId::new()),
    ] {
        round_trip(&MetadataValue::reference(reference));
    }
}

#[test]
fn deepest_legal_struct_survives_its_extra_wire_containers() {
    let mut value = MetadataValue::string("leaf").unwrap();
    for _ in 1..32 {
        value = MetadataValue::structure(vec![MetadataField::new(
            PropertyId::new("field").unwrap(),
            value,
        )])
        .unwrap();
    }
    assert_eq!(value.nesting_depth(), 32);
    round_trip(&value);
}

#[test]
fn malformed_fields_exact_numbers_references_and_base64_reject() {
    for input in [
        r#"{"kind":"i64","value":"01"}"#,
        r#"{"kind":"i64","value":"+1"}"#,
        r#"{"kind":"i64","value":"-0"}"#,
        r#"{"kind":"i64","value":"9223372036854775808"}"#,
        r#"{"kind":"u64","value":"-1"}"#,
        r#"{"kind":"u64","value":true}"#,
        r#"{"kind":"boolean","value":"true"}"#,
        r#"{"kind":"rational","numerator":"1","denominator":"0"}"#,
        r#"{"kind":"bytes","value":"AQ"}"#,
        r#"{"kind":"bytes","value":"AR=="}"#,
        r#"{"kind":"bytes","value":"AQ==\n"}"#,
        r#"{"kind":"bytes","value":"_w=="}"#,
        r#"{"kind":"string","value":"ok","critical":"unknown"}"#,
        r#"{"kind":"string","value":"bad\u0000text"}"#,
        r#"{"kind":"reference","value":{"kind":"asset","id":"EEEEEEEE-0000-0000-0000-000000000000"}}"#,
        r#"{"kind":"reference","value":{"kind":"asset","id":"123"}}"#,
    ] {
        let document = Document::parse(input.as_bytes(), Limits::default()).unwrap();
        assert_eq!(
            decode_metadata(&document).unwrap_err().kind(),
            FailureKind::Malformed
        );
    }
    let unknown = Document::parse(br#"{"kind":"future","value":"x"}"#, Limits::default()).unwrap();
    assert_eq!(
        decode_metadata(&unknown).unwrap_err().kind(),
        FailureKind::Unsupported
    );
}

#[test]
fn binary_expansion_and_nested_domain_limits_remain_supported() {
    round_trip(
        &MetadataValue::bytes(vec![255; postproject_core::MAX_METADATA_BINARY_BYTES]).unwrap(),
    );
    let mut nested = r#"{"kind":"string","value":"leaf"}"#.to_owned();
    for _ in 0..32 {
        nested = format!(r#"{{"kind":"list","value":[{nested}]}}"#);
    }
    let document = Document::parse(nested.as_bytes(), Limits::default()).unwrap();
    assert_eq!(
        decode_metadata(&document).unwrap_err().kind(),
        FailureKind::LimitExceeded
    );
    let oversized = format!(
        r#"{{"kind":"list","value":[{}]}}"#,
        vec![r#"{"kind":"boolean","value":true}"#; 4097].join(",")
    );
    let document = Document::parse(oversized.as_bytes(), Limits::default()).unwrap();
    assert_eq!(
        decode_metadata(&document).unwrap_err().kind(),
        FailureKind::LimitExceeded
    );
}
