use postproject_core::{
    Asset, AssetId, ContentStructure, Dependency, DependencyKind, DependencyTarget, Locator,
    LocatorAvailability, LocatorId, OriginalMediaImport, Representation, RepresentationFingerprint,
    RepresentationId, RepresentationKind, Resource, ResourceId, Timestamp,
};
use postproject_protocol::{
    Document, Extensions, FrameDecoder, Limits, RecordChunk, RecordManifest,
};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

pub(super) fn media(label: u32, fingerprinted: bool) -> OriginalMediaImport {
    let bytes = u128::from(label).to_be_bytes();
    let asset = AssetId::from_bytes(bytes);
    let resource = ResourceId::from_bytes(bytes);
    OriginalMediaImport::new(
        Asset::new(asset, Timestamp::from_unix_micros(-123), None, None),
        Representation::new(
            RepresentationId::from_bytes(bytes),
            asset,
            RepresentationKind::Original,
            ContentStructure::single_resource(resource),
            if fingerprinted {
                vec![
                    RepresentationFingerprint::new(
                        "unknown_Exact",
                        42,
                        label.to_be_bytes().to_vec(),
                    )
                    .unwrap(),
                ]
            } else {
                Vec::new()
            },
        ),
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::from_bytes(bytes),
                resource,
                format!("file:///unavailable/{label}.dat"),
                None,
                LocatorAvailability::Online,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

pub(super) fn dependency(target: DependencyTarget, required: bool) -> Dependency {
    Dependency::new(
        None,
        DependencyKind::new("unknown:Exact").unwrap(),
        target,
        None,
        required,
        "  /镜头/../A.%04d.exr  ",
    )
    .unwrap()
}

pub(super) fn apply(source: &SqliteProduction, mirror: &mut SqliteProduction, sequence: u64) {
    let mut reader = source.record_reader(sequence).unwrap();
    let manifest = reader.manifest().clone();
    assert!(
        mirror
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default()
            )
            .unwrap()
    );
}

pub(super) fn frames(source: &SqliteProduction, sequence: u64) -> Vec<Document> {
    let mut reader = source.record_reader(sequence).unwrap();
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut documents = Vec::new();
    while let Some(chunk) = reader.next_chunk().unwrap() {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (read, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += read;
            if let Some(document) = document {
                documents.push(document);
            }
        }
    }
    decoder.finish().unwrap();
    documents
}

pub(super) fn rehashed(
    original: &RecordManifest,
    documents: &[Document],
) -> (RecordManifest, RecordChunk) {
    let mut payload = Vec::new();
    for document in documents {
        let bytes = document.canonical_bytes().unwrap();
        payload.extend_from_slice(&u64::try_from(bytes.len()).unwrap().to_be_bytes());
        payload.extend(bytes);
    }
    let chunk = RecordChunk::new(
        original.predecessor().scope(),
        original.revision().id(),
        0,
        None,
        payload,
        Extensions::default(),
    )
    .unwrap();
    let mut chain = original.chunk_chain();
    chain.push(&chunk).unwrap();
    let manifest = RecordManifest::new(
        original.predecessor(),
        original.revision().clone(),
        chain.finish().unwrap(),
        original.effect_count(),
        original.event_count(),
        original.extensions().clone(),
    )
    .unwrap()
    .with_required_features(original.required_features())
    .unwrap();
    (manifest, chunk)
}
