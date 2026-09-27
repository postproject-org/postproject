//! Candidate scoring coverage for technical inspection evidence.

use std::{ffi::OsStr, fs, path::Path};

use postproject_core::{
    ContentStructure, EvidenceKind, FileFacts, Locator, LocatorAvailability, LocatorId,
    MetadataAssertion, MetadataField, MetadataProperty, MetadataValue, PropertyId, Resource,
    ResourceId, ResourceResolutionState, VocabularyId,
};
use postproject_media::{
    InspectionOutcome, MediaInspector, MediaResolver, ResolutionItem, SearchScope,
    TECHNICAL_INSPECTION_PROPERTY, TECHNICAL_METADATA_VOCABULARY, TechnicalMetadata,
    canonical_file_uri, prepare_media_root,
};

fn technical_metadata(label: &str) -> TechnicalMetadata {
    let value = MetadataValue::structure(vec![MetadataField::new(
        PropertyId::new("profile").expect("property ID"),
        MetadataValue::string(label).expect("profile value"),
    )])
    .expect("technical structure");
    let assertion = MetadataAssertion::new(
        MetadataProperty::new(
            VocabularyId::new(TECHNICAL_METADATA_VOCABULARY).expect("technical vocabulary"),
            PropertyId::new(TECHNICAL_INSPECTION_PROPERTY).expect("inspection property"),
        ),
        value,
    );
    TechnicalMetadata::from_assertions(&[assertion]).expect("technical metadata")
}

struct FixtureInspector {
    matching: TechnicalMetadata,
    other: TechnicalMetadata,
}

impl MediaInspector for FixtureInspector {
    fn inspect(&self, path: &Path) -> postproject_core::Result<InspectionOutcome> {
        let metadata = if path.parent().and_then(Path::file_name) == Some(OsStr::new("z_match")) {
            self.matching.clone()
        } else {
            self.other.clone()
        };
        Ok(InspectionOutcome::Inspected(metadata))
    }
}

#[test]
fn technical_evidence_orders_a_misleading_filename_match() {
    let directory = tempfile::tempdir().expect("create directory");
    let original_directory = directory.path().join("old");
    let root_directory = directory.path().join("new");
    fs::create_dir_all(&original_directory).expect("create original directory");
    fs::create_dir_all(root_directory.join("a_wrong")).expect("create wrong directory");
    fs::create_dir_all(root_directory.join("z_match")).expect("create matching directory");
    let original = original_directory.join("clip.mov");
    fs::write(&original, b"same-size").expect("write original");
    let resource_id = ResourceId::new();
    let resource = Resource::new(resource_id, Vec::new(), Some(FileFacts::new(9, None)));
    let structure = ContentStructure::single_resource(resource_id);
    let locator = Locator::new(
        LocatorId::new(),
        resource_id,
        canonical_file_uri(&original).expect("original URI"),
        None,
        LocatorAvailability::Online,
    )
    .expect("locator");
    fs::remove_file(&original).expect("remove original");
    for path in [
        root_directory.join("a_wrong/clip.mov"),
        root_directory.join("z_match/clip.mov"),
    ] {
        fs::write(path, b"same-size").expect("write candidate");
    }
    let expected = technical_metadata("A001");
    let inspector = FixtureInspector {
        matching: expected.clone(),
        other: technical_metadata("B002"),
    };
    let root = prepare_media_root(&root_directory, None, 0).expect("root");

    let locators = [locator];
    let item = ResolutionItem::new(&resource, &structure, &locators)
        .with_technical_evidence(&expected, &inspector);
    let resolution = MediaResolver::default()
        .resolve(&[item], &SearchScope::new(vec![root], Vec::new()))
        .expect("resolve candidates")
        .remove(0);

    assert_eq!(resolution.state(), ResourceResolutionState::Ambiguous);
    assert!(resolution.candidates()[0].uri().contains("z_match"));
    assert!(resolution.candidates()[0].confidence() > resolution.candidates()[1].confidence());
    assert!(
        resolution.candidates()[0]
            .evidence()
            .iter()
            .any(|evidence| {
                evidence.kind() == EvidenceKind::PartialFingerprintMatch
                    && evidence.detail() == Some("technical media profile matched")
            })
    );
}
