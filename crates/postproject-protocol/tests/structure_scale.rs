//! A native aggregate larger than one chunk remains a bounded stream of facts.

use postproject_core::{
    AssetId, ContentStructure, FrameRange, ImageSequenceDescriptor, MAX_CONTENT_MEMBERS,
    MAX_SEQUENCE_EXCEPTIONS, RationalRate, Representation, RepresentationId, RepresentationKind,
    ResourceId, ResourceMember, ResourceRole,
};
use postproject_protocol::{
    FrameDecoder, Limits, MAX_RECORD_CHUNK_PAYLOAD, StructureAssembler, StructureHeader,
    encode_structure,
};

fn representation(content: ContentStructure) -> Representation {
    Representation::new(
        RepresentationId::new(),
        AssetId::new(),
        RepresentationKind::Original,
        content,
        Vec::new(),
    )
}

fn restore(source: &Representation) -> (ContentStructure, usize, usize) {
    let mut documents = encode_structure(source).unwrap();
    let header = documents.next().unwrap().unwrap();
    let mut assembler = StructureAssembler::new(StructureHeader::from_document(&header).unwrap());
    let mut decoder = FrameDecoder::new(Limits::new(1024, 16, 100).unwrap());
    let mut total = 0;
    let mut count = 1;
    for document in documents {
        let bytes = document.unwrap().canonical_bytes().unwrap();
        assert!(bytes.len() < 512);
        let length = u64::try_from(bytes.len()).unwrap().to_be_bytes();
        // Separate pieces split both the fixed length and the UTF-8 body.
        for piece in [
            length[..3].as_ref(),
            length[3..].as_ref(),
            bytes[..17].as_ref(),
            bytes[17..].as_ref(),
        ] {
            total += piece.len();
            let mut remaining = piece;
            while !remaining.is_empty() {
                let (consumed, document) = decoder.consume(remaining).unwrap();
                assert!(consumed > 0);
                remaining = &remaining[consumed..];
                if let Some(document) = document {
                    assembler.push(&document).unwrap();
                    count += 1;
                }
            }
        }
    }
    decoder.finish().unwrap();
    (assembler.finish().unwrap(), total, count)
}

#[test]
fn maximum_package_exceeds_one_chunk_without_aggregate_documents_or_proposal_caps() {
    let role = ResourceRole::new(format!("vendor:{}", "r".repeat(121))).unwrap();
    let members = (0..MAX_CONTENT_MEMBERS)
        .map(|position| {
            ResourceMember::new(
                ResourceId::from_bytes(u128::try_from(position + 1).unwrap().to_be_bytes()),
                role.clone(),
                position % 2 == 0,
            )
        })
        .collect();
    let source = representation(ContentStructure::package(members).unwrap());
    let (restored, bytes, count) = restore(&source);
    assert!(bytes > MAX_RECORD_CHUNK_PAYLOAD);
    assert_eq!(count, MAX_CONTENT_MEMBERS + 1);
    assert_eq!(restored, *source.content_structure());
}

#[test]
fn maximum_sparse_sequence_retains_exact_exceptions_and_compact_frame_domain() {
    let missing = (0..MAX_SEQUENCE_EXCEPTIONS)
        .map(|position| i64::try_from(position).unwrap() * 2 - 100_000)
        .collect();
    let source = representation(ContentStructure::image_sequence(
        ImageSequenceDescriptor::new(
            ResourceId::new(),
            FrameRange::new(-100_000, 100_000, 2).unwrap(),
            RationalRate::new(24, 1).unwrap(),
            missing,
        )
        .unwrap(),
    ));
    let (restored, _, count) = restore(&source);
    assert_eq!(count, MAX_SEQUENCE_EXCEPTIONS + 1);
    assert_eq!(restored, *source.content_structure());
}
