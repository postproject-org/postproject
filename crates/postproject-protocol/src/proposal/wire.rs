use postproject_core::{DecisionBase, OriginIdentity, RevisionContext};
use serde_json::{Value, json};

use crate::{
    ClientId, Document, Extensions, Proposal, ProtocolBase, RequestId, Result, Scope, command,
    fields::{array, checked, exact, malformed, object, text, unsupported},
    proposal::MAX_PROPOSAL_COMMANDS,
};

pub(super) fn encode(proposal: &Proposal) -> Result<Value> {
    let base = proposal.base().map(|base| json!({
        "production":base.scope().production().to_string(),"history":base.scope().history().to_string(),
        "revision":base.decision().revision_id().map(|id| id.to_string()),"sequence":base.decision().sequence().to_string()
    }));
    let origin = proposal
        .context()
        .origin()
        .map(|origin| json!({"name":origin.name(),"version":origin.version(),"uri":origin.uri()}));
    Ok(json!({
        "kind":"proposal","version":"1","required_features":proposal.required_features().into_iter().map(crate::RecordFeature::wire_name).collect::<Vec<_>>(),
        "production":proposal.scope().production().to_string(),"history":proposal.scope().history().to_string(),
        "client":proposal.client().to_string(),"request":proposal.request().to_string(),
        "base":base,"origin":origin,"message":proposal.context().message(),
        "commands":proposal.commands().iter().map(command::encode).collect::<Result<Vec<_>>>()?,
        "extensions":proposal.extensions().document().value
    }))
}

fn nullable<T>(value: &Value, decode: impl FnOnce(&Value) -> Result<T>) -> Result<Option<T>> {
    if value.is_null() {
        Ok(None)
    } else {
        decode(value).map(Some)
    }
}

pub(super) fn decode(value: &Value) -> Result<Proposal> {
    let fields = object(
        value,
        &[
            "kind",
            "version",
            "required_features",
            "production",
            "history",
            "client",
            "request",
            "base",
            "origin",
            "message",
            "commands",
            "extensions",
        ],
    )?;
    if text(&fields["kind"])? != "proposal" {
        return Err(malformed());
    }
    if text(&fields["version"])? != "1" {
        return Err(unsupported());
    }
    let features = array(&fields["required_features"], 64)?;
    let mut advertised = Vec::with_capacity(features.len());
    for feature in features {
        let name = text(feature)?;
        if !matches!(
            name,
            "dependencies.v1" | "media.v1" | "metadata.v1" | "provenance.v1"
        ) {
            return Err(unsupported());
        }
        if advertised.last().is_some_and(|previous| *previous >= name) {
            return Err(malformed());
        }
        advertised.push(name);
    }
    let scope = Scope::new(exact(&fields["production"])?, exact(&fields["history"])?);
    let client: ClientId = exact(&fields["client"])?;
    let request: RequestId = exact(&fields["request"])?;
    let base = nullable(&fields["base"], |value| {
        let fields = object(value, &["production", "history", "revision", "sequence"])?;
        let base_scope = Scope::new(exact(&fields["production"])?, exact(&fields["history"])?);
        let decision = checked(DecisionBase::new(
            base_scope.production(),
            nullable(&fields["revision"], exact)?,
            exact(&fields["sequence"])?,
        ))?;
        ProtocolBase::new(base_scope, decision)
    })?;
    let origin = nullable(&fields["origin"], |value| {
        let fields = object(value, &["name", "version", "uri"])?;
        checked(OriginIdentity::new(
            text(&fields["name"])?,
            nullable(&fields["version"], |v| Ok(text(v)?.to_owned()))?,
            nullable(&fields["uri"], |v| Ok(text(v)?.to_owned()))?,
        ))
    })?;
    let context = checked(RevisionContext::new(
        origin,
        nullable(&fields["message"], |v| Ok(text(v)?.to_owned()))?,
    ))?;
    let commands = array(&fields["commands"], MAX_PROPOSAL_COMMANDS)?
        .iter()
        .map(command::decode)
        .collect::<Result<_>>()?;
    let extensions = Extensions::new(Document {
        value: fields["extensions"].clone(),
    })?;
    let proposal = Proposal::new(scope, client, request, base, context, commands, extensions)?;
    if proposal
        .required_features()
        .into_iter()
        .map(crate::RecordFeature::wire_name)
        .collect::<Vec<_>>()
        != advertised
    {
        return Err(malformed());
    }
    Ok(proposal)
}
