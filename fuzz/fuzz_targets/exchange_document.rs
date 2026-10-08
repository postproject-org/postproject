#![no_main]

use libfuzzer_sys::fuzz_target;
use postproject_protocol::{
    Command, Document, Limits, MetadataEffect, Proposal, decode_metadata, encode_metadata,
};

fuzz_target!(|bytes: &[u8]| {
    let limits = Limits::new(64 * 1024, 192, 8192).unwrap();
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
});
