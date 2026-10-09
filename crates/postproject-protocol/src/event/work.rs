//! Public worker and evidence notifications carry no credential material.

use postproject_core::{ResourceFingerprint, RevisionEventKind};
use serde_json::{Value, json};

use crate::{
    Result,
    fields::{checked, exact, malformed, object, text, unsupported},
};

pub(super) fn encode(event: &RevisionEventKind) -> Result<Value> {
    Ok(match event {
        RevisionEventKind::ResourceFingerprintObserved {
            resource_id,
            algorithm,
            version,
        } => {
            validate_algorithm(algorithm)?;
            json!({"kind":"resource_fingerprint_observed", "resource_id":resource_id.to_string(), "algorithm":algorithm, "version":version.to_string()})
        }
        RevisionEventKind::RepresentationFingerprintObserved {
            representation_id,
            algorithm,
            version,
        } => {
            validate_algorithm(algorithm)?;
            json!({"kind":"representation_fingerprint_observed", "representation_id":representation_id.to_string(), "algorithm":algorithm, "version":version.to_string()})
        }
        RevisionEventKind::JobRequested { job_id }
        | RevisionEventKind::JobClaimed { job_id }
        | RevisionEventKind::JobClaimRenewed { job_id }
        | RevisionEventKind::JobClaimReleased { job_id }
        | RevisionEventKind::JobSucceeded { job_id }
        | RevisionEventKind::JobFailed { job_id }
        | RevisionEventKind::JobCancelled { job_id } => {
            json!({"kind":event.event_type().as_str(), "job_id":job_id.to_string()})
        }
        _ => return Err(unsupported()),
    })
}

pub(super) fn decode(value: &Value) -> Result<RevisionEventKind> {
    let kind = text(value.get("kind").ok_or_else(malformed)?)?;
    Ok(match kind {
        "resource_fingerprint_observed" => {
            let f = object(value, &["kind", "resource_id", "algorithm", "version"])?;
            let algorithm = text(&f["algorithm"])?;
            validate_algorithm(algorithm)?;
            RevisionEventKind::ResourceFingerprintObserved {
                resource_id: exact(&f["resource_id"])?,
                algorithm: algorithm.to_owned(),
                version: exact(&f["version"])?,
            }
        }
        "representation_fingerprint_observed" => {
            let f = object(
                value,
                &["kind", "representation_id", "algorithm", "version"],
            )?;
            let algorithm = text(&f["algorithm"])?;
            validate_algorithm(algorithm)?;
            RevisionEventKind::RepresentationFingerprintObserved {
                representation_id: exact(&f["representation_id"])?,
                algorithm: algorithm.to_owned(),
                version: exact(&f["version"])?,
            }
        }
        "job_requested" | "job_claimed" | "job_claim_renewed" | "job_claim_released"
        | "job_succeeded" | "job_failed" | "job_cancelled" => {
            let f = object(value, &["kind", "job_id"])?;
            let job_id = exact(&f["job_id"])?;
            match kind {
                "job_requested" => RevisionEventKind::JobRequested { job_id },
                "job_claimed" => RevisionEventKind::JobClaimed { job_id },
                "job_claim_renewed" => RevisionEventKind::JobClaimRenewed { job_id },
                "job_claim_released" => RevisionEventKind::JobClaimReleased { job_id },
                "job_succeeded" => RevisionEventKind::JobSucceeded { job_id },
                "job_failed" => RevisionEventKind::JobFailed { job_id },
                _ => RevisionEventKind::JobCancelled { job_id },
            }
        }
        _ => return Err(unsupported()),
    })
}

fn validate_algorithm(algorithm: &str) -> Result<()> {
    // The event has no fingerprint bytes. Use the existing constructor solely
    // for its algorithm-domain validation; this temporary value is never stored.
    checked(ResourceFingerprint::new(algorithm, 0, vec![0])).map(|_| ())
}
