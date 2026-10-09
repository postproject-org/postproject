//! Prepared fingerprint bytes have no authority-assigned observation boundary.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

use crate::{
    Result,
    fields::{exact, malformed, object, text},
};

pub(super) fn encode(algorithm: &str, version: u16, value: &[u8]) -> Value {
    json!({"algorithm":algorithm, "version":version.to_string(), "value":STANDARD.encode(value)})
}

pub(super) fn decode(value: &Value) -> Result<(&str, u16, Vec<u8>)> {
    let fields = object(value, &["algorithm", "version", "value"])?;
    let encoded = text(&fields["value"])?;
    let bytes = STANDARD.decode(encoded).map_err(|_| malformed())?;
    if STANDARD.encode(&bytes) != encoded {
        return Err(malformed());
    }
    Ok((
        text(&fields["algorithm"])?,
        exact(&fields["version"])?,
        bytes,
    ))
}
