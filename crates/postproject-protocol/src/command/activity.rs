//! Activity intent contains attribution and edges; storage captures evidence.

use postproject_core::{Activity, ActivityInput, ActivityOutput, ActivityRole, MAX_ACTIVITY_EDGES};
use serde_json::{Value, json};

use crate::{
    ActivityHeader, Document, Result,
    fields::{array, checked, exact, malformed, nullable, object, text},
};

pub(super) fn encode(activity: &Activity) -> Result<Value> {
    if activity
        .inputs()
        .iter()
        .any(|edge| edge.snapshot().is_some())
        || activity
            .outputs()
            .iter()
            .any(|edge| edge.snapshot().is_some())
    {
        return Err(malformed());
    }
    let inputs: Vec<_> = activity
        .inputs()
        .iter()
        .map(|edge| {
            json!({
        "representation_id":edge.representation_id().to_string(),
        "role":edge.role().map(ActivityRole::as_str)})
        })
        .collect();
    let outputs: Vec<_> = activity
        .outputs()
        .iter()
        .map(|edge| {
            json!({
        "representation_id":edge.representation_id().to_string(),
        "role":edge.role().map(ActivityRole::as_str)})
        })
        .collect();
    Ok(
        json!({"kind":"activity.create", "header":ActivityHeader::from_activity(activity)?.document().value,
        "inputs":inputs, "outputs":outputs}),
    )
}

pub(super) fn decode(value: &Value) -> Result<Activity> {
    let fields = object(value, &["kind", "header", "inputs", "outputs"])?;
    let header = ActivityHeader::from_document(&Document {
        value: fields["header"].clone(),
    })?;
    let inputs = array(&fields["inputs"], MAX_ACTIVITY_EDGES)?
        .iter()
        .map(|value| {
            let (id, role) = edge(value)?;
            Ok(ActivityInput::new(id, role))
        })
        .collect::<Result<Vec<_>>>()?;
    let outputs = array(&fields["outputs"], MAX_ACTIVITY_EDGES)?
        .iter()
        .map(|value| {
            let (id, role) = edge(value)?;
            Ok(ActivityOutput::new(id, role))
        })
        .collect::<Result<Vec<_>>>()?;
    if u64::try_from(inputs.len()).map_err(|_| malformed())? != header.input_count()
        || u64::try_from(outputs.len()).map_err(|_| malformed())? != header.output_count()
    {
        return Err(malformed());
    }
    let mut activity = checked(Activity::new(
        header.id(),
        header.kind().clone(),
        inputs,
        outputs,
    ))?;
    activity = checked(activity.with_timing(header.started_at(), header.finished_at()))?;
    if let Some(tool) = header.tool() {
        activity = activity.with_tool(tool.clone());
    }
    if let Some(agent) = header.agent() {
        activity = activity.with_agent(agent.clone());
    }
    Ok(activity)
}

fn edge(value: &Value) -> Result<(postproject_core::RepresentationId, Option<ActivityRole>)> {
    let fields = object(value, &["representation_id", "role"])?;
    Ok((
        exact(&fields["representation_id"])?,
        nullable(&fields["role"], |value| {
            checked(ActivityRole::new(text(value)?))
        })?,
    ))
}
