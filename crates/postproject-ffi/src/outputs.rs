//! Borrowed output task inputs and copied identities.

use crate::{PpActivityEdge, PpAssetId, PpMediaSource, PpMetadataInput, PpRepresentationId};
use postproject_media::OutputIdentity;
use std::ffi::c_char;

/// Borrowed typed assertion on the actual producing activity.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpActivityParameter {
    /// Vocabulary identity.
    pub vocabulary: *const c_char,
    /// Property identity within that vocabulary.
    pub property: *const c_char,
    /// Live metadata input, copied during staging.
    pub value: *const PpMetadataInput,
}

/// Borrowed attribution and repeated parameters of a completed operation.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpActivityRecipe {
    /// Extensible activity kind.
    pub kind: *const c_char,
    /// Input edges, nullable only for zero count.
    pub inputs: *const PpActivityEdge,
    /// Number of readable input edges.
    pub input_count: u64,
    /// Optional actual start time.
    pub started_at_unix_micros: *const i64,
    /// Optional actual finish time.
    pub finished_at_unix_micros: *const i64,
    /// Optional tool name.
    pub tool_name: *const c_char,
    /// Optional tool version; requires name.
    pub tool_version: *const c_char,
    /// Optional tool URI; requires name.
    pub tool_uri: *const c_char,
    /// Optional responsible agent name.
    pub agent_name: *const c_char,
    /// Optional agent identifier scheme, paired with value.
    pub agent_identifier_scheme: *const c_char,
    /// Optional opaque agent identifier value.
    pub agent_identifier_value: *const c_char,
    /// Optional agent identifier qualifier.
    pub agent_identifier_qualifier: *const c_char,
    /// Parameters, nullable only for zero count.
    pub parameters: *const PpActivityParameter,
    /// Number of readable parameter assertions.
    pub parameter_count: u64,
}

/// Borrowed file or compound output and its explicit identity choice.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpOutputSource {
    /// Existing asset, or null to create a new original asset.
    pub asset_id: *const PpAssetId,
    /// Representation kind; a new asset requires original.
    pub kind: u32,
    /// Direct single-file path, mutually exclusive with source.
    pub path: *const c_char,
    /// Existing compound source, mutually exclusive with path.
    pub source: *const PpMediaSource,
    /// Optional output edge role.
    pub role: *const c_char,
    /// Optional display name, only for a new asset.
    pub display_name: *const c_char,
}

/// Copied staged asset and representation identities.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpOutputIdentity {
    /// Explicitly selected or newly created logical asset.
    pub asset_id: PpAssetId,
    /// Newly produced representation.
    pub representation_id: PpRepresentationId,
}

impl From<OutputIdentity> for PpOutputIdentity {
    fn from(value: OutputIdentity) -> Self {
        Self {
            asset_id: PpAssetId {
                bytes: value.asset_id.into_bytes(),
            },
            representation_id: PpRepresentationId {
                bytes: value.representation_id.into_bytes(),
            },
        }
    }
}
