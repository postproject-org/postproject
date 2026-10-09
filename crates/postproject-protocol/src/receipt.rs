//! Exact committed receipt encoding shared by outcomes and history records.

use postproject_core::{CommitReceipt, OriginIdentity, Revision, RevisionContext, Timestamp};
use serde_json::{Value, json};

use crate::{
    Document, Result,
    fields::{checked, exact, malformed, nullable, object, text},
};

/// Encodes an original retained revision as a checkpoint body item.
///
/// # Errors
/// Rejects sequences or context outside the supported domain.
pub fn encode_revision_observation(revision: &Revision) -> Result<Document> {
    Ok(Document {
        value: json!({"kind":"revision.observation", "revision":encode_revision(revision)?}),
    })
}

/// Decodes original revision facts without allocating a local revision.
///
/// # Errors
/// Rejects unknown fields, kinds and invalid domain values.
pub fn decode_revision_observation(document: &Document) -> Result<Revision> {
    let fields = object(&document.value, &["kind", "revision"])?;
    if text(&fields["kind"])? != "revision.observation" {
        return Err(crate::fields::unsupported());
    }
    decode_revision(&fields["revision"])
}

/// Encodes this transaction's receipt, including an explicit no-revision result.
///
/// # Errors
/// Rejects future unsupported receipt values.
pub fn encode_receipt(receipt: &CommitReceipt) -> Result<Document> {
    Ok(Document {
        value: json!({
            "production":receipt.production_id().to_string(),
            "revision":receipt.revision().map(encode_revision).transpose()?
        }),
    })
}

/// Decodes a complete receipt without querying the production's latest head.
///
/// # Errors
/// Rejects unknown fields, invalid IDs, out-of-range sequences or context.
pub fn decode_receipt(document: &Document) -> Result<CommitReceipt> {
    let fields = object(&document.value, &["production", "revision"])?;
    Ok(CommitReceipt::new(
        exact(&fields["production"])?,
        nullable(&fields["revision"], decode_revision)?,
    ))
}

pub(crate) fn encode_context(context: &RevisionContext) -> Value {
    json!({
        "origin":context.origin().map(|origin| json!({"name":origin.name(),"version":origin.version(),"uri":origin.uri()})),
        "message":context.message()
    })
}

pub(crate) fn decode_context(value: &Value) -> Result<RevisionContext> {
    let fields = object(value, &["origin", "message"])?;
    let origin = nullable(&fields["origin"], |value| {
        let fields = object(value, &["name", "version", "uri"])?;
        checked(OriginIdentity::new(
            text(&fields["name"])?,
            nullable(&fields["version"], |v| Ok(text(v)?.to_owned()))?,
            nullable(&fields["uri"], |v| Ok(text(v)?.to_owned()))?,
        ))
    })?;
    checked(RevisionContext::new(
        origin,
        nullable(&fields["message"], |v| Ok(text(v)?.to_owned()))?,
    ))
}

pub(crate) fn encode_revision(revision: &Revision) -> Result<Value> {
    if revision.sequence() > i64::MAX as u64 {
        return Err(malformed());
    }
    let context = checked(RevisionContext::new(
        revision.origin().cloned(),
        revision.message().map(str::to_owned),
    ))?;
    Ok(json!({
        "id":revision.id().to_string(),"sequence":revision.sequence().to_string(),
        "transaction":revision.transaction_id().to_string(),
        "committed_at":revision.committed_at().as_unix_micros().to_string(),
        "context":encode_context(&context)
    }))
}

pub(crate) fn decode_revision(value: &Value) -> Result<Revision> {
    let fields = object(
        value,
        &["id", "sequence", "transaction", "committed_at", "context"],
    )?;
    let sequence: u64 = exact(&fields["sequence"])?;
    if sequence > i64::MAX as u64 {
        return Err(malformed());
    }
    let context = decode_context(&fields["context"])?;
    checked(Revision::new(
        exact(&fields["id"])?,
        sequence,
        exact(&fields["transaction"])?,
        Timestamp::from_unix_micros(exact(&fields["committed_at"])?),
        context.origin().cloned(),
        context.message().map(str::to_owned),
    ))
}
