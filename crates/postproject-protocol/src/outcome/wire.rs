use serde_json::json;

use crate::{
    Document, Extensions, JobResult, MAX_PROPOSAL_COMMANDS, Outcome, OutcomeStatus, Rejection,
    Result, Scope, decode_receipt, encode_receipt,
    fields::{array, exact, malformed, object, text, unsupported},
};

pub(super) fn encode(outcome: &Outcome) -> Result<Document> {
    let (status, receipt, rejection) = match outcome.status() {
        OutcomeStatus::Accepted(receipt) => {
            ("accepted", Some(encode_receipt(receipt)?.value), None)
        }
        OutcomeStatus::Rejected(rejection) => ("rejected", None, Some(rejection.document()?.value)),
    };
    Ok(Document {
        value: json!({
            "kind":"outcome","version":"1","required_features":if outcome.jobs().is_empty() {vec!["outcomes.v1"]} else {vec!["jobs.v1", "outcomes.v1"]},
            "production":outcome.scope().production().to_string(),"history":outcome.scope().history().to_string(),
            "client":outcome.client().to_string(),"request":outcome.request().to_string(),
            "request_digest":outcome.request_digest().to_string(),"extensions":outcome.extensions().document().value,
            "status":status,"receipt":receipt,"rejection":rejection,
            "jobs":outcome.jobs().iter().map(|job|job.document().value).collect::<Vec<_>>()
        }),
    })
}

pub(super) fn decode(document: &Document) -> Result<Outcome> {
    let features = array(
        document
            .value
            .get("required_features")
            .ok_or_else(malformed)?,
        64,
    )?;
    let names = features.iter().map(text).collect::<Result<Vec<_>>>()?;
    // Read retained pre-publication outcomes without rewriting their rows.
    let legacy = names == ["metadata.v1"];
    if !legacy && names != ["outcomes.v1"] && names != ["jobs.v1", "outcomes.v1"] {
        return Err(unsupported());
    }
    let mut keys = vec![
        "kind",
        "version",
        "required_features",
        "production",
        "history",
        "client",
        "request",
        "request_digest",
        "extensions",
        "status",
        "receipt",
        "rejection",
    ];
    if !legacy {
        keys.push("jobs");
    }
    let fields = object(&document.value, &keys)?;
    if text(&fields["kind"])? != "outcome" {
        return Err(malformed());
    }
    if text(&fields["version"])? != "1" {
        return Err(unsupported());
    }
    let jobs = if legacy {
        vec![]
    } else {
        array(&fields["jobs"], MAX_PROPOSAL_COMMANDS)?
            .iter()
            .map(|value| {
                JobResult::from_document(&Document {
                    value: value.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?
    };
    if !legacy && names.contains(&"jobs.v1") == jobs.is_empty() {
        return Err(malformed());
    }
    let receipt = Document {
        value: fields["receipt"].clone(),
    };
    let rejection = Document {
        value: fields["rejection"].clone(),
    };
    let status = match text(&fields["status"])? {
        "accepted" if fields["rejection"].is_null() => {
            OutcomeStatus::Accepted(decode_receipt(&receipt)?)
        }
        "rejected" if fields["receipt"].is_null() => {
            OutcomeStatus::Rejected(Rejection::from_document(&rejection)?)
        }
        _ => return Err(malformed()),
    };
    Outcome::from_parts(
        Scope::new(exact(&fields["production"])?, exact(&fields["history"])?),
        exact(&fields["client"])?,
        exact(&fields["request"])?,
        exact(&fields["request_digest"])?,
        Extensions::new(Document {
            value: fields["extensions"].clone(),
        })?,
        status,
        jobs,
    )
}
