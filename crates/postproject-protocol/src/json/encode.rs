use serde_json::Value;

use crate::{FailureKind, ProtocolError, Result};

pub(super) fn canonical(value: &Value, omit_digest: bool) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    write(&mut output, value, omit_digest)?;
    Ok(output)
}

fn scalar(output: &mut Vec<u8>, value: &impl serde::Serialize) -> Result<()> {
    serde_json::to_writer(output, value)
        .map_err(|_| ProtocolError::new(FailureKind::Malformed, "cannot encode JSON scalar"))
}

fn write(output: &mut Vec<u8>, value: &Value, omit_digest: bool) -> Result<()> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::String(text) => scalar(output, text)?,
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write(output, value, false)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut fields: Vec<_> = values
                .iter()
                .filter(|(key, _)| !omit_digest || *key != "digest")
                .collect();
            fields.sort_unstable_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
            for (index, (key, value)) in fields.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                scalar(output, key)?;
                output.push(b':');
                write(output, value, false)?;
            }
            output.push(b'}');
        }
        Value::Number(_) => {
            return Err(ProtocolError::new(
                FailureKind::Malformed,
                "JSON numbers are not exact wire values",
            ));
        }
    }
    Ok(())
}
