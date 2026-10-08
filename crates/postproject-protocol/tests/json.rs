//! Strict framing, resource bounds and input confidentiality regressions.

use postproject_protocol::{Document, FailureKind, Limits};

fn failure(input: &[u8], limits: Limits) -> FailureKind {
    Document::parse(input, limits)
        .expect_err("reject input")
        .kind()
}

#[test]
fn duplicate_keys_reject_at_every_level_including_equivalent_escapes() {
    for input in [
        r#"{"a":null,"a":true}"#,
        r#"{"a":[{"nested":"one","nested":"two"}]}"#,
        r#"{"a":null,"\u0061":true}"#,
    ] {
        assert_eq!(
            failure(input.as_bytes(), Limits::default()),
            FailureKind::Malformed
        );
    }
}

#[test]
fn numbers_utf8_surrogates_trailing_data_and_nonobjects_reject() {
    for input in [
        b"{\"a\":1}".as_slice(),
        b"{\"a\":1.0}",
        b"{\"a\":-0}",
        b"{\"a\":1e20}",
        b"{\"a\":NaN}",
        b"{\"a\":\"\xff\"}",
        br#"{"a":"\ud800"}"#,
        br#"{"a":"\udfff"}"#,
        b"{}{}",
        b"null",
        b"[]",
        b"\xef\xbb\xbf{}",
    ] {
        assert_eq!(failure(input, Limits::default()), FailureKind::Malformed);
    }
}

#[test]
fn input_spelling_normalizes_but_arrays_and_exact_text_do_not() {
    let a = Document::parse(br#"{"b":"\u0061","a":["x","x"]}"#, Limits::default()).unwrap();
    let b = Document::parse(br#" { "a": ["x", "x"], "b": "a" } "#, Limits::default()).unwrap();
    assert_eq!(a, b);
    assert_ne!(
        a,
        Document::parse(br#"{"b":"a","a":["x"]}"#, Limits::default()).unwrap()
    );
    Document::parse(
        br#"{"pair":"\ud83d\ude00","yes":true,"absent":null}"#,
        Limits::default(),
    )
    .unwrap();
}

#[test]
fn byte_node_and_container_limits_have_typed_failures() {
    assert_eq!(
        failure(b"{}", Limits::new(1, 1, 1).unwrap()),
        FailureKind::LimitExceeded
    );
    Document::parse(b"{}", Limits::new(2, 1, 1).unwrap()).unwrap();
    assert_eq!(
        failure(br#"{"a":null}"#, Limits::new(128, 1, 1).unwrap()),
        FailureKind::LimitExceeded
    );
    Document::parse(br#"{"a":null}"#, Limits::new(128, 1, 2).unwrap()).unwrap();
    assert_eq!(
        failure(br#"{"a":[]}"#, Limits::new(128, 1, 2).unwrap()),
        FailureKind::LimitExceeded
    );
    Document::parse(br#"{"a":[]}"#, Limits::new(128, 2, 2).unwrap()).unwrap();
    let nested = format!("{{\"a\":{}null{}}}", "[".repeat(192), "]".repeat(192));
    assert_eq!(
        failure(nested.as_bytes(), Limits::default()),
        FailureKind::LimitExceeded
    );
    let legal = format!("{{\"a\":{}null{}}}", "[".repeat(150), "]".repeat(150));
    Document::parse(legal.as_bytes(), Limits::default()).unwrap();
}

#[test]
fn diagnostics_never_echo_caller_data() {
    let error = Document::parse(br#"{"private-token":oops}"#, Limits::default()).unwrap_err();
    assert!(!error.to_string().contains("private-token"));
    assert!(!format!("{error:?}").contains("private-token"));
}
