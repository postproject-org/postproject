//! Retained failures contain semantic facts, never raw backend diagnostics.

use postproject_core::{Error, ErrorKind};
use postproject_protocol::{Document, Limits, Rejection};

#[test]
fn transient_failures_cannot_be_retained_and_diagnostics_are_discarded() {
    for kind in [
        ErrorKind::Io,
        ErrorKind::Storage,
        ErrorKind::Migration,
        ErrorKind::Internal,
        ErrorKind::Cancelled,
    ] {
        assert!(Rejection::domain(&Error::new(kind, "private credential diagnostic")).is_err());
    }
    for kind in [
        ErrorKind::InvalidArgument,
        ErrorKind::NotFound,
        ErrorKind::AlreadyExists,
        ErrorKind::Conflict,
        ErrorKind::Unsupported,
        ErrorKind::AmbiguousResolution,
        ErrorKind::Fingerprint,
    ] {
        let rejection =
            Rejection::domain(&Error::new(kind, "private credential diagnostic")).unwrap();
        let document = rejection.document().unwrap();
        let bytes = document.canonical_bytes().unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("private"));
        assert_eq!(
            Rejection::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap(),
            rejection
        );
    }
}

#[test]
fn transient_wire_categories_and_unknown_fields_reject() {
    for wire in [
        r#"{"kind":"storage","conflict":null}"#,
        r#"{"kind":"not_found","conflict":null,"message":"secret"}"#,
    ] {
        assert!(
            Rejection::from_document(&Document::parse(wire.as_bytes(), Limits::default()).unwrap())
                .is_err()
        );
    }
}
