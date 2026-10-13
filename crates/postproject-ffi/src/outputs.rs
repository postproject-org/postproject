//! Atomic staging of produced media, its activity and actual parameters.

mod recipe;

use std::ffi::c_char;

use crate::{
    PpActivityEdge, PpActivityId, PpAssetId, PpError, PpMediaSource, PpMetadataInput,
    PpRepresentationId, PpTransaction, StagedMutation, ffi_call, initialize_value,
    invalid_argument, require_output,
};
use postproject_core::{Activity, ActivityId, ActivityOutput, ObjectRef};
use postproject_media::OutputIdentity;

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

/// Stages an activity describing existing output representations and its parameters.
///
/// # Safety
/// Handles must be live; recipe/arrays readable for their bounded counts,
/// output writable and error null or writable. All strings are borrowed UTF-8.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_create_activity(
    transaction: *mut PpTransaction,
    recipe: *const PpActivityRecipe,
    outputs: *const PpActivityEdge,
    output_count: u64,
    out_activity_id: *mut PpActivityId,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Borrowed values are checked and copied before modifying the edit.
    unsafe {
        initialize_value(out_activity_id, PpActivityId { bytes: [0; 16] });
        ffi_call(out_error, || {
            require_output(out_activity_id, "out_activity_id")?;
            let edit = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            edit.lifecycle.ensure_open()?;
            let recipe = recipe::read(recipe)?;
            let outputs = crate::activity_edges_from_abi(outputs, output_count, "output")?
                .into_iter()
                .map(|(id, role)| ActivityOutput::new(id, role))
                .collect();
            let mut activity =
                Activity::new(ActivityId::new(), recipe.kind, recipe.inputs, outputs)?
                    .with_timing(recipe.started_at, recipe.finished_at)?;
            if let Some(tool) = recipe.tool {
                activity = activity.with_tool(tool);
            }
            if let Some(agent) = recipe.agent {
                activity = activity.with_agent(agent);
            }
            let id = activity.id();
            append_activity(edit, activity, recipe.parameters)?;
            out_activity_id.write(PpActivityId {
                bytes: id.into_bytes(),
            });
            Ok(())
        })
    }
}

fn append_activity(
    edit: &mut PpTransaction,
    activity: Activity,
    parameters: Vec<(
        postproject_core::MetadataProperty,
        postproject_core::MetadataValue,
    )>,
) -> Result<(), postproject_core::Error> {
    let target = ObjectRef::Activity(activity.id());
    let mut additions = Vec::with_capacity(parameters.len() + 1);
    additions.push(StagedMutation::Activity(activity));
    additions.extend(
        parameters
            .into_iter()
            .map(|(key, value)| StagedMutation::AddMetadataValue(target, key, value)),
    );
    edit.mutations
        .try_reserve(additions.len())
        .map_err(|error| {
            postproject_core::Error::new(
                postproject_core::ErrorKind::Internal,
                format!("allocate staged activity: {error}"),
            )
        })?;
    edit.mutations.extend(additions);
    Ok(())
}
