//! Complete observations reuse checked domain evidence and native decision guards.

use postproject_core::{MAX_DEPENDENCIES_PER_SET, RepresentationFingerprint, ResourceFingerprint};
use serde_json::{Value, json};

use super::{Command, evidence};
use crate::{
    DependencyOccurrence, Document, Result,
    fields::{array, checked, exact, malformed, object, unsupported},
};

pub(super) fn encode(command: &Command) -> Result<Value> {
    Ok(match command {
        Command::RecordResourceFingerprint {
            resource_id,
            fingerprint,
        } => json!({
            "kind":"resource.observe-fingerprint", "id":resource_id.to_string(),
            "fingerprint":evidence::encode(fingerprint.algorithm(), fingerprint.version(), fingerprint.value())}),
        Command::RecordRepresentationFingerprint {
            representation_id,
            fingerprint,
        } => json!({
            "kind":"representation.observe-fingerprint", "id":representation_id.to_string(),
            "fingerprint":evidence::encode(fingerprint.algorithm(), fingerprint.version(), fingerprint.value())}),
        Command::RecordDependencySet {
            representation_id,
            dependencies,
        } => {
            let occurrences = dependencies
                .iter()
                .enumerate()
                .map(|(position, dependency)| {
                    DependencyOccurrence::new(
                        *representation_id,
                        u64::try_from(position).map_err(|_| malformed())?,
                        dependency.clone(),
                    )?
                    .document()
                    .map(|frame| frame.value)
                })
                .collect::<Result<Vec<_>>>()?;
            json!({"kind":"dependency.observe-set", "id":representation_id.to_string(),
                "occurrences":occurrences})
        }
        _ => return Err(unsupported()),
    })
}

pub(super) fn decode(kind: &str, value: &Value) -> Result<Command> {
    Ok(match kind {
        "resource.observe-fingerprint" | "representation.observe-fingerprint" => {
            let fields = object(value, &["kind", "id", "fingerprint"])?;
            let (algorithm, version, bytes) = evidence::decode(&fields["fingerprint"])?;
            if kind == "resource.observe-fingerprint" {
                Command::RecordResourceFingerprint {
                    resource_id: exact(&fields["id"])?,
                    fingerprint: checked(ResourceFingerprint::new(algorithm, version, bytes))?,
                }
            } else {
                Command::RecordRepresentationFingerprint {
                    representation_id: exact(&fields["id"])?,
                    fingerprint: checked(RepresentationFingerprint::new(
                        algorithm, version, bytes,
                    ))?,
                }
            }
        }
        "dependency.observe-set" => {
            let fields = object(value, &["kind", "id", "occurrences"])?;
            let id = exact(&fields["id"])?;
            let dependencies = array(&fields["occurrences"], MAX_DEPENDENCIES_PER_SET)?
                .iter()
                .enumerate()
                .map(|(position, value)| {
                    let occurrence = DependencyOccurrence::from_document(&Document {
                        value: value.clone(),
                    })?;
                    if occurrence.source_representation_id() != id
                        || occurrence.position()
                            != u64::try_from(position).map_err(|_| malformed())?
                    {
                        return Err(malformed());
                    }
                    Ok(occurrence.dependency().clone())
                })
                .collect::<Result<Vec<_>>>()?;
            Command::RecordDependencySet {
                representation_id: id,
                dependencies,
            }
        }
        _ => return Err(unsupported()),
    })
}
