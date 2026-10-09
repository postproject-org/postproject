use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::{
    CheckpointChunk, CheckpointSection, Digest, DigestDomain, Document, Extensions, FailureKind,
    MAX_RECORD_CHUNK_BYTES, MAX_RECORD_CHUNK_PAYLOAD, ProtocolError, Result, Scope,
    fields::{array, bounded_text, exact, limit, malformed, nullable, object, text, unsupported},
};

pub(super) fn decode_chunk(document: &Document) -> Result<CheckpointChunk> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "version",
            "required_features",
            "production",
            "history",
            "checkpoint",
            "section",
            "index",
            "previous",
            "payload",
            "extensions",
            "digest",
        ],
    )?;
    if text(&fields["kind"])? != "checkpoint.chunk" {
        return Err(malformed());
    }
    if text(&fields["version"])? != "1" {
        return Err(unsupported());
    }
    let features = array(&fields["required_features"], 64)?;
    if features.len() != 1 || text(&features[0])? != "checkpoints.v1" {
        return Err(unsupported());
    }
    let name = text(&fields["section"])?;
    let section = CheckpointSection::ALL
        .into_iter()
        .find(|section| section.as_str() == name)
        .ok_or_else(unsupported)?;
    let encoded = bounded_text(&fields["payload"], MAX_RECORD_CHUNK_PAYLOAD.div_ceil(3) * 4)?;
    let payload = STANDARD.decode(encoded).map_err(|_| malformed())?;
    if STANDARD.encode(&payload) != encoded {
        return Err(malformed());
    }
    let chunk = CheckpointChunk::new(
        Scope::new(exact(&fields["production"])?, exact(&fields["history"])?),
        exact(&fields["checkpoint"])?,
        section,
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
            "checkpoint chunk digest mismatch",
        ));
    }
    Ok(chunk)
}
