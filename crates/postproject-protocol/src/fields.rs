//! Shared exact field checks; no unchecked serde-derived domain constructors.

use std::{fmt::Display, str::FromStr};

use postproject_core::{ObjectRef, Result as DomainResult};
use serde_json::{Map, Value, json};

use crate::{FailureKind, ProtocolError, Result};

pub(crate) fn malformed() -> ProtocolError {
    ProtocolError::new(FailureKind::Malformed, "invalid exact domain value")
}

pub(crate) fn unsupported() -> ProtocolError {
    ProtocolError::new(FailureKind::Unsupported, "unsupported required domain kind")
}

pub(crate) fn limit() -> ProtocolError {
    ProtocolError::new(FailureKind::LimitExceeded, "domain value exceeds limit")
}

pub(crate) fn checked<T>(result: DomainResult<T>) -> Result<T> {
    result.map_err(|_| malformed())
}

pub(crate) fn object<'a>(value: &'a Value, keys: &[&str]) -> Result<&'a Map<String, Value>> {
    let fields = value.as_object().ok_or_else(malformed)?;
    if fields.len() != keys.len() || keys.iter().any(|key| !fields.contains_key(*key)) {
        return Err(malformed());
    }
    Ok(fields)
}

pub(crate) fn text(value: &Value) -> Result<&str> {
    value.as_str().ok_or_else(malformed)
}

pub(crate) fn nullable<T>(
    value: &Value,
    decode: impl FnOnce(&Value) -> Result<T>,
) -> Result<Option<T>> {
    if value.is_null() {
        Ok(None)
    } else {
        decode(value).map(Some)
    }
}

pub(crate) fn bounded_text(value: &Value, max: usize) -> Result<&str> {
    let text = text(value)?;
    if text.len() > max {
        return Err(limit());
    }
    Ok(text)
}

pub(crate) fn exact<T: FromStr + Display>(value: &Value) -> Result<T> {
    let text = text(value)?;
    let parsed: T = text.parse().map_err(|_| malformed())?;
    if parsed.to_string() != text {
        return Err(malformed());
    }
    Ok(parsed)
}

pub(crate) fn array(value: &Value, max: usize) -> Result<&[Value]> {
    let values = value.as_array().ok_or_else(malformed)?;
    if values.len() > max {
        return Err(limit());
    }
    Ok(values)
}

pub(crate) fn encode_reference(reference: ObjectRef) -> Result<Value> {
    let (kind, id) = match reference {
        ObjectRef::Production(id) => ("production", id.to_string()),
        ObjectRef::Asset(id) => ("asset", id.to_string()),
        ObjectRef::Representation(id) => ("representation", id.to_string()),
        ObjectRef::Resource(id) => ("resource", id.to_string()),
        ObjectRef::Activity(id) => ("activity", id.to_string()),
        ObjectRef::Job(id) => ("job", id.to_string()),
        _ => return Err(unsupported()),
    };
    Ok(json!({"kind":kind,"id":id}))
}

pub(crate) fn decode_reference(value: &Value) -> Result<ObjectRef> {
    let fields = object(value, &["kind", "id"])?;
    let id = &fields["id"];
    Ok(match text(&fields["kind"])? {
        "production" => ObjectRef::Production(exact(id)?),
        "asset" => ObjectRef::Asset(exact(id)?),
        "representation" => ObjectRef::Representation(exact(id)?),
        "resource" => ObjectRef::Resource(exact(id)?),
        "activity" => ObjectRef::Activity(exact(id)?),
        "job" => ObjectRef::Job(exact(id)?),
        _ => return Err(unsupported()),
    })
}
