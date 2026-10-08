//! Exact tagged metadata, preserving repetition and ordered struct fields.

use base64::{Engine, engine::general_purpose::STANDARD};
use postproject_core::{
    DecimalValue, MAX_LANGUAGE_TAG_BYTES, MAX_METADATA_BINARY_BYTES, MAX_METADATA_COLLECTION_ITEMS,
    MAX_METADATA_NESTING_DEPTH, MAX_METADATA_TEXT_BYTES, MAX_METADATA_URI_BYTES,
    MAX_PROPERTY_ID_BYTES, MetadataField, MetadataValue, MetadataValueKind, PropertyId,
    RationalValue, Timestamp,
};
use serde_json::{Value, json};

use crate::{
    Document, Result,
    fields::{
        array, bounded_text, checked, decode_reference, encode_reference, exact, limit, malformed,
        object, text, unsupported,
    },
};

/// Converts validated metadata to the exact protocol representation.
///
/// # Errors
/// Rejects any future kind unsupported by this protocol version.
pub fn encode_metadata(value: &MetadataValue) -> Result<Document> {
    Ok(Document {
        value: encode(value)?,
    })
}

/// Decodes through the checked domain constructors, including nested limits.
///
/// # Errors
/// Rejects malformed values, unsupported kinds or domain limit violations.
pub fn decode_metadata(document: &Document) -> Result<MetadataValue> {
    decode(&document.value, 1)
}

fn required<T>(value: Option<T>) -> Result<T> {
    value.ok_or_else(malformed)
}

fn tagged(kind: &str, value: impl serde::Serialize) -> Value {
    json!({"kind":kind,"value":value})
}

pub(crate) fn encode(value: &MetadataValue) -> Result<Value> {
    Ok(match value.kind() {
        MetadataValueKind::String => tagged("string", required(value.as_string())?),
        MetadataValueKind::LangString => {
            let (text, language) = required(value.as_language_string())?;
            json!({"kind":"language_string","value":text,"language":language})
        }
        MetadataValueKind::I64 => tagged("i64", required(value.as_i64())?.to_string()),
        MetadataValueKind::U64 => tagged("u64", required(value.as_u64())?.to_string()),
        MetadataValueKind::Decimal => {
            let decimal = required(value.as_decimal())?;
            json!({"kind":"decimal","coefficient":decimal.coefficient().to_string(),"scale":decimal.scale().to_string()})
        }
        MetadataValueKind::Bool => tagged("boolean", required(value.as_bool())?),
        MetadataValueKind::Timestamp => tagged(
            "timestamp",
            required(value.as_timestamp())?.as_unix_micros().to_string(),
        ),
        MetadataValueKind::Uri => tagged("uri", required(value.as_uri())?),
        MetadataValueKind::Bytes => tagged("bytes", STANDARD.encode(required(value.as_bytes())?)),
        MetadataValueKind::Rational => {
            let rational = required(value.as_rational())?;
            json!({"kind":"rational","numerator":rational.numerator().to_string(),"denominator":rational.denominator().to_string()})
        }
        MetadataValueKind::List => tagged(
            "list",
            required(value.as_list())?
                .iter()
                .map(encode)
                .collect::<Result<Vec<_>>>()?,
        ),
        MetadataValueKind::Struct => tagged(
            "struct",
            required(value.as_structure())?
                .iter()
                .map(|field| {
                    Ok(json!({"name":field.name().as_str(),"value":encode(field.value())?}))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        MetadataValueKind::Reference => tagged(
            "reference",
            encode_reference(required(value.as_reference())?)?,
        ),
        _ => return Err(unsupported()),
    })
}

pub(crate) fn decode(value: &Value, depth: usize) -> Result<MetadataValue> {
    if depth > MAX_METADATA_NESTING_DEPTH {
        return Err(limit());
    }
    let kind = text(value.get("kind").ok_or_else(malformed)?)?;
    let keys = match kind {
        "decimal" => &["kind", "coefficient", "scale"][..],
        "rational" => &["kind", "numerator", "denominator"][..],
        "language_string" => &["kind", "value", "language"][..],
        "string" | "i64" | "u64" | "boolean" | "timestamp" | "uri" | "bytes" | "list"
        | "struct" | "reference" => &["kind", "value"][..],
        _ => return Err(unsupported()),
    };
    let fields = object(value, keys)?;
    let data = fields.get("value").unwrap_or(&Value::Null);
    match kind {
        "string" => checked(MetadataValue::string(bounded_text(
            data,
            MAX_METADATA_TEXT_BYTES,
        )?)),
        "language_string" => checked(MetadataValue::language_string(
            bounded_text(data, MAX_METADATA_TEXT_BYTES)?,
            bounded_text(&fields["language"], MAX_LANGUAGE_TAG_BYTES)?,
        )),
        "i64" => Ok(MetadataValue::i64(exact(data)?)),
        "u64" => Ok(MetadataValue::u64(exact(data)?)),
        "decimal" => Ok(MetadataValue::decimal(
            DecimalValue::new(exact(&fields["coefficient"])?, exact(&fields["scale"])?)
                .map_err(|_| limit())?,
        )),
        "boolean" => Ok(MetadataValue::boolean(
            data.as_bool().ok_or_else(malformed)?,
        )),
        "timestamp" => Ok(MetadataValue::timestamp(Timestamp::from_unix_micros(
            exact(data)?,
        ))),
        "uri" => checked(MetadataValue::uri(bounded_text(
            data,
            MAX_METADATA_URI_BYTES,
        )?)),
        "bytes" => {
            // Check expansion before allocating decoded bytes; the engine also
            // rejects nonzero pad bits, absent padding and alternate alphabets.
            let encoded = bounded_text(data, MAX_METADATA_BINARY_BYTES.div_ceil(3) * 4)?;
            let bytes = STANDARD.decode(encoded).map_err(|_| malformed())?;
            if bytes.len() > MAX_METADATA_BINARY_BYTES {
                return Err(limit());
            }
            checked(MetadataValue::bytes(bytes))
        }
        "rational" => Ok(MetadataValue::rational(checked(RationalValue::new(
            exact(&fields["numerator"])?,
            exact(&fields["denominator"])?,
        ))?)),
        "list" => {
            let children = array(data, MAX_METADATA_COLLECTION_ITEMS)?;
            MetadataValue::list(
                children
                    .iter()
                    .map(|v| decode(v, depth + 1))
                    .collect::<Result<_>>()?,
            )
            .map_err(|_| limit())
        }
        "struct" => {
            let children = array(data, MAX_METADATA_COLLECTION_ITEMS)?;
            let fields = children
                .iter()
                .map(|v| {
                    let field = object(v, &["name", "value"])?;
                    Ok(MetadataField::new(
                        checked(PropertyId::new(bounded_text(
                            &field["name"],
                            MAX_PROPERTY_ID_BYTES,
                        )?))?,
                        decode(&field["value"], depth + 1)?,
                    ))
                })
                .collect::<Result<_>>()?;
            MetadataValue::structure(fields).map_err(|_| limit())
        }
        "reference" => Ok(MetadataValue::reference(decode_reference(data)?)),
        _ => Err(unsupported()),
    }
}
