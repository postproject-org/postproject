use std::collections::BTreeSet;

use postproject_core::{
    ContentStructure, ImageSequenceDescriptor, Representation, ResourceId, ResourceMember,
};

use super::{SequenceException, StructureHeader, StructureMember, header::Shape};
use crate::{
    Document, Result,
    fields::{checked, malformed, text, unsupported},
};

/// Decodes one complete structure with explicit continuation totals.
///
/// Memory is bounded by the existing core aggregate limits. An invalid item
/// makes this assembler terminal; partial aggregates are never returned.
pub struct StructureAssembler {
    header: StructureHeader,
    members: Vec<ResourceMember>,
    resources: BTreeSet<ResourceId>,
    exceptions: Vec<i64>,
    failed: bool,
}

impl StructureAssembler {
    /// Starts an empty assembler without allocating from advertised totals.
    #[must_use]
    pub const fn new(header: StructureHeader) -> Self {
        Self {
            header,
            members: Vec::new(),
            resources: BTreeSet::new(),
            exceptions: Vec::new(),
            failed: false,
        }
    }

    /// Accepts exactly the next member or sparse exception frame.
    ///
    /// # Errors
    /// Rejects wrong kinds/owners, noncontiguous positions, duplicates, invalid
    /// domains and surplus items. Any error leaves the assembler terminal.
    pub fn push(&mut self, document: &Document) -> Result<()> {
        if self.failed {
            return Err(malformed());
        }
        let result = self.push_open(document);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn push_open(&mut self, document: &Document) -> Result<()> {
        match text(document.value.get("kind").ok_or_else(malformed)?)? {
            "structure.member" => {
                let item = StructureMember::from_document(document)?;
                if item.representation_id() != self.header.owner
                    || item.position() != self.members.len()
                    || self.members.len() >= self.header.member_count()
                    || self.resources.contains(&item.member().resource_id())
                    || matches!(self.header.shape, Shape::Ordered(_))
                        && !item.member().is_required()
                {
                    return Err(malformed());
                }
                self.resources.insert(item.member().resource_id());
                self.members.push(item.member().clone());
            }
            "structure.exception" => {
                let item = SequenceException::from_document(document)?;
                let Shape::Sequence { frames, .. } = self.header.shape else {
                    return Err(malformed());
                };
                if item.representation_id() != self.header.owner
                    || item.position() != self.exceptions.len()
                    || self.exceptions.len() >= self.header.exception_count()
                    || !frames.contains(item.frame())
                    || self
                        .exceptions
                        .last()
                        .is_some_and(|last| *last >= item.frame())
                {
                    return Err(malformed());
                }
                self.exceptions.push(item.frame());
            }
            _ => return Err(unsupported()),
        }
        Ok(())
    }

    /// Returns a fully checked aggregate only after consuming its exact totals.
    ///
    /// # Errors
    /// Rejects failed/incomplete assemblers and invalid package/ordered membership.
    pub fn finish(self) -> Result<ContentStructure> {
        if self.failed
            || self.members.len() != self.header.member_count()
            || self.exceptions.len() != self.header.exception_count()
        {
            return Err(malformed());
        }
        match self.header.shape {
            Shape::Single(resource) => Ok(ContentStructure::single_resource(resource)),
            Shape::Sequence {
                resource,
                frames,
                rate,
                ..
            } => Ok(ContentStructure::image_sequence(checked(
                ImageSequenceDescriptor::new(resource, frames, rate, self.exceptions),
            )?)),
            Shape::Ordered(_) => checked(ContentStructure::ordered_parts(self.members)),
            Shape::Package(_) => checked(ContentStructure::package(self.members)),
        }
    }
}

/// Encodes a header followed by individual members or sorted sparse exceptions.
///
/// Each item can be framed and streamed independently; no aggregate JSON array
/// is built and no proposal-command limit applies.
///
/// # Errors
/// Rejects unsupported shapes or items outside the existing core limits.
pub fn encode_structure(
    representation: &Representation,
) -> Result<impl Iterator<Item = Result<Document>> + '_> {
    let header = StructureHeader::from_representation(representation)?;
    let owner = representation.id();
    let structure = representation.content_structure();
    let members =
        structure
            .members()
            .into_iter()
            .flatten()
            .enumerate()
            .map(move |(position, member)| {
                StructureMember::new(owner, position, member.clone()).map(|item| item.document())
            });
    let exceptions = structure
        .image_sequence_descriptor()
        .into_iter()
        .flat_map(ImageSequenceDescriptor::known_missing_frames)
        .enumerate()
        .map(move |(position, frame)| {
            SequenceException::new(owner, position, *frame).map(SequenceException::document)
        });
    Ok(std::iter::once(Ok(header.document()))
        .chain(members)
        .chain(exceptions))
}
