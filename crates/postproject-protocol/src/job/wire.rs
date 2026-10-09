use postproject_core::{JobKind, RepresentationKind, RequestedJobOutput};
use serde_json::json;

use super::{JobHeader, state};
use crate::{
    Document, Result,
    fields::{checked, exact, nullable, object, text, unsupported},
};

pub(super) fn encode(header: &JobHeader) -> Result<Document> {
    let output = header.requested_output();
    let role = match output.representation_kind() {
        RepresentationKind::Original => "original",
        RepresentationKind::Proxy => "proxy",
        RepresentationKind::Optimized => "optimized",
        RepresentationKind::Derived => "derived",
        _ => return Err(unsupported()),
    };
    Ok(Document {
        value: json!({"kind":"job.header", "id":header.id().to_string(), "job_kind":header.kind().as_str(), "input_count":header.input_count().to_string(), "requested_output":{"asset_id":output.asset_id().to_string(), "role":role, "target_root":output.target_root()}, "state":state::encode(header.state())?}),
    })
}

pub(super) fn decode(document: &Document) -> Result<JobHeader> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "id",
            "job_kind",
            "input_count",
            "requested_output",
            "state",
        ],
    )?;
    if document.kind()? != "job.header" {
        return Err(unsupported());
    }
    let output = object(
        &fields["requested_output"],
        &["asset_id", "role", "target_root"],
    )?;
    let role = match text(&output["role"])? {
        "original" => RepresentationKind::Original,
        "proxy" => RepresentationKind::Proxy,
        "optimized" => RepresentationKind::Optimized,
        "derived" => RepresentationKind::Derived,
        _ => return Err(unsupported()),
    };
    JobHeader::new(
        exact(&fields["id"])?,
        checked(JobKind::new(text(&fields["job_kind"])?))?,
        exact(&fields["input_count"])?,
        checked(RequestedJobOutput::new(
            exact(&output["asset_id"])?,
            role,
            nullable(&output["target_root"], |value| Ok(text(value)?.to_owned()))?,
        ))?,
        state::decode(&fields["state"])?,
    )
}
