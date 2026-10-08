use crate::{
    ChunkSummary, Digest, DigestDomain, Document, Extensions, FailureKind, Position, ProtocolError,
    RecordManifest, Result,
    fields::{array, exact, malformed, object, text, unsupported},
    receipt::decode_revision,
};

pub(super) fn decode(document: &Document) -> Result<RecordManifest> {
    let fields = object(
        &document.value,
        &[
            "kind",
            "version",
            "required_features",
            "predecessor",
            "revision",
            "chunks",
            "effects",
            "events",
            "extensions",
            "record_digest",
            "digest",
        ],
    )?;
    if text(&fields["kind"])? != "record.manifest" {
        return Err(malformed());
    }
    if text(&fields["version"])? != "1" {
        return Err(unsupported());
    }
    let features = array(&fields["required_features"], 64)?;
    if features.len() != 2
        || text(&features[0])? != "metadata.v1"
        || text(&features[1])? != "record-chunks.v1"
    {
        return Err(unsupported());
    }
    let chunks = object(
        &fields["chunks"],
        &["count", "payload_bytes", "last_digest"],
    )?;
    let manifest = RecordManifest::new(
        Position::from_document(&Document {
            value: fields["predecessor"].clone(),
        })?,
        decode_revision(&fields["revision"])?,
        ChunkSummary::new(
            exact(&chunks["count"])?,
            exact(&chunks["payload_bytes"])?,
            exact(&chunks["last_digest"])?,
        )?,
        exact(&fields["effects"])?,
        exact(&fields["events"])?,
        Extensions::new(Document {
            value: fields["extensions"].clone(),
        })?,
    )?;
    let declared_record: Digest = exact(&fields["record_digest"])?;
    let declared_manifest: Digest = exact(&fields["digest"])?;
    if manifest.record_digest()? != declared_record
        || document.digest(DigestDomain::Manifest)? != declared_manifest
    {
        return Err(ProtocolError::new(
            FailureKind::Integrity,
            "record manifest digest mismatch",
        ));
    }
    Ok(manifest)
}
