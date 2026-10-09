use postproject_core::{
    Asset, AssetId, ContentStructure, ExternalIdentifier, FileFacts, FingerprintSnapshot,
    IdentifierScheme, Locator, LocatorAvailability, LocatorId, ObjectRef, OriginalMediaImport,
    Representation, RepresentationId, RepresentationKind, Resource, ResourceFingerprint,
    ResourceId, Timestamp,
};
use postproject_protocol::{
    CheckpointSection, Document, FingerprintObservation, FingerprintState, IdentifierAttachment,
    RepresentationHeader, ResourceHeader, encode_asset, encode_locator,
};
use postproject_storage_sqlite::{CheckpointLimits, SqliteProduction};

#[test]
fn rehashed_media_contradictions_never_publish_a_destination_or_change_the_source() {
    let directory = tempfile::tempdir().unwrap();
    let (source, import) = fixture(&directory.path().join("source.pproj"));
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let head = source.exchange_head().unwrap();
    for (section, body) in contradictions(&import) {
        let (rewritten, body) = super::replace_section(&manifest, &chunks, section, &body);
        let path = directory.path().join("rejected.pproj");
        assert!(
            SqliteProduction::import_checkpoint(
                &path,
                &rewritten,
                body.into_iter().map(Ok),
                CheckpointLimits::default()
            )
            .is_err(),
            "{section:?}"
        );
        assert!(!path.exists(), "{section:?}");
        assert_eq!(source.exchange_head().unwrap(), head);
        assert_eq!(source.asset(import.asset().id()).unwrap(), *import.asset());
    }
    assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".postproject-import-")
    }));
    let mirror = SqliteProduction::import_checkpoint(
        directory.path().join("valid.pproj"),
        &manifest,
        chunks.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        mirror.representation(import.representation().id()).unwrap(),
        *import.representation()
    );
}

fn contradictions(import: &OriginalMediaImport) -> Vec<(CheckpointSection, Vec<Document>)> {
    let asset = import.asset();
    let representation = import.representation();
    let resource = &import.resources()[0];
    let locator = &import.locators()[0];
    let wrong_asset = Asset::new(
        asset.id(),
        asset.created_at(),
        Some("Different".into()),
        None,
    );
    let wrong_role =
        RepresentationHeader::new(representation.id(), asset.id(), RepresentationKind::Proxy)
            .unwrap();
    let wrong_locator = Locator::new(
        locator.id(),
        resource.id(),
        "file:///different.mov",
        None,
        LocatorAvailability::Offline,
    )
    .unwrap();
    let wrong_fingerprint = FingerprintObservation::new(
        ObjectRef::Resource(resource.id()),
        FingerprintSnapshot::new("content", 1, vec![2], Some(1)).unwrap(),
        FingerprintState::Current,
    )
    .unwrap();
    let wrong_identifier =
        IdentifierAttachment::new(ObjectRef::Asset(asset.id()), identifier("Other")).unwrap();
    vec![
        (CheckpointSection::Assets, vec![encode_asset(&wrong_asset)]),
        (
            CheckpointSection::Resources,
            vec![
                ResourceHeader::from_resource(&Resource::new(
                    resource.id(),
                    Vec::new(),
                    Some(FileFacts::new(999, None)),
                ))
                .document(),
            ],
        ),
        (
            CheckpointSection::Representations,
            vec![wrong_role.document().unwrap()],
        ),
        (CheckpointSection::Structures, Vec::new()),
        (
            CheckpointSection::Locators,
            vec![encode_locator(&wrong_locator).unwrap()],
        ),
        (
            CheckpointSection::Identifiers,
            vec![wrong_identifier.document().unwrap()],
        ),
        (
            CheckpointSection::Fingerprints,
            vec![wrong_fingerprint.document().unwrap()],
        ),
        (CheckpointSection::ConflictVersions, Vec::new()),
    ]
}

fn identifier(value: &str) -> ExternalIdentifier {
    ExternalIdentifier::new(
        IdentifierScheme::new("unknown:CASE").unwrap(),
        value,
        Some("Qual".into()),
    )
    .unwrap()
}

fn fixture(path: &std::path::Path) -> (SqliteProduction, OriginalMediaImport) {
    let mut source = SqliteProduction::create(path, None).unwrap();
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(-1), None, None);
    let resource = Resource::new(
        ResourceId::new(),
        vec![ResourceFingerprint::new("content", 1, vec![1]).unwrap()],
        Some(FileFacts::new(123, None)),
    );
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        ContentStructure::single_resource(resource.id()),
        Vec::new(),
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource.id(),
        "file:///missing.mov",
        None,
        LocatorAvailability::Offline,
    )
    .unwrap();
    let import =
        OriginalMediaImport::new(asset, representation, vec![resource], vec![locator]).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.add_external_identifier(
        ObjectRef::Asset(import.asset().id()),
        &identifier("Exact 名"),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    (source, import)
}
