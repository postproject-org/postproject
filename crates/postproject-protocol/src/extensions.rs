//! Bounded preserved namespaced extension facts.

use crate::{
    Document, Limits, Result,
    fields::{limit, malformed},
};

/// Extension facts included in request equality and integrity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Extensions(Document);

impl Default for Extensions {
    fn default() -> Self {
        Self(Document {
            value: serde_json::json!({}),
        })
    }
}

impl Extensions {
    /// Validates a bounded object with namespaced keys and strict wire values.
    ///
    /// # Errors
    /// Rejects unnamespaced keys, excessive counts, bytes or container depth.
    pub fn new(document: Document) -> Result<Self> {
        let fields = document.value.as_object().ok_or_else(malformed)?;
        if fields.len() > 64 {
            return Err(limit());
        }
        for key in fields.keys() {
            if key.len() > 512 {
                return Err(limit());
            }
            let (namespace, local) = key.split_once(':').ok_or_else(malformed)?;
            if namespace.is_empty()
                || local.is_empty()
                || key.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                return Err(malformed());
            }
        }
        let bytes = document.canonical_bytes()?;
        Document::parse(&bytes, Limits::new(64 * 1024, 16, 4096)?)?;
        Ok(Self(document))
    }

    /// Borrows the preserved facts without changing their ordering semantics.
    #[must_use]
    pub const fn document(&self) -> &Document {
        &self.0
    }
}
