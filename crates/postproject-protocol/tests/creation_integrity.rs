//! Valid individual facts cannot make an incomplete or contradictory aggregate valid.

use postproject_core::{
    AssetId, ContentStructure, Locator, LocatorAvailability, LocatorId, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationImport, RepresentationKind,
    Resource, ResourceFingerprint, ResourceId, ResourceMember, ResourceRole,
};
use postproject_protocol::{
    CreationDecoder, Document, Limits, RepresentationCreationStart, encode_representation_creation,
};

fn documents() -> Vec<Document> {
    let first = ResourceId::new();
    let second = ResourceId::new();
    let member = |id| ResourceMember::new(id, ResourceRole::new("test:essence").unwrap(), true);
    let resource = |id| {
        Resource::new(
            id,
            vec![ResourceFingerprint::new("valid", 1, vec![1]).unwrap()],
            None,
        )
    };
    let locator = |id| {
        Locator::new(
            LocatorId::new(),
            id,
            format!("unknown:{id}"),
            None,
            LocatorAvailability::Unknown,
        )
        .unwrap()
    };
    let import = RepresentationImport::new(
        Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Original,
            ContentStructure::package(vec![member(first), member(second)]).unwrap(),
            vec![RepresentationFingerprint::new("valid", 1, vec![1]).unwrap()],
        ),
        vec![resource(first), resource(second)],
        vec![locator(first), locator(second)],
    )
    .unwrap();
    encode_representation_creation(&import, 7)
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn edit(document: &Document, path: &str, value: serde_json::Value) -> Document {
    let mut fields: serde_json::Value =
        serde_json::from_slice(&document.canonical_bytes().unwrap()).unwrap();
    *fields.pointer_mut(path).unwrap() = value;
    Document::parse(&serde_json::to_vec(&fields).unwrap(), Limits::default()).unwrap()
}

#[test]
fn every_truncated_prefix_rejects_and_complete_streams_reject_surplus_items() {
    let documents = documents();
    let header = RepresentationCreationStart::from_document(&documents[0]).unwrap();
    for end in 1..documents.len() {
        let mut decoder = CreationDecoder::new(header, 7).unwrap();
        for document in &documents[1..end] {
            decoder.push(document).unwrap();
        }
        assert!(decoder.finish().is_err());
    }
    let mut decoder = CreationDecoder::new(header, 7).unwrap();
    for document in &documents[1..] {
        decoder.push(document).unwrap();
    }
    assert!(decoder.push(documents.last().unwrap()).is_err());
    assert!(decoder.finish().is_err());
}

#[test]
fn foreign_evidence_wrong_boundaries_and_missing_locator_coverage_reject() {
    let documents = documents();
    let header = RepresentationCreationStart::from_document(&documents[0]).unwrap();
    let modifications = [
        (1, "/fingerprint/observed_revision_sequence", "8".into()),
        (
            1,
            "/fingerprint/observed_revision_sequence",
            serde_json::Value::Null,
        ),
        (
            2,
            "/representation_id",
            RepresentationId::new().to_string().into(),
        ),
        (5, "/resource/id", ResourceId::new().to_string().into()),
        (6, "/target/id", ResourceId::new().to_string().into()),
        (
            6,
            "/state",
            serde_json::json!({"kind":"superseded", "position":"0", "revision_sequence":"7"}),
        ),
        (9, "/resource_id", ResourceId::new().to_string().into()),
        (
            9,
            "/sequence_naming",
            serde_json::json!({"prefix":"shot.","suffix":".exr","padding":"4"}),
        ),
    ];
    for (index, path, value) in modifications {
        let mut decoder = CreationDecoder::new(header, 7).unwrap();
        for document in &documents[1..index] {
            decoder.push(document).unwrap();
        }
        assert!(decoder.push(&edit(&documents[index], path, value)).is_err());
        assert!(decoder.push(&documents[index]).is_err());
        assert!(decoder.finish().is_err());
    }
    // Exact frame count alone cannot establish coverage of every resource.
    let mut decoder = CreationDecoder::new(header, 7).unwrap();
    for document in &documents[1..10] {
        decoder.push(document).unwrap();
    }
    decoder.push(&documents[9]).unwrap();
    assert!(decoder.finish().is_err());
}
