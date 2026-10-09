//! Fixed metadata stream bytes/digests authored without the `PostProject` codec.

use std::path::Path;

use postproject_core::DecisionBase;
use postproject_protocol::{
    Document, FrameDecoder, Limits, MetadataEffect, MetadataEffectStart, Position, ProtocolBase,
    RecordChunk, RecordManifest, decode_event, encode_event,
};

fn document(value: &serde_json::Value) -> Document {
    Document::parse(value.to_string().as_bytes(), Limits::default()).unwrap()
}

#[test]
fn independent_split_stream_preserves_exact_authored_frames_and_both_digests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/exchange");
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/exchange/stream-vectors.json"
    ))
    .unwrap();
    for vector in vectors.as_array().unwrap() {
        let input: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(vector["file"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let manifest = RecordManifest::from_document(&document(&input["manifest"])).unwrap();
        assert_eq!(
            manifest.document().unwrap().canonical_bytes().unwrap(),
            vector["canonical_manifest"].as_str().unwrap().as_bytes()
        );
        assert_eq!(
            manifest.record_digest().unwrap().to_string(),
            vector["record_digest"].as_str().unwrap()
        );
        assert_eq!(
            manifest.head().unwrap().document(),
            document(&input["head"])
        );
        let scope = manifest.predecessor().scope();
        let anchor = Position::anchor(
            ProtocolBase::new(
                scope,
                DecisionBase::new(scope.production(), None, 0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(anchor.document(), document(&input["floor"]));
        let frames = decode_chunks(&input, vector, &manifest);
        let expected = input["body"].as_array().unwrap();
        assert_eq!(frames.len(), expected.len());
        for (actual, expected) in frames.iter().zip(expected) {
            assert_eq!(actual, &document(expected));
        }
        let start = MetadataEffectStart::from_document(&frames[0]).unwrap();
        let value = MetadataEffectStart::decode_value(&frames[1]).unwrap();
        let effect =
            MetadataEffect::appended(start.target(), start.property().clone(), 0, value).unwrap();
        for (actual, expected) in effect.frames().zip(&frames) {
            assert_eq!(actual.unwrap(), *expected);
        }
        assert_eq!(
            encode_event(&decode_event(&frames[2]).unwrap()).unwrap(),
            frames[2]
        );
    }
}

fn decode_chunks(
    input: &serde_json::Value,
    vector: &serde_json::Value,
    manifest: &RecordManifest,
) -> Vec<Document> {
    let mut chain = manifest.chunk_chain();
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut frames = Vec::new();
    for (index, encoded) in input["chunks"].as_array().unwrap().iter().enumerate() {
        let chunk = RecordChunk::from_document(&document(encoded)).unwrap();
        assert_eq!(
            chunk.digest().unwrap().to_string(),
            vector["chunk_digests"][index].as_str().unwrap()
        );
        assert_eq!(
            chunk.document().unwrap().canonical_bytes().unwrap(),
            vector["canonical_chunks"][index]
                .as_str()
                .unwrap()
                .as_bytes()
        );
        chain.push(&chunk).unwrap();
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, item) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            frames.extend(item);
        }
    }
    decoder.finish().unwrap();
    manifest.verify_chain(chain).unwrap();
    frames
}
