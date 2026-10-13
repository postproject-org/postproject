//! CLI adapter for already-produced output registration.

use super::{
    ActivityEdgeArg, MetadataValueInput, RepresentationKindArg, begin_cli_transaction, json_input,
    print_json_with_receipt, representation_source, set_cli_revision_context,
};
use anyhow::{Context, Result, bail};
use clap::Args;
use postproject_core::{
    ActivityInput, ActivityKind, ActivityRole, AgentIdentity, AssetId, DecisionBase,
    ExternalIdentifier, IdentifierScheme, MetadataProperty, PropertyId, RepresentationKind,
    Timestamp, ToolIdentity, VocabularyId,
};
use postproject_media::MediaSource;
use postproject_media::{OutputRecipe, OutputSource, prepare_outputs};
use postproject_storage_sqlite::SqliteProduction;
use serde::Deserialize;
use std::{path::PathBuf, str::FromStr};

#[derive(Debug, Args)]
pub(crate) struct RegisterArgs {
    production: PathBuf,
    /// Already-produced files; each becomes one output representation.
    #[arg(
        required_unless_present = "source_spec",
        conflicts_with = "source_spec"
    )]
    paths: Vec<PathBuf>,
    /// Existing tagged source specification for one compound output.
    #[arg(long)]
    source_spec: Option<PathBuf>,
    /// Existing logical asset; omit to create new original assets explicitly.
    #[arg(long)]
    asset: Option<String>,
    #[arg(long, value_enum, default_value = "original")]
    kind: RepresentationKindArg,
    /// Actual completed operation, such as org.example:render.
    #[arg(long)]
    activity_kind: String,
    /// Consumed representation, optionally followed by =ROLE.
    #[arg(long = "input")]
    inputs: Vec<ActivityEdgeArg>,
    /// Role applied to these output edges.
    #[arg(long)]
    output_role: Option<String>,
    #[arg(long)]
    tool_name: Option<String>,
    #[arg(long)]
    tool_version: Option<String>,
    #[arg(long)]
    tool_uri: Option<String>,
    #[arg(long)]
    agent_name: Option<String>,
    #[arg(long)]
    agent_identifier_scheme: Option<String>,
    #[arg(long)]
    agent_identifier_value: Option<String>,
    #[arg(long)]
    agent_identifier_qualifier: Option<String>,
    #[arg(long)]
    started_at_unix_micros: Option<i64>,
    #[arg(long)]
    finished_at_unix_micros: Option<i64>,
    /// Exact actual parameters as a JSON array of vocabulary/property/value entries.
    #[arg(long)]
    parameters: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameter {
    vocabulary: String,
    property: String,
    value: MetadataValueInput,
}

#[derive(Deserialize)]
struct Parameters(#[serde(deserialize_with = "json_input::collection")] Vec<Parameter>);

pub(crate) fn register(args: RegisterArgs, json: bool, base: Option<DecisionBase>) -> Result<()> {
    let asset = args.asset.as_deref().map(AssetId::from_str).transpose()?;
    let kind = match args.kind {
        RepresentationKindArg::Original => RepresentationKind::Original,
        RepresentationKindArg::Proxy => RepresentationKind::Proxy,
        RepresentationKindArg::Optimized => RepresentationKind::Optimized,
        RepresentationKindArg::Derived => RepresentationKind::Derived,
    };
    let sources = if let Some(path) = args.source_spec {
        vec![representation_source(json_input::read(&path)?)?]
    } else {
        args.paths.into_iter().map(MediaSource::File).collect()
    };
    let role = args.output_role.map(ActivityRole::new).transpose()?;
    let outputs: Vec<_> = sources
        .into_iter()
        .map(|source| {
            let mut output = OutputSource::new(asset, kind, source);
            output.role.clone_from(&role);
            output
        })
        .collect();
    let mut recipe = OutputRecipe::new(
        ActivityKind::new(args.activity_kind)?,
        args.inputs
            .into_iter()
            .map(|edge| ActivityInput::new(edge.representation_id, edge.role)),
    )?;
    recipe.started_at = args.started_at_unix_micros.map(Timestamp::from_unix_micros);
    recipe.finished_at = args
        .finished_at_unix_micros
        .map(Timestamp::from_unix_micros);
    if let Some(name) = args.tool_name {
        recipe.tool = Some(ToolIdentity::new(name, args.tool_version, args.tool_uri)?);
    } else if args.tool_version.is_some() || args.tool_uri.is_some() {
        bail!("--tool-version and --tool-uri require --tool-name");
    }
    let identifier = match (args.agent_identifier_scheme, args.agent_identifier_value) {
        (Some(scheme), Some(value)) => Some(ExternalIdentifier::new(
            IdentifierScheme::new(scheme)?,
            value,
            args.agent_identifier_qualifier,
        )?),
        (None, None) if args.agent_identifier_qualifier.is_none() => None,
        _ => bail!("agent identifier requires both scheme and value"),
    };
    if args.agent_name.is_some() || identifier.is_some() {
        recipe.agent = Some(AgentIdentity::new(args.agent_name, identifier)?);
    }
    if let Some(path) = args.parameters {
        let Parameters(parameters) = json_input::read(&path)?;
        recipe.parameters = parameters
            .into_iter()
            .map(|parameter| {
                Ok((
                    MetadataProperty::new(
                        VocabularyId::new(parameter.vocabulary)?,
                        PropertyId::new(parameter.property)?,
                    ),
                    parameter.value.into_value()?,
                ))
            })
            .collect::<Result<_>>()?;
    }
    // Media inspection finishes before acquiring a transaction's write lock.
    let prepared = prepare_outputs(&outputs, recipe).context("prepare produced outputs")?;
    let identities = prepared.identities();
    let activity = prepared.activity().id();
    let mut store = SqliteProduction::open(args.production).context("open production")?;
    let mut edit = begin_cli_transaction(&mut store, base, "output registration")?;
    set_cli_revision_context(&mut edit, "Register produced outputs")?;
    let mut edit = prepared.stage(edit).context("stage produced outputs")?;
    let receipt = edit.commit().context("commit produced outputs")?;
    if json {
        print_json_with_receipt(
            &serde_json::json!({"activity_id": activity.to_string(),
            "outputs": identities.iter().map(|identity| serde_json::json!({
                "asset_id": identity.asset_id.to_string(),
                "representation_id": identity.representation_id.to_string()
            })).collect::<Vec<_>>()}),
            &receipt,
        )
    } else {
        println!("recorded activity {activity}");
        for identity in identities {
            println!("{} {}", identity.asset_id, identity.representation_id);
        }
        Ok(())
    }
}
