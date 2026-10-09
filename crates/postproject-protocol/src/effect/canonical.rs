use std::slice;

use postproject_core::MetadataValue;

use super::{MetadataChange, MetadataEffect};
use crate::{Result, encode_metadata, fields::malformed};

/// Ordered pieces of one effect's canonical document, retaining at most one value.
///
/// Concatenate successful pieces to obtain exactly `document().canonical_bytes()`.
/// This is document encoding, distinct from the length-prefixed record body.
pub struct MetadataCanonicalParts<'a> {
    prefix: Option<Vec<u8>>,
    values: slice::Iter<'a, MetadataValue>,
    comma: bool,
    suffix: bool,
    closed: bool,
}

impl<'a> MetadataCanonicalParts<'a> {
    pub(super) fn new(effect: &'a MetadataEffect) -> Result<Self> {
        let (prefix, values, suffix) = match effect.change() {
            MetadataChange::Replaced(values) => {
                let empty = MetadataEffect::replaced(
                    effect.target(),
                    effect.property().clone(),
                    Vec::new(),
                )
                .document()?
                .canonical_bytes()?;
                // Canonical key order places "values" last. Reuse the actual
                // empty document's encoding rather than another JSON encoder.
                let prefix = empty.strip_suffix(b"]}").ok_or_else(malformed)?.to_vec();
                (prefix, values, true)
            }
            _ => (effect.document()?.canonical_bytes()?, &[][..], false),
        };
        Ok(Self {
            prefix: Some(prefix),
            values: values.iter(),
            comma: false,
            suffix,
            closed: false,
        })
    }
}

impl Iterator for MetadataCanonicalParts<'_> {
    type Item = Result<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.closed {
            return None;
        }
        if let Some(prefix) = self.prefix.take() {
            return Some(Ok(prefix));
        }
        if let Some(value) = self.values.next() {
            let result = encode_metadata(value)
                .and_then(|document| document.canonical_bytes())
                .map(|mut bytes| {
                    if self.comma {
                        bytes.insert(0, b',');
                    }
                    self.comma = true;
                    bytes
                });
            if result.is_err() {
                self.closed = true;
            }
            return Some(result);
        }
        self.closed = true;
        self.suffix.then(|| Ok(b"]}".to_vec()))
    }
}
