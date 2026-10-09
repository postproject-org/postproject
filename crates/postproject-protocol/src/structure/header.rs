use postproject_core::{
    ContentStructureKind, FrameRange, MAX_CONTENT_MEMBERS, MAX_SEQUENCE_EXCEPTIONS, RationalRate,
    Representation, RepresentationId, ResourceId,
};
use serde_json::{Value, json};

use crate::{
    Document, Result,
    fields::{checked, exact, limit, malformed, object, text, unsupported},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Shape {
    Single(ResourceId),
    Sequence {
        resource: ResourceId,
        frames: FrameRange,
        rate: RationalRate,
        exceptions: usize,
    },
    Ordered(usize),
    Package(usize),
}

/// A representation's content shape and exact continuation totals.
///
/// Members and sparse exceptions are separate frames, rather than header arrays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructureHeader {
    pub(super) owner: RepresentationId,
    pub(super) shape: Shape,
}

impl StructureHeader {
    /// Copies the shape and totals without cloning aggregate collections.
    ///
    /// # Errors
    /// Rejects future core shapes without a defined wire representation.
    pub fn from_representation(representation: &Representation) -> Result<Self> {
        let structure = representation.content_structure();
        let shape = match structure.kind() {
            ContentStructureKind::SingleResource => {
                Shape::Single(structure.single_resource_id().ok_or_else(malformed)?)
            }
            ContentStructureKind::ImageSequence => {
                let sequence = structure
                    .image_sequence_descriptor()
                    .ok_or_else(malformed)?;
                Shape::Sequence {
                    resource: sequence.resource_id(),
                    frames: sequence.frames(),
                    rate: sequence.rate(),
                    exceptions: sequence.known_missing_frames().len(),
                }
            }
            ContentStructureKind::OrderedParts => {
                Shape::Ordered(structure.members().ok_or_else(malformed)?.len())
            }
            ContentStructureKind::Package => {
                Shape::Package(structure.members().ok_or_else(malformed)?.len())
            }
            _ => return Err(unsupported()),
        };
        Ok(Self {
            owner: representation.id(),
            shape,
        })
    }

    /// Returns the owning representation identity.
    #[must_use]
    pub const fn representation_id(self) -> RepresentationId {
        self.owner
    }

    /// Returns the structural discriminator.
    #[must_use]
    pub const fn kind(self) -> ContentStructureKind {
        match self.shape {
            Shape::Single(_) => ContentStructureKind::SingleResource,
            Shape::Sequence { .. } => ContentStructureKind::ImageSequence,
            Shape::Ordered(_) => ContentStructureKind::OrderedParts,
            Shape::Package(_) => ContentStructureKind::Package,
        }
    }

    /// Returns the exact number of following member frames.
    #[must_use]
    pub const fn member_count(self) -> usize {
        match self.shape {
            Shape::Ordered(count) | Shape::Package(count) => count,
            _ => 0,
        }
    }

    /// Returns the exact number of following sparse exception frames.
    #[must_use]
    pub const fn exception_count(self) -> usize {
        match self.shape {
            Shape::Sequence { exceptions, .. } => exceptions,
            _ => 0,
        }
    }

    /// Encodes one bounded header without member or exception arrays.
    #[must_use]
    pub fn document(self) -> Document {
        let content = match self.shape {
            Shape::Single(resource) => {
                json!({"shape":"single", "resource_id":resource.to_string()})
            }
            Shape::Sequence {
                resource,
                frames,
                rate,
                exceptions,
            } => {
                json!({"shape":"image_sequence", "resource_id":resource.to_string(), "frames":{"start":frames.start().to_string(),"end":frames.end().to_string(),"step":frames.step().to_string()}, "rate":{"numerator":rate.numerator().to_string(),"denominator":rate.denominator().to_string()}, "exception_count":exceptions.to_string()})
            }
            Shape::Ordered(count) => {
                json!({"shape":"ordered_parts", "member_count":count.to_string()})
            }
            Shape::Package(count) => json!({"shape":"package", "member_count":count.to_string()}),
        };
        Document {
            value: json!({"kind":"structure.header", "representation_id":self.owner.to_string(), "content":content}),
        }
    }

    /// Decodes checked shape facts and bounded continuation totals.
    ///
    /// # Errors
    /// Rejects unknown fields/shapes, invalid domains/rates and excessive totals.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "representation_id", "content"])?;
        if text(&fields["kind"])? != "structure.header" {
            return Err(unsupported());
        }
        Ok(Self {
            owner: exact(&fields["representation_id"])?,
            shape: decode_shape(&fields["content"])?,
        })
    }
}

fn decode_shape(value: &Value) -> Result<Shape> {
    let shape = text(value.get("shape").ok_or_else(malformed)?)?;
    Ok(match shape {
        "single" => {
            let fields = object(value, &["shape", "resource_id"])?;
            Shape::Single(exact(&fields["resource_id"])?)
        }
        "image_sequence" => {
            let fields = object(
                value,
                &["shape", "resource_id", "frames", "rate", "exception_count"],
            )?;
            let frames = object(&fields["frames"], &["start", "end", "step"])?;
            let rate = object(&fields["rate"], &["numerator", "denominator"])?;
            Shape::Sequence {
                resource: exact(&fields["resource_id"])?,
                frames: checked(FrameRange::new(
                    exact(&frames["start"])?,
                    exact(&frames["end"])?,
                    exact(&frames["step"])?,
                ))?,
                rate: checked(RationalRate::new(
                    exact(&rate["numerator"])?,
                    exact(&rate["denominator"])?,
                ))?,
                exceptions: count(&fields["exception_count"], MAX_SEQUENCE_EXCEPTIONS, true)?,
            }
        }
        "ordered_parts" | "package" => {
            let fields = object(value, &["shape", "member_count"])?;
            let count = count(&fields["member_count"], MAX_CONTENT_MEMBERS, false)?;
            if shape == "package" {
                Shape::Package(count)
            } else {
                Shape::Ordered(count)
            }
        }
        _ => return Err(unsupported()),
    })
}

fn count(value: &Value, max: usize, zero: bool) -> Result<usize> {
    let count: u64 = exact(value)?;
    if count == 0 && !zero {
        return Err(malformed());
    }
    let count = usize::try_from(count).map_err(|_| limit())?;
    if count > max {
        return Err(limit());
    }
    Ok(count)
}
