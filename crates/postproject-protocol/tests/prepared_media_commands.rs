//! Prepared aggregate intent round-trips without authored revision fields.

use postproject_core::{
    Asset, AssetId, ContentStructure, FileFacts, FrameRange, ImageSequenceDescriptor, Locator,
    LocatorAvailability, LocatorId, OriginalMediaImport, RationalRate, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationImport, RepresentationKind,
    Resource, ResourceFingerprint, ResourceId, ResourceMember, ResourceRole, SequenceNaming,
    Timestamp,
};
use postproject_protocol::{Command, Document, FailureKind, Limits};

fn fixture(content: ContentStructure) -> OriginalMediaImport {
    let asset = Asset::new(
        AssetId::new(),
        Timestamp::from_unix_micros(i64::MIN),
        Some("  名\0  ".into()),
        Some("Unknown:Exact".into()),
    );
    let resources: Vec<_> = content
        .resource_ids()
        .into_iter()
        .rev()
        .map(|id| {
            Resource::new(
                id,
                vec![ResourceFingerprint::new("Unknown_R", u16::MAX, vec![0, 255]).unwrap()],
                Some(FileFacts::new(
                    u64::MAX,
                    Some(Timestamp::from_unix_micros(i64::MAX)),
                )),
            )
        })
        .collect();
    let locators = resources
        .iter()
        .map(|resource| {
            let locator = Locator::new(
                LocatorId::new(),
                resource.id(),
                "file:///missing/名",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap();
            if content.image_sequence_descriptor().is_some() {
                locator.with_sequence_naming(SequenceNaming::new("Exact.", ".exr", 4).unwrap())
            } else {
                locator
            }
        })
        .collect();
    OriginalMediaImport::new(
        asset.clone(),
        Representation::new(
            RepresentationId::new(),
            asset.id(),
            RepresentationKind::Original,
            content,
            vec![RepresentationFingerprint::new("Unknown_P", 0, vec![255, 0]).unwrap()],
        ),
        resources,
        locators,
    )
    .unwrap()
}

fn shapes() -> Vec<ContentStructure> {
    let first = ResourceId::new();
    let second = ResourceId::new();
    let member = |id, required| {
        ResourceMember::new(id, ResourceRole::new("unknown:Exact").unwrap(), required)
    };
    vec![
        ContentStructure::single_resource(first),
        ContentStructure::image_sequence(
            ImageSequenceDescriptor::new(
                first,
                FrameRange::new(-10, 10, 2).unwrap(),
                RationalRate::new(24000, 1001).unwrap(),
                vec![-2, 4],
            )
            .unwrap(),
        ),
        ContentStructure::ordered_parts(vec![member(second, true), member(first, true)]).unwrap(),
        ContentStructure::package(vec![member(first, true), member(second, false)]).unwrap(),
    ]
}

#[test]
fn original_and_representation_intent_preserve_all_four_shapes_and_exact_evidence() {
    for shape in shapes() {
        let original = fixture(shape);
        let (_, representation, resources, locators) = original.clone().into_parts();
        let proxy = RepresentationImport::new(
            Representation::new(
                representation.id(),
                representation.asset_id(),
                RepresentationKind::Proxy,
                representation.content_structure().clone(),
                representation.fingerprints().to_vec(),
            ),
            resources,
            locators,
        )
        .unwrap();
        for command in [
            Command::ImportOriginal(original),
            Command::AddRepresentation(proxy),
        ] {
            let bytes = command.document().unwrap().canonical_bytes().unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            assert!(!text.contains("observed_revision"));
            assert!(!text.contains("authority_time"));
            let decoded =
                Command::from_document(&Document::parse(&bytes, Limits::default()).unwrap())
                    .unwrap();
            assert_eq!(decoded, command);
            assert_eq!(
                decoded.document().unwrap().canonical_bytes().unwrap(),
                bytes
            );
        }
    }
}

fn reject(value: &serde_json::Value) {
    let document = Document::parse(&serde_json::to_vec(value).unwrap(), Limits::default()).unwrap();
    assert_eq!(
        Command::from_document(&document).unwrap_err().kind(),
        FailureKind::Malformed
    );
}

#[test]
fn inconsistent_owners_incomplete_aggregates_and_authority_fields_reject() {
    let original = fixture(shapes().remove(2));
    let value: serde_json::Value = serde_json::from_slice(
        &Command::ImportOriginal(original)
            .document()
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    let mut wrong_owner = value.clone();
    wrong_owner["media"]["structure"][0]["representation_id"] =
        ResourceId::new().to_string().into();
    reject(&wrong_owner);
    let mut extra_resource = value.clone();
    let duplicate = extra_resource["media"]["resources"][0].clone();
    extra_resource["media"]["resources"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    reject(&extra_resource);
    let mut missing_member = value.clone();
    missing_member["media"]["structure"]
        .as_array_mut()
        .unwrap()
        .pop();
    reject(&missing_member);
    let mut missing_locator = value.clone();
    missing_locator["media"]["locators"] = serde_json::json!([]);
    reject(&missing_locator);
    for key in [
        "observed_revision_sequence",
        "authority_time_micros",
        "claim_id",
    ] {
        let mut injected = value.clone();
        injected["media"]["fingerprints"][0][key] = "1".into();
        reject(&injected);
    }
    for bytes in ["AP9=", "AP8", "AP8=\n", "_w=="] {
        let mut bad_bytes = value.clone();
        bad_bytes["media"]["resources"][0]["fingerprints"][0]["value"] = bytes.into();
        reject(&bad_bytes);
    }
}
