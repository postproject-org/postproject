//! C-ABI-owned dependency observations.

use std::ffi::{CString, c_char};

use postproject_core::{
    Dependency, DependencyQueryMatch, DependencySet, DependencySetStatus, DependencyTarget, Error,
    ErrorKind, QueryCursor, RepresentationId,
};

use crate::{
    PP_OBJECT_ASSET, PP_OBJECT_REPRESENTATION, PpObjectRef, PpRepresentationId, PpUuid,
    exact_cstring,
};

/// Opaque immutable dependency observation owned by the C caller.
pub struct PpDependencySet {
    pub(crate) source_representation_id: RepresentationId,
    pub(crate) recorded_at_revision: u64,
    pub(crate) status: u32,
    pub(crate) present: bool,
    dependencies: Vec<AbiDependency>,
}

/// Opaque immutable page of dependency-query matches owned by the C caller.
pub struct PpDependencyQuerySet {
    matches: Vec<PpDependencyMatch>,
    next_cursor: Option<CString>,
    pub(crate) traversal_truncated: bool,
}

/// Fixed-layout dependency-query match borrowed from a query set.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpDependencyMatch {
    /// Floating asset or pinned/dependent representation identity.
    pub target: PpObjectRef,
    /// Shortest observed edge depth from the query root.
    pub depth: u32,
}

/// Borrowed dependency edge used for both input and output.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpDependency {
    /// Whether `source_resource_id` is present.
    pub has_source_resource: u8,
    /// Optional resource containing the authored reference.
    pub source_resource_id: PpUuid,
    /// Required NUL-terminated namespaced dependency kind.
    pub kind: *const c_char,
    /// Floating asset or pinned representation target.
    pub target: PpObjectRef,
    /// Whether `resolved_representation_id` is present.
    pub has_resolved_representation: u8,
    /// Representation selected for a floating target.
    pub resolved_representation_id: PpRepresentationId,
    /// Exactly zero or one.
    pub required: u8,
    /// Required NUL-terminated exact authored reference.
    pub authored_reference: *const c_char,
}

struct AbiDependency {
    source_resource_id: Option<PpUuid>,
    kind: CString,
    target: PpObjectRef,
    resolved_representation_id: Option<PpRepresentationId>,
    required: bool,
    authored_reference: CString,
}

impl PpDependencySet {
    pub(crate) fn new(
        source_representation_id: RepresentationId,
        set: Option<DependencySet>,
    ) -> Result<Self, Error> {
        let Some(set) = set else {
            return Ok(Self {
                source_representation_id,
                recorded_at_revision: 0,
                status: 0,
                present: false,
                dependencies: Vec::new(),
            });
        };
        let status = match set.status() {
            DependencySetStatus::Current => 1,
            DependencySetStatus::NeedsExtraction => 2,
            _ => 0,
        };
        let dependencies = set
            .dependencies()
            .iter()
            .map(AbiDependency::try_from)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            source_representation_id,
            recorded_at_revision: set.recorded_at_revision(),
            status,
            present: true,
            dependencies,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.dependencies.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<PpDependency> {
        self.dependencies.get(index).map(AbiDependency::as_abi)
    }
}

impl PpDependencyQuerySet {
    pub(crate) fn new(
        matches: &[DependencyQueryMatch],
        next_cursor: Option<&QueryCursor>,
        traversal_truncated: bool,
    ) -> Result<Self, Error> {
        let matches = matches
            .iter()
            .map(PpDependencyMatch::try_from)
            .collect::<Result<_, _>>()?;
        let next_cursor = next_cursor
            .map(|cursor| exact_cstring(cursor.as_str(), "query cursor"))
            .transpose()?;
        Ok(Self {
            matches,
            next_cursor,
            traversal_truncated,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.matches.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<PpDependencyMatch> {
        self.matches.get(index).copied()
    }

    pub(crate) fn next_cursor(&self) -> *const c_char {
        self.next_cursor
            .as_ref()
            .map_or(std::ptr::null(), |cursor| cursor.as_ptr())
    }
}

impl TryFrom<&DependencyQueryMatch> for PpDependencyMatch {
    type Error = Error;

    fn try_from(value: &DependencyQueryMatch) -> Result<Self, Self::Error> {
        let (kind, bytes) = match value.target() {
            DependencyTarget::Asset(id) => (PP_OBJECT_ASSET, id.into_bytes()),
            DependencyTarget::Representation(id) => (PP_OBJECT_REPRESENTATION, id.into_bytes()),
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "dependency query target is not supported by this ABI",
                ));
            }
        };
        Ok(Self {
            target: PpObjectRef {
                kind,
                id: PpUuid { bytes },
            },
            depth: value.depth(),
        })
    }
}

impl AbiDependency {
    fn as_abi(&self) -> PpDependency {
        PpDependency {
            has_source_resource: u8::from(self.source_resource_id.is_some()),
            source_resource_id: self.source_resource_id.unwrap_or(PpUuid { bytes: [0; 16] }),
            kind: self.kind.as_ptr(),
            target: self.target,
            has_resolved_representation: u8::from(self.resolved_representation_id.is_some()),
            resolved_representation_id: self
                .resolved_representation_id
                .unwrap_or(PpRepresentationId { bytes: [0; 16] }),
            required: u8::from(self.required),
            authored_reference: self.authored_reference.as_ptr(),
        }
    }
}

impl TryFrom<&Dependency> for AbiDependency {
    type Error = Error;

    fn try_from(dependency: &Dependency) -> Result<Self, Self::Error> {
        let (target_kind, target_id) = match dependency.target() {
            DependencyTarget::Asset(id) => (PP_OBJECT_ASSET, id.into_bytes()),
            DependencyTarget::Representation(id) => (PP_OBJECT_REPRESENTATION, id.into_bytes()),
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "dependency target is not supported by this ABI",
                ));
            }
        };
        Ok(Self {
            source_resource_id: dependency.source_resource_id().map(|id| PpUuid {
                bytes: id.into_bytes(),
            }),
            kind: exact_cstring(dependency.kind().as_str(), "dependency kind")?,
            target: PpObjectRef {
                kind: target_kind,
                id: PpUuid { bytes: target_id },
            },
            resolved_representation_id: dependency.resolved_representation_id().map(|id| {
                PpRepresentationId {
                    bytes: id.into_bytes(),
                }
            }),
            required: dependency.is_required(),
            authored_reference: exact_cstring(
                dependency.authored_reference(),
                "authored dependency reference",
            )?,
        })
    }
}
