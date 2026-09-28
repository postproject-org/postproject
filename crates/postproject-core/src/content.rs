//! Structure and membership values for compound representations.

use std::collections::BTreeSet;

use crate::{Error, ErrorKind, RationalRate, ResourceId, Result};

/// Maximum encoded length of an extensible resource-role identifier.
pub const MAX_RESOURCE_ROLE_BYTES: usize = 128;
/// Maximum combined UTF-8 length of a sequence naming's prefix and suffix.
pub const MAX_SEQUENCE_NAMING_BYTES: usize = 1_024;
/// Maximum supported zero-padding width for an image-sequence frame number.
pub const MAX_FRAME_PADDING: u8 = 32;
/// Maximum number of sparse frame exceptions stored in one sequence descriptor.
pub const MAX_SEQUENCE_EXCEPTIONS: usize = 100_000;
/// Maximum number of materialized resource members in one content structure.
pub const MAX_CONTENT_MEMBERS: usize = 100_000;

/// An inclusive, regularly stepped frame domain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FrameRange {
    start: i64,
    end: i64,
    step: u32,
}

impl FrameRange {
    /// Creates an inclusive range whose end is aligned to its step.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for a reversed range, zero step,
    /// or an end frame not reachable from the start by whole steps.
    pub fn new(start: i64, end: i64, step: u32) -> Result<Self> {
        if step == 0 || end < start {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "frame range must be ascending with a non-zero step",
            ));
        }
        let distance = i128::from(end) - i128::from(start);
        if distance % i128::from(step) != 0 {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "frame range end must be aligned to its step",
            ));
        }
        Ok(Self { start, end, step })
    }

    /// Returns the first frame number.
    #[must_use]
    pub const fn start(self) -> i64 {
        self.start
    }

    /// Returns the last frame number.
    #[must_use]
    pub const fn end(self) -> i64 {
        self.end
    }

    /// Returns the positive frame-number increment.
    #[must_use]
    pub const fn step(self) -> u32 {
        self.step
    }

    /// Returns the exact number of frames in the regular domain.
    #[must_use]
    pub fn frame_count(self) -> u128 {
        let distance = i128::from(self.end) - i128::from(self.start);
        distance.unsigned_abs() / u128::from(self.step) + 1
    }

    /// Returns whether the frame belongs to the stepped domain.
    #[must_use]
    pub fn contains(self, frame: i64) -> bool {
        if frame < self.start || frame > self.end {
            return false;
        }
        let distance = i128::from(frame) - i128::from(self.start);
        distance % i128::from(self.step) == 0
    }
}

/// How the files of an image sequence are named in one place.
///
/// The prefix, suffix, and frame-number padding belong to a locator, not to
/// the sequence's content (ADR 0038): copies of one sequence may name their
/// files differently.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SequenceNaming {
    prefix: String,
    suffix: String,
    padding: u8,
}

impl SequenceNaming {
    /// Creates a bounded file naming without directory separators.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the naming is empty, too
    /// long, contains a path separator or NUL, or requests excessive padding.
    pub fn new(prefix: impl Into<String>, suffix: impl Into<String>, padding: u8) -> Result<Self> {
        let prefix = prefix.into();
        let suffix = suffix.into();
        let invalid_character = |character| matches!(character, '/' | '\\' | '\0');
        if (prefix.is_empty() && suffix.is_empty())
            || prefix.len().saturating_add(suffix.len()) > MAX_SEQUENCE_NAMING_BYTES
            || prefix.chars().any(invalid_character)
            || suffix.chars().any(invalid_character)
            || padding > MAX_FRAME_PADDING
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "invalid image-sequence file naming",
            ));
        }
        Ok(Self {
            prefix,
            suffix,
            padding,
        })
    }

    /// Returns the text before the frame number.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Returns the text after the frame number.
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Returns the minimum frame-number width.
    #[must_use]
    pub const fn padding(&self) -> u8 {
        self.padding
    }

    /// Formats the file name of `frame` without joining it to a directory.
    #[must_use]
    pub fn filename(&self, frame: i64) -> String {
        let frame = format!("{frame:0width$}", width = usize::from(self.padding));
        format!("{}{frame}{}", self.prefix, self.suffix)
    }
}

/// A compact description of one regular or sparse image sequence.
///
/// The descriptor holds what the sequence is: its frames, rate, and known
/// gaps. How its files are named is part of each locator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageSequenceDescriptor {
    resource_id: ResourceId,
    frames: FrameRange,
    rate: RationalRate,
    known_missing_frames: Vec<i64>,
}

impl ImageSequenceDescriptor {
    /// Creates a sequence descriptor and canonicalizes its sparse exceptions.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when there are too many missing
    /// frames or an exception does not belong to the regular frame domain.
    pub fn new(
        resource_id: ResourceId,
        frames: FrameRange,
        rate: RationalRate,
        mut known_missing_frames: Vec<i64>,
    ) -> Result<Self> {
        if known_missing_frames.len() > MAX_SEQUENCE_EXCEPTIONS {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("image sequence has more than {MAX_SEQUENCE_EXCEPTIONS} sparse exceptions"),
            ));
        }
        if known_missing_frames
            .iter()
            .any(|frame| !frames.contains(*frame))
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "missing image-sequence frame is outside the regular frame domain",
            ));
        }
        known_missing_frames.sort_unstable();
        known_missing_frames.dedup();
        Ok(Self {
            resource_id,
            frames,
            rate,
            known_missing_frames,
        })
    }

    /// Returns the compact patterned resource identity.
    #[must_use]
    pub const fn resource_id(&self) -> ResourceId {
        self.resource_id
    }

    /// Returns the regular frame domain before sparse exceptions.
    #[must_use]
    pub const fn frames(&self) -> FrameRange {
        self.frames
    }

    /// Returns the exact playback or capture rate.
    #[must_use]
    pub const fn rate(&self) -> RationalRate {
        self.rate
    }

    /// Returns sorted, unique frames known to be absent.
    #[must_use]
    pub fn known_missing_frames(&self) -> &[i64] {
        &self.known_missing_frames
    }

    /// Returns whether a frame is recorded as missing.
    #[must_use]
    pub fn is_known_missing(&self, frame: i64) -> bool {
        self.known_missing_frames.binary_search(&frame).is_ok()
    }
}

/// A namespaced, open-world role for a resource within a representation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceRole(String);

impl ResourceRole {
    /// Creates a role such as `org.postproject:essence` or `vendor:playlist`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the role is empty, too long,
    /// is not namespaced, or contains unsupported bytes.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let valid_bytes = value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'));
        let valid_namespace = value
            .split_once(':')
            .is_some_and(|(namespace, local)| !namespace.is_empty() && !local.is_empty());
        if value.len() > MAX_RESOURCE_ROLE_BYTES || !valid_bytes || !valid_namespace {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!(
                    "resource role must be a namespaced identifier of at most {MAX_RESOURCE_ROLE_BYTES} ASCII bytes"
                ),
            ));
        }
        Ok(Self(value))
    }

    /// Returns the exact role identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One resource's role and requiredness within a content structure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceMember {
    resource_id: ResourceId,
    role: ResourceRole,
    required: bool,
}

impl ResourceMember {
    /// Creates a resource membership.
    #[must_use]
    pub const fn new(resource_id: ResourceId, role: ResourceRole, required: bool) -> Self {
        Self {
            resource_id,
            role,
            required,
        }
    }

    /// Returns the participating resource.
    #[must_use]
    pub const fn resource_id(&self) -> ResourceId {
        self.resource_id
    }

    /// Returns the member's extensible semantic role.
    #[must_use]
    pub const fn role(&self) -> &ResourceRole {
        &self.role
    }

    /// Returns whether availability of this member is required for completeness.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }
}

/// The structural shape used to realize a representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ContentStructureKind {
    /// One concrete resource.
    SingleResource,
    /// One compact patterned image-sequence resource.
    ImageSequence,
    /// Several required resources consumed in stable order.
    OrderedParts,
    /// A role-bearing collection of required and optional resources.
    Package,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ContentStructureData {
    SingleResource(ResourceId),
    ImageSequence(ImageSequenceDescriptor),
    OrderedParts(Vec<ResourceMember>),
    Package(Vec<ResourceMember>),
}

/// A validated description of how resources realize one representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentStructure(ContentStructureData);

impl ContentStructure {
    /// Creates a representation backed by one required resource.
    #[must_use]
    pub const fn single_resource(resource_id: ResourceId) -> Self {
        Self(ContentStructureData::SingleResource(resource_id))
    }

    /// Creates a representation backed by one compact image sequence.
    #[must_use]
    pub const fn image_sequence(descriptor: ImageSequenceDescriptor) -> Self {
        Self(ContentStructureData::ImageSequence(descriptor))
    }

    /// Creates an ordered span whose members are all required.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the membership is empty,
    /// unbounded, duplicated, or contains an optional member.
    pub fn ordered_parts(members: Vec<ResourceMember>) -> Result<Self> {
        validate_members(&members)?;
        if members.iter().any(|member| !member.is_required()) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "ordered content parts must all be required",
            ));
        }
        Ok(Self(ContentStructureData::OrderedParts(members)))
    }

    /// Creates a package with at least one required member.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when the membership is empty,
    /// unbounded, duplicated, or has no required member.
    pub fn package(members: Vec<ResourceMember>) -> Result<Self> {
        validate_members(&members)?;
        if !members.iter().any(ResourceMember::is_required) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "a content package must have at least one required member",
            ));
        }
        Ok(Self(ContentStructureData::Package(members)))
    }

    /// Returns the structure discriminator.
    #[must_use]
    pub const fn kind(&self) -> ContentStructureKind {
        match self.0 {
            ContentStructureData::SingleResource(_) => ContentStructureKind::SingleResource,
            ContentStructureData::ImageSequence(_) => ContentStructureKind::ImageSequence,
            ContentStructureData::OrderedParts(_) => ContentStructureKind::OrderedParts,
            ContentStructureData::Package(_) => ContentStructureKind::Package,
        }
    }

    /// Returns the resource when this is a single-resource structure.
    #[must_use]
    pub const fn single_resource_id(&self) -> Option<ResourceId> {
        match self.0 {
            ContentStructureData::SingleResource(resource_id) => Some(resource_id),
            _ => None,
        }
    }

    /// Returns the descriptor when this is an image sequence.
    #[must_use]
    pub const fn image_sequence_descriptor(&self) -> Option<&ImageSequenceDescriptor> {
        match &self.0 {
            ContentStructureData::ImageSequence(descriptor) => Some(descriptor),
            _ => None,
        }
    }

    /// Returns members for an ordered or package structure.
    #[must_use]
    pub fn members(&self) -> Option<&[ResourceMember]> {
        match &self.0 {
            ContentStructureData::OrderedParts(members)
            | ContentStructureData::Package(members) => Some(members),
            _ => None,
        }
    }

    /// Returns referenced resources in structural order.
    #[must_use]
    pub fn resource_ids(&self) -> Vec<ResourceId> {
        match &self.0 {
            ContentStructureData::SingleResource(resource_id) => vec![*resource_id],
            ContentStructureData::ImageSequence(descriptor) => vec![descriptor.resource_id()],
            ContentStructureData::OrderedParts(members)
            | ContentStructureData::Package(members) => {
                members.iter().map(ResourceMember::resource_id).collect()
            }
        }
    }
}

fn validate_members(members: &[ResourceMember]) -> Result<()> {
    if members.is_empty() || members.len() > MAX_CONTENT_MEMBERS {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("content structure must have 1-{MAX_CONTENT_MEMBERS} members"),
        ));
    }
    let unique: BTreeSet<_> = members.iter().map(ResourceMember::resource_id).collect();
    if unique.len() != members.len() {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "content structure contains a duplicate resource",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_ranges_are_inclusive_and_stepped() {
        let frames = FrameRange::new(-2, 4, 2).expect("valid range");

        assert_eq!(frames.frame_count(), 4);
        assert!(frames.contains(-2));
        assert!(frames.contains(4));
        assert!(!frames.contains(1));
        assert!(FrameRange::new(1, 0, 1).is_err());
        assert!(FrameRange::new(0, 5, 2).is_err());
    }

    #[test]
    fn sequence_namings_format_frames_without_paths() {
        let naming = SequenceNaming::new("shot.", ".exr", 4).expect("valid naming");

        assert_eq!(naming.filename(12), "shot.0012.exr");
        assert_eq!(naming.filename(-2), "shot.-002.exr");
        assert!(SequenceNaming::new("directory/shot.", ".exr", 4).is_err());
        assert!(SequenceNaming::new("", "", 0).is_err());
    }

    #[test]
    fn sequence_descriptor_is_compact_and_canonical() {
        let frames = FrameRange::new(1_001, 1_010, 1).expect("valid range");
        let rate = RationalRate::new(24_000, 1_001).expect("valid rate");
        let descriptor = ImageSequenceDescriptor::new(
            ResourceId::new(),
            frames,
            rate,
            vec![1_007, 1_003, 1_007],
        )
        .expect("valid sequence");

        assert_eq!(descriptor.known_missing_frames(), [1_003, 1_007]);
        assert!(descriptor.is_known_missing(1_003));
        assert!(!descriptor.is_known_missing(1_004));
        assert!(ImageSequenceDescriptor::new(ResourceId::new(), frames, rate, vec![999],).is_err());
    }

    #[test]
    fn resource_roles_are_namespaced_and_open_world() {
        let standard = ResourceRole::new("org.postproject:essence").expect("valid role");
        let vendor = ResourceRole::new("example.camera:playlist-v2").expect("valid role");

        assert_eq!(standard.as_str(), "org.postproject:essence");
        assert_eq!(vendor.as_str(), "example.camera:playlist-v2");
        assert!(ResourceRole::new("essence").is_err());
        assert!(ResourceRole::new("vendor:").is_err());
        assert!(ResourceRole::new("vendor:side car").is_err());
    }

    #[test]
    fn membership_preserves_role_and_requiredness() {
        let resource_id = ResourceId::new();
        let role = ResourceRole::new("org.postproject:thumbnail").expect("valid role");
        let member = ResourceMember::new(resource_id, role, false);

        assert_eq!(member.resource_id(), resource_id);
        assert_eq!(member.role().as_str(), "org.postproject:thumbnail");
        assert!(!member.is_required());
    }

    #[test]
    fn compound_structures_enforce_membership_invariants() {
        let essence = ResourceRole::new("org.postproject:essence").expect("valid role");
        let thumbnail = ResourceRole::new("org.postproject:thumbnail").expect("valid role");
        let required = ResourceMember::new(ResourceId::new(), essence, true);
        let optional = ResourceMember::new(ResourceId::new(), thumbnail, false);

        let ordered =
            ContentStructure::ordered_parts(vec![required.clone()]).expect("valid ordered parts");
        let package = ContentStructure::package(vec![required.clone(), optional.clone()])
            .expect("valid package");

        assert_eq!(ordered.kind(), ContentStructureKind::OrderedParts);
        assert_eq!(package.members().expect("package members").len(), 2);
        assert_eq!(
            package.resource_ids(),
            vec![required.resource_id(), optional.resource_id()]
        );
        assert!(ContentStructure::ordered_parts(vec![optional.clone()]).is_err());
        assert!(ContentStructure::package(vec![optional]).is_err());
        assert!(ContentStructure::package(vec![required.clone(), required]).is_err());
    }
}
