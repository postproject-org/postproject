//! Fixed record streams authored without the `PostProject` codec.

use std::path::Path;

use postproject_core::DecisionBase;
use postproject_protocol::{
    ActivityEdgeHeader, ActivityHeader, ActivityPathHeader, ActivityPathSegment, CreationDecoder,
    CreationFact, DependencyOccurrence, DependencySetHeader, Document, FingerprintChangeStart,
    FingerprintRecomputation, FrameDecoder, IdentifierChange, JobHeader, JobInput, JobTransition,
    Limits, MediaChange, MetadataEffect, MetadataEffectStart, MetadataOperation, Position,
    ProtocolBase, RecordChunk, RecordManifest, RepresentationCreationStart, ResourceCreationStart,
    SequenceException, StructureHeader, StructureMember, decode_asset, decode_event,
    decode_fingerprint_snapshot, decode_locator, encode_asset, encode_event,
    encode_fingerprint_snapshot, encode_locator,
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
        if frames[0].kind().unwrap() == "metadata.effect" {
            check_metadata_frames(&frames);
        } else if frames[0].kind().unwrap() == "original.creation" {
            check_media_frames(&frames, expected, manifest.revision().sequence());
        } else {
            for frame in frames
                .iter()
                .filter(|frame| frame.kind().unwrap() != "observation")
            {
                assert_eq!(round_trip_fact(frame), *frame);
            }
        }
        for frame in frames
            .iter()
            .filter(|frame| frame.kind().unwrap() == "observation")
        {
            assert_eq!(encode_event(&decode_event(frame).unwrap()).unwrap(), *frame);
        }
    }
}

fn check_metadata_frames(frames: &[Document]) {
    let mut offset = 0;
    while frames[offset].kind().unwrap() != "observation" {
        let start = MetadataEffectStart::from_document(&frames[offset]).unwrap();
        let count = usize::try_from(start.value_count()).unwrap();
        let values = frames[offset + 1..offset + 1 + count]
            .iter()
            .map(|frame| MetadataEffectStart::decode_value(frame).unwrap())
            .collect::<Vec<_>>();
        let effect = match start.operation() {
            MetadataOperation::Appended(position) => MetadataEffect::appended(
                start.target(),
                start.property().clone(),
                position,
                values[0].clone(),
            )
            .unwrap(),
            MetadataOperation::Replaced => {
                MetadataEffect::replaced(start.target(), start.property().clone(), values)
            }
            MetadataOperation::Removed => {
                MetadataEffect::removed(start.target(), start.property().clone())
            }
        };
        let encoded = effect.frames().map(Result::unwrap).collect::<Vec<_>>();
        assert_eq!(encoded, frames[offset..offset + 1 + count]);
        offset += 1 + count;
    }
    assert!(
        frames[offset..]
            .iter()
            .all(|frame| frame.kind().unwrap() == "observation")
    );
}

fn check_media_frames(frames: &[Document], expected: &[serde_json::Value], sequence: u64) {
    let mut offset = 0;
    while frames[offset].kind().unwrap() != "observation" {
        if frames[offset].kind().unwrap() == "original.creation" {
            let asset = document(&expected[offset]["asset"]);
            assert_eq!(encode_asset(&decode_asset(&asset).unwrap()), asset);
            offset += 1;
        }
        let start = RepresentationCreationStart::from_document(&frames[offset]).unwrap();
        assert_eq!(start.document().unwrap(), frames[offset]);
        let resources = start.resource_count();
        let mut decoder = CreationDecoder::new(start, sequence).unwrap();
        offset += 1;
        while !matches!(
            frames[offset].kind().unwrap(),
            "original.creation" | "representation.creation" | "observation"
        ) {
            let frame = &frames[offset];
            assert_eq!(round_trip_fact(frame), *frame);
            match decoder.push(frame).unwrap() {
                Some(CreationFact::Resource(header)) => {
                    let original = document(&expected[offset]["resource"]);
                    assert_eq!(header.document(), original);
                }
                Some(CreationFact::Locator(locator)) => {
                    assert_eq!(encode_locator(&locator).unwrap(), *frame);
                }
                Some(CreationFact::Structure(content)) => {
                    assert_eq!(content.resource_ids().len() as u64, resources)
                }
                None => {}
                other => panic!("unexpected creation fact {other:?}"),
            }
            offset += 1;
        }
        decoder.finish().unwrap();
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

fn round_trip_fact(frame: &Document) -> Document {
    match frame.kind().unwrap() {
        "representation.creation" => RepresentationCreationStart::from_document(frame)
            .unwrap()
            .document()
            .unwrap(),
        "resource.creation" => ResourceCreationStart::from_document(frame)
            .unwrap()
            .document(),
        "structure.header" => StructureHeader::from_document(frame).unwrap().document(),
        "structure.member" => StructureMember::from_document(frame).unwrap().document(),
        "structure.exception" => SequenceException::from_document(frame).unwrap().document(),
        "locator.fact" => encode_locator(&decode_locator(frame).unwrap()).unwrap(),
        "job.header" => JobHeader::from_document(frame).unwrap().document().unwrap(),
        "job.input" => JobInput::from_document(frame).unwrap().document(),
        "job.transition" => JobTransition::from_document(frame)
            .unwrap()
            .document()
            .unwrap(),
        "activity.header" => ActivityHeader::from_document(frame).unwrap().document(),
        "activity.edge" => ActivityEdgeHeader::from_document(frame).unwrap().document(),
        "activity.dependency-path" => ActivityPathHeader::from_document(frame).unwrap().document(),
        "activity.dependency-segment" => ActivityPathSegment::from_document(frame)
            .unwrap()
            .document()
            .unwrap(),
        "fingerprint.snapshot" => {
            encode_fingerprint_snapshot(&decode_fingerprint_snapshot(frame).unwrap()).unwrap()
        }
        "fingerprint.change" => FingerprintChangeStart::from_document(frame)
            .unwrap()
            .document()
            .unwrap(),
        "fingerprint.recomputation" => FingerprintRecomputation::from_document(frame)
            .unwrap()
            .document(),
        "dependency.set" => DependencySetHeader::from_document(frame)
            .unwrap()
            .document(),
        "dependency.occurrence" => DependencyOccurrence::from_document(frame)
            .unwrap()
            .document()
            .unwrap(),
        kind if kind.starts_with("identifier.") => IdentifierChange::from_document(frame)
            .unwrap()
            .document()
            .unwrap(),
        _ => MediaChange::from_document(frame)
            .unwrap()
            .document()
            .unwrap(),
    }
}
