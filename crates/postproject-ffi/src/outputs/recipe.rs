//! Convert a completely borrowed recipe before media inspection or staging.

use super::PpActivityRecipe;
use crate::{invalid_argument, optional_utf8, required_utf8};
use postproject_core::{
    ActivityInput, ActivityKind, AgentIdentity, Error, ExternalIdentifier, IdentifierScheme,
    MAX_METADATA_COLLECTION_ITEMS, Timestamp, ToolIdentity,
};
use postproject_media::OutputRecipe;

pub(super) unsafe fn read(recipe: *const PpActivityRecipe) -> Result<OutputRecipe, Error> {
    // SAFETY: Caller supplies a readable recipe with live borrowed handles/strings.
    unsafe {
        let recipe = recipe
            .as_ref()
            .ok_or_else(|| invalid_argument("recipe must not be null"))?;
        let inputs = crate::activity_edges_from_abi(recipe.inputs, recipe.input_count, "input")?
            .into_iter()
            .map(|(id, role)| ActivityInput::new(id, role));
        let mut result = OutputRecipe::new(
            ActivityKind::new(required_utf8(recipe.kind, "kind")?)?,
            inputs,
        )?;
        result.started_at = recipe
            .started_at_unix_micros
            .as_ref()
            .copied()
            .map(Timestamp::from_unix_micros);
        result.finished_at = recipe
            .finished_at_unix_micros
            .as_ref()
            .copied()
            .map(Timestamp::from_unix_micros);
        let name = optional_utf8(recipe.tool_name, "tool_name")?;
        let version = optional_utf8(recipe.tool_version, "tool_version")?;
        let uri = optional_utf8(recipe.tool_uri, "tool_uri")?;
        if let Some(name) = name {
            result.tool = Some(ToolIdentity::new(
                name,
                version.map(str::to_owned),
                uri.map(str::to_owned),
            )?);
        } else if version.is_some() || uri.is_some() {
            return Err(invalid_argument("tool version and URI require a tool name"));
        }
        let name = optional_utf8(recipe.agent_name, "agent_name")?;
        let scheme = optional_utf8(recipe.agent_identifier_scheme, "agent_identifier_scheme")?;
        let value = optional_utf8(recipe.agent_identifier_value, "agent_identifier_value")?;
        let qualifier = optional_utf8(
            recipe.agent_identifier_qualifier,
            "agent_identifier_qualifier",
        )?;
        let identifier = match (scheme, value) {
            (Some(scheme), Some(value)) => Some(ExternalIdentifier::new(
                IdentifierScheme::new(scheme)?,
                value,
                qualifier.map(str::to_owned),
            )?),
            (None, None) if qualifier.is_none() => None,
            _ => {
                return Err(invalid_argument(
                    "agent identifier scheme and value must be supplied together",
                ));
            }
        };
        if name.is_some() || identifier.is_some() {
            result.agent = Some(AgentIdentity::new(name.map(str::to_owned), identifier)?);
        }
        let count = usize::try_from(recipe.parameter_count)
            .map_err(|_| invalid_argument("parameter count is too large"))?;
        if count > MAX_METADATA_COLLECTION_ITEMS {
            return Err(invalid_argument("too many activity parameters"));
        }
        if count != 0 {
            if recipe.parameters.is_null() {
                return Err(invalid_argument("parameters must not be null"));
            }
            for parameter in std::slice::from_raw_parts(recipe.parameters, count) {
                let value = parameter
                    .value
                    .as_ref()
                    .ok_or_else(|| invalid_argument("parameter value must not be null"))?;
                result.parameters.push((
                    crate::metadata_property_from_abi(parameter.vocabulary, parameter.property)?,
                    value.value.clone(),
                ));
            }
        }
        Ok(result)
    }
}
