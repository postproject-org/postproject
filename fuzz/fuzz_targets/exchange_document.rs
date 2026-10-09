#![no_main]

use libfuzzer_sys::fuzz_target;
use postproject_protocol::{
    Command, ConflictVersion, Document, FrameDecoder, Limits, MetadataEffect, MetadataEffectStart,
    Outcome, Position, ProductionHeader, Proposal, RecordChunk, RecordManifest, Rejection,
    decode_event, decode_metadata, decode_receipt, encode_event, encode_metadata, encode_receipt,
};

fuzz_target!(|bytes: &[u8]| {
    let limits = Limits::new(64 * 1024, 192, 8192).unwrap();
    frames(bytes, limits);
    let Ok(document) = Document::parse(bytes, limits) else {
        return;
    };
    let canonical = document.canonical_bytes().unwrap();
    assert_eq!(Document::parse(&canonical, limits).unwrap(), document);
    if let Ok(value) = decode_metadata(&document) {
        let normalized = encode_metadata(&value).unwrap();
        assert_eq!(decode_metadata(&normalized).unwrap(), value);
    }
    if let Ok(command) = Command::from_document(&document) {
        assert_eq!(
            Command::from_document(&command.document().unwrap()).unwrap(),
            command
        );
    }
    if let Ok(effect) = MetadataEffect::from_document(&document) {
        assert_eq!(
            MetadataEffect::from_document(&effect.document().unwrap()).unwrap(),
            effect
        );
    }
    if let Ok(proposal) = Proposal::from_document(&document) {
        assert_eq!(
            Proposal::from_document(&proposal.document().unwrap()).unwrap(),
            proposal
        );
    }
    if let Ok(position) = Position::from_document(&document) {
        assert_eq!(
            Position::from_document(&position.document()).unwrap(),
            position
        );
    }
    if let Ok(chunk) = RecordChunk::from_document(&document) {
        assert_eq!(
            RecordChunk::from_document(&chunk.document().unwrap()).unwrap(),
            chunk
        );
    }
    if let Ok(manifest) = RecordManifest::from_document(&document) {
        assert_eq!(
            RecordManifest::from_document(&manifest.document().unwrap()).unwrap(),
            manifest
        );
    }
    if let Ok(outcome) = Outcome::from_document(&document) {
        assert_eq!(
            Outcome::from_document(&outcome.document().unwrap()).unwrap(),
            outcome
        );
    }
    if let Ok(rejection) = Rejection::from_document(&document) {
        assert_eq!(
            Rejection::from_document(&rejection.document().unwrap()).unwrap(),
            rejection
        );
    }
    if let Ok(receipt) = decode_receipt(&document) {
        assert_eq!(
            decode_receipt(&encode_receipt(&receipt).unwrap()).unwrap(),
            receipt
        );
    }
    if let Ok(event) = decode_event(&document) {
        assert_eq!(decode_event(&encode_event(&event).unwrap()).unwrap(), event);
    }
    if let Ok(header) = ProductionHeader::from_document(&document) {
        assert_eq!(
            ProductionHeader::from_document(&header.document()).unwrap(),
            header
        );
    }
    if let Ok(version) = ConflictVersion::from_document(&document) {
        assert_eq!(
            ConflictVersion::from_document(&version.document().unwrap()).unwrap(),
            version
        );
    }
    // Headers and individual values also undergo the same checked constructors.
    let _ = MetadataEffectStart::from_document(&document);
    let _ = MetadataEffectStart::decode_value(&document);
});

fn frames(bytes: &[u8], limits: Limits) {
    let mut decoder = FrameDecoder::new(limits);
    let size = usize::from(bytes.first().copied().unwrap_or(0) % 31) + 1;
    for input in bytes.chunks(size) {
        let mut position = 0;
        while position < input.len() {
            match decoder.consume(&input[position..]) {
                Ok((count, document)) => {
                    assert!(count > 0 && count <= input.len() - position);
                    position += count;
                    if let Some(document) = document {
                        let canonical = document.canonical_bytes().unwrap();
                        assert_eq!(Document::parse(&canonical, limits).unwrap(), document);
                    }
                }
                Err(_) => {
                    assert!(decoder.consume(&[]).is_err());
                    return;
                }
            }
        }
    }
    let _ = decoder.finish();
    assert!(decoder.consume(&[]).is_err());
}
