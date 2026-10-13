//! Prepare produced media and its actual recipe before opening a write.

mod staging;

use std::collections::BTreeSet;

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityRole, AgentIdentity,
    AssetId, Error, ErrorKind, MAX_ACTIVITY_EDGES, MAX_METADATA_COLLECTION_ITEMS, MetadataProperty,
    MetadataValue, OriginalMediaImport, RepresentationId, RepresentationImport, RepresentationKind,
    Result, Timestamp, ToolIdentity,
};

use crate::{MediaSource, prepare_original_media, prepare_representation};

/// One produced source, with an explicit choice of logical identity.
#[derive(Clone, Debug)]
pub struct OutputSource {
    /// Existing asset, or absent to create a new original asset.
    pub asset_id: Option<AssetId>,
    /// New assets require `Original`; existing assets accept every kind.
    pub kind: RepresentationKind,
    /// File or compound content whose bytes were already produced by the host.
    pub source: MediaSource,
    /// Optional role on this activity's output edge.
    pub role: Option<ActivityRole>,
    /// Optional display name, only for a new asset.
    pub display_name: Option<String>,
}

impl OutputSource {
    /// Describes produced content without inspecting or writing it.
    #[must_use]
    pub fn new(
        asset_id: Option<AssetId>,
        kind: RepresentationKind,
        source: impl Into<MediaSource>,
    ) -> Self {
        Self {
            asset_id,
            kind,
            source: source.into(),
            role: None,
            display_name: None,
        }
    }
}

/// Actual attribution and repeated typed parameters of one completed operation.
#[derive(Clone, Debug)]
pub struct OutputRecipe {
    /// Extensible activity kind.
    pub kind: ActivityKind,
    /// Consumed representations, with optional roles.
    pub inputs: Vec<ActivityInput>,
    /// Actual start time, when known.
    pub started_at: Option<Timestamp>,
    /// Actual finish time, when known.
    pub finished_at: Option<Timestamp>,
    /// Tool that produced the content, when known.
    pub tool: Option<ToolIdentity>,
    /// Responsible agent, when known.
    pub agent: Option<AgentIdentity>,
    /// Ordered assertions attached to the producing activity, not its outputs.
    pub parameters: Vec<(MetadataProperty, MetadataValue)>,
}

impl OutputRecipe {
    /// Starts a recipe; bare representation IDs have no role.
    /// # Errors
    /// Rejects excessive or duplicate input edges before allocating a recipe.
    pub fn new<I: Into<ActivityInput>>(
        kind: ActivityKind,
        inputs: impl IntoIterator<Item = I>,
    ) -> Result<Self> {
        let mut collected = Vec::new();
        for input in inputs {
            if collected.len() == MAX_ACTIVITY_EDGES {
                return Err(Error::new(
                    ErrorKind::InvalidArgument,
                    "too many activity inputs",
                ));
            }
            collected.push(input.into());
        }
        validate_inputs(&collected)?;
        Ok(Self {
            kind,
            inputs: collected,
            started_at: None,
            finished_at: None,
            tool: None,
            agent: None,
            parameters: Vec::new(),
        })
    }
}

/// Copied identities retained by a host after successful publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputIdentity {
    /// Logical media identity, new or explicitly selected.
    pub asset_id: AssetId,
    /// Concrete representation produced by this operation.
    pub representation_id: RepresentationId,
}

/// A prepared output on a new or existing logical asset.
#[derive(Clone, Debug)]
pub enum PreparedOutput {
    /// New original asset and its content.
    Original(OriginalMediaImport),
    /// New representation of an explicitly selected existing asset.
    Representation(RepresentationImport),
}

impl PreparedOutput {
    /// Copies the staged identities; they do not exist durably until commit.
    #[must_use]
    pub fn identity(&self) -> OutputIdentity {
        let representation = match self {
            Self::Original(value) => value.representation(),
            Self::Representation(value) => value.representation(),
        };
        OutputIdentity {
            asset_id: representation.asset_id(),
            representation_id: representation.id(),
        }
    }
}

/// Filesystem-prepared outputs and the one actual producing activity.
#[derive(Clone, Debug)]
pub struct PreparedOutputs {
    outputs: Vec<PreparedOutput>,
    activity: Activity,
    parameters: Vec<(MetadataProperty, MetadataValue)>,
}

/// Prepares every output before any production write; performs explicit media I/O.
///
/// No activity input implies an authored dependency, and no job is created.
/// # Errors
/// Rejects invalid counts, new-asset kinds, timing or edges and media I/O errors.
pub fn prepare_outputs(outputs: &[OutputSource], recipe: OutputRecipe) -> Result<PreparedOutputs> {
    if outputs.is_empty()
        || outputs.len() > MAX_ACTIVITY_EDGES
        || recipe.parameters.len() > MAX_METADATA_COLLECTION_ITEMS
    {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "invalid output or parameter count",
        ));
    }
    validate_inputs(&recipe.inputs)?;
    if recipe
        .started_at
        .zip(recipe.finished_at)
        .is_some_and(|(start, finish)| finish < start)
    {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "activity finish time must not precede its start time",
        ));
    }
    for output in outputs {
        if (output.asset_id.is_none() && output.kind != RepresentationKind::Original)
            || (output.asset_id.is_some() && output.display_name.is_some())
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "new assets require original media; display names require a new asset",
            ));
        }
    }
    let prepared: Vec<_> = outputs
        .iter()
        .map(|output| match output.asset_id {
            Some(asset) => prepare_representation(asset, output.kind, output.source.clone())
                .map(PreparedOutput::Representation),
            None => {
                prepare_original_media(output.source.clone(), output.display_name.clone(), None)
                    .map(PreparedOutput::Original)
            }
        })
        .collect::<Result<_>>()?;
    let edges = prepared
        .iter()
        .zip(outputs)
        .map(|(prepared, source)| {
            ActivityOutput::new(prepared.identity().representation_id, source.role.clone())
        })
        .collect();
    let mut activity = Activity::new(ActivityId::new(), recipe.kind, recipe.inputs, edges)?
        .with_timing(recipe.started_at, recipe.finished_at)?;
    if let Some(tool) = recipe.tool {
        activity = activity.with_tool(tool);
    }
    if let Some(agent) = recipe.agent {
        activity = activity.with_agent(agent);
    }
    Ok(PreparedOutputs {
        outputs: prepared,
        activity,
        parameters: recipe.parameters,
    })
}

fn validate_inputs(inputs: &[ActivityInput]) -> Result<()> {
    if inputs.len() > MAX_ACTIVITY_EDGES
        || inputs.iter().collect::<BTreeSet<_>>().len() != inputs.len()
    {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "excessive or duplicate activity input edges",
        ));
    }
    Ok(())
}

impl PreparedOutputs {
    /// Copies all output identities in source order.
    #[must_use]
    pub fn identities(&self) -> Vec<OutputIdentity> {
        self.outputs.iter().map(PreparedOutput::identity).collect()
    }

    /// Returns the actual producing activity, including its attribution.
    #[must_use]
    pub const fn activity(&self) -> &Activity {
        &self.activity
    }

    /// Splits prepared values for adapters that stage their own mutation buffer.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        Vec<PreparedOutput>,
        Activity,
        Vec<(MetadataProperty, MetadataValue)>,
    ) {
        (self.outputs, self.activity, self.parameters)
    }
}
