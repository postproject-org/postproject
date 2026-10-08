use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::{
    Digest, DigestDomain, Document, Extensions, FailureKind, MAX_RECORD_CHUNK_BYTES,
    MAX_RECORD_CHUNK_PAYLOAD, ProtocolError, RecordChunk, Result, Scope,
    fields::{array, bounded_text, exact, limit, malformed, nullable, object, text, unsupported},
};

pub(super) fn decode(document: &Document) -> Result<RecordChunk> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "version",
            "required_features",
            "production",
            "history",
            "revision",
            "index",
            "previous",
            "payload",
            "extensions",
            "digest",
        ],
    )?;
    if text(&fields["kind"])? != "record.chunk" {
        return Err(malformed());
    }
    if text(&fields["version"])? != "1" {
        return Err(unsupported());
    }
    let features = array(&fields["required_features"], 64)?;
    if features.len() != 1 || text(&features[0])? != "record-chunks.v1" {
        return Err(unsupported());
    }
    // Bound the decoded allocation before invoking the base64 engine.
    let encoded = bounded_text(&fields["payload"], MAX_RECORD_CHUNK_PAYLOAD.div_ceil(3) * 4)?;
    let payload = STANDARD.decode(encoded).map_err(|_| malformed())?;
    if STANDARD.encode(&payload) != encoded {
        return Err(malformed());
    }
    let chunk = RecordChunk::new(
        Scope::new(exact(&fields["production"])?, exact(&fields["history"])?),
        exact(&fields["revision"])?,
        exact(&fields["index"])?,
        nullable(&fields["previous"], exact)?,
        payload,
        Extensions::new(Document {
            value: fields["extensions"].clone(),
        })?,
    )?;
    if document.canonical_bytes()?.len() > MAX_RECORD_CHUNK_BYTES {
        return Err(limit());
    }
    let declared: Digest = exact(&fields["digest"])?;
    if document.digest(DigestDomain::Chunk)? != declared {
        return Err(ProtocolError::new(
            FailureKind::Integrity,
            "record chunk digest mismatch",
        ));
    }
    Ok(chunk)
}
