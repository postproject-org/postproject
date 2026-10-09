use postproject_core::{
    Dependency, DependencyKind, DependencyTarget, MAX_DEPENDENCIES_PER_SET, ObjectRef,
    RepresentationId,
};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{
        checked, decode_reference, encode_reference, exact, malformed, nullable, object, text,
        unsupported,
    },
};

/// One occurrence with its exact authored position, including repeated values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyOccurrence {
    source: RepresentationId,
    position: u64,
    dependency: Dependency,
}

impl DependencyOccurrence {
    /// Creates an occurrence within the existing core aggregate position bound.
    ///
    /// # Errors
    /// Rejects positions that cannot belong to a valid core dependency set.
    pub fn new(source: RepresentationId, position: u64, dependency: Dependency) -> Result<Self> {
        if usize::try_from(position)
            .ok()
            .is_none_or(|position| position >= MAX_DEPENDENCIES_PER_SET)
        {
            return Err(malformed());
        }
        Ok(Self {
            source,
            position,
            dependency,
        })
    }

    /// Returns the representation whose complete set contains this occurrence.
    #[must_use]
    pub const fn source_representation_id(&self) -> RepresentationId {
        self.source
    }

    /// Returns its original zero-based position.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.position
    }

    /// Returns exact open-world kind, target, resolution and authored text.
    #[must_use]
    pub const fn dependency(&self) -> &Dependency {
        &self.dependency
    }

    /// Encodes one occurrence without materializing the complete set.
    ///
    /// # Errors
    /// Rejects future target alternatives lacking a defined representation.
    pub fn document(&self) -> Result<Document> {
        let dependency = &self.dependency;
        let target = match dependency.target() {
            DependencyTarget::Asset(id) => ObjectRef::Asset(id),
            DependencyTarget::Representation(id) => ObjectRef::Representation(id),
            _ => return Err(unsupported()),
        };
        Ok(Document {
            value: json!({"kind":"dependency.occurrence", "source_representation_id":self.source.to_string(), "position":self.position.to_string(), "dependency":{
                "source_resource_id":dependency.source_resource_id().map(|id|id.to_string()), "kind":dependency.kind().as_str(), "target":encode_reference(target)?,
                "resolved_representation_id":dependency.resolved_representation_id().map(|id|id.to_string()), "required":dependency.is_required(), "authored_reference":dependency.authored_reference(),
            }}),
        })
    }

    /// Decodes through the same checked constructors as native dependency edits.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds, invalid targets, pinned resolutions and text.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &["kind", "source_representation_id", "position", "dependency"],
        )?;
        if document.kind()? != "dependency.occurrence" {
            return Err(unsupported());
        }
        let dependency = object(
            &fields["dependency"],
            &[
                "source_resource_id",
                "kind",
                "target",
                "resolved_representation_id",
                "required",
                "authored_reference",
            ],
        )?;
        let target = match decode_reference(&dependency["target"])? {
            ObjectRef::Asset(id) => DependencyTarget::Asset(id),
            ObjectRef::Representation(id) => DependencyTarget::Representation(id),
            _ => return Err(unsupported()),
        };
        Self::new(
            exact(&fields["source_representation_id"])?,
            exact(&fields["position"])?,
            checked(Dependency::new(
                nullable(&dependency["source_resource_id"], exact)?,
                checked(DependencyKind::new(text(&dependency["kind"])?))?,
                target,
                nullable(&dependency["resolved_representation_id"], exact)?,
                dependency["required"].as_bool().ok_or_else(malformed)?,
                text(&dependency["authored_reference"])?,
            ))?,
        )
    }
}
