use serde_json::json;

use crate::{
    Document, Extensions, Outcome, OutcomeStatus, Rejection, Result, Scope, decode_receipt,
    encode_receipt,
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
            "kind":"outcome","version":"1","required_features":["metadata.v1"],
            "production":outcome.scope().production().to_string(),"history":outcome.scope().history().to_string(),
            "client":outcome.client().to_string(),"request":outcome.request().to_string(),
            "request_digest":outcome.request_digest().to_string(),"extensions":outcome.extensions().document().value,
            "status":status,"receipt":receipt,"rejection":rejection
        }),
    })
}

pub(super) fn decode(document: &Document) -> Result<Outcome> {
    let fields = object(
        &document.value,
        &[
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
        ],
    )?;
    if text(&fields["kind"])? != "outcome" {
        return Err(malformed());
    }
    let features = array(&fields["required_features"], 64)?;
    if text(&fields["version"])? != "1"
        || features.len() != 1
        || text(&features[0])? != "metadata.v1"
    {
        return Err(unsupported());
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
    )
}
