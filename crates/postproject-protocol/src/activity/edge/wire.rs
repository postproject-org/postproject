use super::{ActivityEdgeHeader, ActivityEdgeSide};
use crate::{
    Document, Result,
    fields::{checked, exact, malformed, nullable, object, text, unsupported},
};
use postproject_core::ActivityRole;
use serde_json::json;

pub(super) fn encode(header: &ActivityEdgeHeader) -> Document {
    Document {
        value: json!({"kind":"activity.edge", "activity_id":header.activity_id().to_string(), "side":match header.side() { ActivityEdgeSide::Input => "input", ActivityEdgeSide::Output => "output" }, "position":header.position().to_string(), "representation_id":header.representation_id().to_string(), "role":header.role().map(ActivityRole::as_str), "snapshot_revision_sequence":header.snapshot_revision_sequence().map(|sequence|sequence.to_string()), "fingerprint_count":header.fingerprint_count().to_string(), "dependency_snapshot":header.has_dependency_snapshot(), "dependency_path_count":header.dependency_path_count().to_string()}),
    }
}

pub(super) fn decode(document: &Document) -> Result<ActivityEdgeHeader> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "activity_id",
            "side",
            "position",
            "representation_id",
            "role",
            "snapshot_revision_sequence",
            "fingerprint_count",
            "dependency_snapshot",
            "dependency_path_count",
        ],
    )?;
    if document.kind()? != "activity.edge" {
        return Err(unsupported());
    }
    let side = match text(&fields["side"])? {
        "input" => ActivityEdgeSide::Input,
        "output" => ActivityEdgeSide::Output,
        _ => return Err(unsupported()),
    };
    let mut header = ActivityEdgeHeader::new(
        exact(&fields["activity_id"])?,
        side,
        exact(&fields["position"])?,
        exact(&fields["representation_id"])?,
        nullable(&fields["role"], |value| {
            checked(ActivityRole::new(text(value)?))
        })?,
    )?;
    let fingerprints = exact(&fields["fingerprint_count"])?;
    if let Some(sequence) = nullable(&fields["snapshot_revision_sequence"], exact)? {
        header = header.with_snapshot(sequence, fingerprints)?;
    } else if fingerprints != 0 {
        return Err(malformed());
    }
    let paths = exact(&fields["dependency_path_count"])?;
    if fields["dependency_snapshot"]
        .as_bool()
        .ok_or_else(malformed)?
    {
        header = header.with_dependency_snapshot(paths)?;
    } else if paths != 0 {
        return Err(malformed());
    }
    Ok(header)
}
