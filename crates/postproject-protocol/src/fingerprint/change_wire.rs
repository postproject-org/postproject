use serde_json::json;

use super::{FingerprintChangeStart, FingerprintRecomputation, decode_snapshot, encode_snapshot};
use crate::{
    Document, Result,
    fields::{decode_reference, encode_reference, exact, malformed, nullable, object, unsupported},
};

impl FingerprintChangeStart {
    /// Encodes a bounded evidence pair without collecting an unbounded owner list.
    ///
    /// # Errors
    /// Rejects unsupported targets or invalid snapshot revision boundaries.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: json!({
                "kind":"fingerprint.change", "target":encode_reference(self.target())?,
                "previous":self.previous().map(encode_snapshot).transpose()?,
                "current":encode_snapshot(self.current())?,
                "archived_position":self.archived_position().map(|position|position.to_string()),
                "marker_count":self.marker_count().to_string(),
                "cleared_marker":self.cleared_marker().map(|marker|marker.document().value),
                "dependency_invalidated":self.dependency_invalidated(),
            }),
        })
    }

    /// Decodes checked evidence without recomputing fingerprints or work state.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds and impossible transition combinations.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "target",
                "previous",
                "current",
                "archived_position",
                "marker_count",
                "cleared_marker",
                "dependency_invalidated",
            ],
        )?;
        if document.kind()? != "fingerprint.change" {
            return Err(unsupported());
        }
        Self::new(
            decode_reference(&fields["target"])?,
            nullable(&fields["previous"], decode_snapshot)?,
            decode_snapshot(&fields["current"])?,
            nullable(&fields["archived_position"], exact)?,
            exact(&fields["marker_count"])?,
            nullable(&fields["cleared_marker"], |value| {
                FingerprintRecomputation::from_document(&Document {
                    value: value.clone(),
                })
            })?,
            fields["dependency_invalidated"]
                .as_bool()
                .ok_or_else(malformed)?,
        )
    }
}
