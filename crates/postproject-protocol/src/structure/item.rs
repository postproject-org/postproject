use postproject_core::{
    MAX_CONTENT_MEMBERS, MAX_SEQUENCE_EXCEPTIONS, RepresentationId, ResourceMember, ResourceRole,
};
use serde_json::json;

use crate::{
    Document, Result,
    fields::{checked, exact, limit, malformed, object, text, unsupported},
};

/// One ordered resource membership, bound to its representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructureMember {
    owner: RepresentationId,
    position: usize,
    member: ResourceMember,
}

impl StructureMember {
    /// Creates a bounded membership frame; the assembler checks continuity.
    ///
    /// # Errors
    /// Rejects positions outside the native content-member limit.
    pub fn new(owner: RepresentationId, position: usize, member: ResourceMember) -> Result<Self> {
        if position >= MAX_CONTENT_MEMBERS {
            return Err(limit());
        }
        Ok(Self {
            owner,
            position,
            member,
        })
    }

    /// Returns the owning representation.
    #[must_use]
    pub const fn representation_id(&self) -> RepresentationId {
        self.owner
    }

    /// Returns the zero-based structural position.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Returns the checked resource, role and requiredness.
    #[must_use]
    pub const fn member(&self) -> &ResourceMember {
        &self.member
    }

    /// Encodes one member without collecting its siblings.
    #[must_use]
    pub fn document(&self) -> Document {
        Document {
            value: json!({"kind":"structure.member", "representation_id":self.owner.to_string(), "position":self.position.to_string(), "resource_id":self.member.resource_id().to_string(), "role":self.member.role().as_str(), "required":self.member.is_required()}),
        }
    }

    /// Decodes a checked member and bounded position.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds, invalid roles and nonboolean requiredness.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "representation_id",
                "position",
                "resource_id",
                "role",
                "required",
            ],
        )?;
        if text(&fields["kind"])? != "structure.member" {
            return Err(unsupported());
        }
        let member = ResourceMember::new(
            exact(&fields["resource_id"])?,
            checked(ResourceRole::new(text(&fields["role"])?))?,
            fields["required"].as_bool().ok_or_else(malformed)?,
        );
        Self::new(
            exact(&fields["representation_id"])?,
            exact(&fields["position"])?,
            member,
        )
    }
}

/// One sorted sparse image-sequence exception, bound to its representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceException {
    owner: RepresentationId,
    position: usize,
    frame: i64,
}

impl SequenceException {
    /// Creates a bounded frame; the assembler checks its frame domain and order.
    ///
    /// # Errors
    /// Rejects positions outside the native exception limit.
    pub fn new(owner: RepresentationId, position: usize, frame: i64) -> Result<Self> {
        if position >= MAX_SEQUENCE_EXCEPTIONS {
            return Err(limit());
        }
        Ok(Self {
            owner,
            position,
            frame,
        })
    }

    /// Returns the owning representation.
    #[must_use]
    pub const fn representation_id(self) -> RepresentationId {
        self.owner
    }

    /// Returns the zero-based position in the sorted exception list.
    #[must_use]
    pub const fn position(self) -> usize {
        self.position
    }

    /// Returns the exact missing frame number.
    #[must_use]
    pub const fn frame(self) -> i64 {
        self.frame
    }

    /// Encodes one exception without collecting its siblings.
    #[must_use]
    pub fn document(self) -> Document {
        Document {
            value: json!({"kind":"structure.exception", "representation_id":self.owner.to_string(), "position":self.position.to_string(), "frame":self.frame.to_string()}),
        }
    }

    /// Decodes exact ownership, bounded position and frame number.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds and invalid integer or identity spelling.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &["kind", "representation_id", "position", "frame"],
        )?;
        if text(&fields["kind"])? != "structure.exception" {
            return Err(unsupported());
        }
        Self::new(
            exact(&fields["representation_id"])?,
            exact(&fields["position"])?,
            exact(&fields["frame"])?,
        )
    }
}
