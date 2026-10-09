use postproject_core::{ArtifactDependencyPathSegment, Dependency};
use serde_json::json;

use crate::{
    DependencyOccurrence, Document, Result,
    fields::{checked, exact, malformed, object, unsupported},
};

/// One required authored edge at its original position in a captured path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityPathSegment {
    position: u64,
    occurrence: DependencyOccurrence,
}

impl ActivityPathSegment {
    /// Creates a segment without replacing its source dependency position.
    ///
    /// # Errors
    /// Rejects positions beyond the capture depth and optional dependencies.
    pub fn new(position: u64, occurrence: DependencyOccurrence) -> Result<Self> {
        if position >= 64 || !occurrence.dependency().is_required() {
            return Err(malformed());
        }
        Ok(Self {
            position,
            occurrence,
        })
    }

    /// Copies one historical segment through checked native dependency values.
    ///
    /// # Errors
    /// Rejects invalid stored text, targets or either original position.
    pub fn from_segment(position: u64, segment: &ArtifactDependencyPathSegment) -> Result<Self> {
        Self::new(
            position,
            DependencyOccurrence::new(
                segment.source_representation_id(),
                u64::from(segment.dependency_position()),
                checked(Dependency::new(
                    segment.source_resource_id(),
                    segment.kind().clone(),
                    segment.target(),
                    segment.resolved_representation_id(),
                    true,
                    segment.authored_reference(),
                ))?,
            )?,
        )
    }

    /// Returns its zero-based position within this captured path.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.position
    }

    /// Returns the source, authored position and exact required dependency.
    #[must_use]
    pub const fn occurrence(&self) -> &DependencyOccurrence {
        &self.occurrence
    }

    /// Encodes one historical edge without materializing its path.
    ///
    /// # Errors
    /// Rejects future dependency target alternatives without a wire definition.
    pub fn document(&self) -> Result<Document> {
        Ok(Document {
            value: json!({"kind":"activity.dependency-segment", "position":self.position.to_string(), "occurrence":self.occurrence.document()?.value}),
        })
    }

    /// Decodes exact authored facts through the native dependency constructor.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds, invalid references and optional edges.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "position", "occurrence"])?;
        if document.kind()? != "activity.dependency-segment" {
            return Err(unsupported());
        }
        Self::new(
            exact(&fields["position"])?,
            DependencyOccurrence::from_document(&Document {
                value: fields["occurrence"].clone(),
            })?,
        )
    }
}
