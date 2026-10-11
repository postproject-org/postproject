//! The existing 10,000-asset native benchmark shape.

use postproject_core::{
    Asset, AssetId, ContentStructure, Locator, LocatorAvailability, LocatorId, OriginalMediaImport,
    Representation, RepresentationFingerprint, RepresentationId, RepresentationKind, Resource,
    ResourceFingerprint, ResourceId, Timestamp,
};

pub(super) fn synthetic_import(index: usize) -> OriginalMediaImport {
    let asset = Asset::new(
        AssetId::new(),
        Timestamp::from_unix_micros(i64::try_from(index).expect("benchmark index fits i64")),
        None,
        Some("benchmark".to_owned()),
    );
    let resource_id = ResourceId::new();
    let fingerprint_value = u64::try_from(index)
        .expect("benchmark index fits u64")
        .to_le_bytes()
        .to_vec();
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        ContentStructure::single_resource(resource_id),
        vec![
            RepresentationFingerprint::new(
                "benchmark-representation",
                1,
                fingerprint_value.clone(),
            )
            .expect("construct benchmark representation fingerprint"),
        ],
    );
    let resource = Resource::new(
        resource_id,
        vec![
            ResourceFingerprint::new("benchmark-resource", 1, fingerprint_value)
                .expect("construct benchmark resource fingerprint"),
        ],
        None,
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource_id,
        format!("file:///benchmark/{index}.mov"),
        None,
        LocatorAvailability::Unknown,
    )
    .expect("construct benchmark locator");
    OriginalMediaImport::new(asset, representation, vec![resource], vec![locator])
        .expect("construct benchmark import")
}
