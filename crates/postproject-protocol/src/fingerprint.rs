//! Original fingerprint evidence; encoding never recomputes media fingerprints.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use postproject_core::FingerprintSnapshot;
use serde_json::{Value, json};

use crate::{
    Document, Result,
    fields::{checked, exact, malformed, nullable, object, text, unsupported},
};

/// Encodes exact fingerprint bytes and their original optional observation boundary.
///
/// # Errors
/// Rejects zero or out-of-storage-range revision sequences.
pub fn encode_fingerprint_snapshot(snapshot: &FingerprintSnapshot) -> Result<Document> {
    Ok(Document {
        value: json!({"kind":"fingerprint.snapshot", "fingerprint":encode_snapshot(snapshot)?}),
    })
}

/// Decodes exact versioned fingerprint evidence without accessing media.
///
/// # Errors
/// Rejects unknown fields/kinds, invalid domains, noncanonical padded base64 and
/// invalid observation sequences. Per-document limits are checked by the parser.
pub fn decode_fingerprint_snapshot(document: &Document) -> Result<FingerprintSnapshot> {
    let fields = object(&document.value, &["kind", "fingerprint"])?;
    if text(&fields["kind"])? != "fingerprint.snapshot" {
        return Err(unsupported());
    }
    decode_snapshot(&fields["fingerprint"])
}

pub(crate) fn encode_snapshot(snapshot: &FingerprintSnapshot) -> Result<Value> {
    if let Some(sequence) = snapshot.observed_revision_sequence() {
        revision_sequence(sequence)?;
    }
    Ok(
        json!({"algorithm":snapshot.algorithm(), "version":snapshot.version().to_string(), "value":STANDARD.encode(snapshot.value()), "observed_revision_sequence":snapshot.observed_revision_sequence().map(|sequence| sequence.to_string())}),
    )
}

pub(crate) fn decode_snapshot(value: &Value) -> Result<FingerprintSnapshot> {
    let fields = object(
        value,
        &[
            "algorithm",
            "version",
            "value",
            "observed_revision_sequence",
        ],
    )?;
    let sequence = nullable(&fields["observed_revision_sequence"], |value| {
        revision_sequence(exact(value)?)
    })?;
    let bytes = STANDARD
        .decode(text(&fields["value"])?)
        .map_err(|_| malformed())?;
    checked(FingerprintSnapshot::new(
        text(&fields["algorithm"])?,
        exact(&fields["version"])?,
        bytes,
        sequence,
    ))
}

pub(crate) fn revision_sequence(sequence: u64) -> Result<u64> {
    if sequence == 0 || sequence > i64::MAX.unsigned_abs() {
        return Err(malformed());
    }
    Ok(sequence)
}
