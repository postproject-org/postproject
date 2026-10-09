use postproject_core::{
    FingerprintSnapshot, Locator, ObjectRef, OriginalMediaImport, Representation,
    RepresentationImport, Resource,
};
use serde_json::json;

use super::{RepresentationCreationStart, ResourceCreationStart};
use crate::{
    Document, FingerprintObservation, FingerprintState, Result, encode_asset, encode_locator,
    encode_structure, fields::checked, fingerprint::revision_sequence,
};

/// Streams a complete authored representation creation, retaining initial evidence.
///
/// Order: creation header, representation fingerprints, complete structure,
/// each resource header and its fingerprints, then locators. Each value is one
/// independently framed document; aggregate size has no proposal-command cap.
/// `sequence` is the original committed observation boundary, not current time.
///
/// # Errors
/// Rejects invalid original revision boundaries or unsupported domain variants.
pub fn encode_representation_creation(
    import: &RepresentationImport,
    sequence: u64,
) -> Result<impl Iterator<Item = Result<Document>> + '_> {
    representation_frames(
        RepresentationCreationStart::from_import(import)?,
        import.representation(),
        import.resources(),
        import.locators(),
        sequence,
    )
}

/// Streams one original import effect, including its exact asset header.
///
/// A bounded `original.creation` header precedes the complete representation
/// creation. The receiver must check that it belongs to this asset and is original.
///
/// # Errors
/// Rejects invalid original revision boundaries or unsupported domain variants.
pub fn encode_original_creation(
    import: &OriginalMediaImport,
    sequence: u64,
) -> Result<impl Iterator<Item = Result<Document>> + '_> {
    let header = Document {
        value: json!({"kind":"original.creation", "asset":encode_asset(import.asset()).value}),
    };
    Ok(std::iter::once(Ok(header)).chain(representation_frames(
        RepresentationCreationStart::from_original(import)?,
        import.representation(),
        import.resources(),
        import.locators(),
        sequence,
    )?))
}

fn representation_frames<'a>(
    start: RepresentationCreationStart,
    representation: &'a Representation,
    resources: &'a [Resource],
    locators: &'a [Locator],
    sequence: u64,
) -> Result<impl Iterator<Item = Result<Document>> + 'a> {
    revision_sequence(sequence)?;
    let header = start.document()?;
    let target = ObjectRef::Representation(representation.id());
    let fingerprints = representation
        .fingerprints()
        .iter()
        .map(move |fingerprint| {
            observation(
                target,
                fingerprint.algorithm(),
                fingerprint.version(),
                fingerprint.value(),
                sequence,
            )
        });
    let structure = encode_structure(representation)?;
    let resources = resources
        .iter()
        .flat_map(move |resource| resource_frames(resource, sequence));
    let locators = locators.iter().map(encode_locator);
    Ok(std::iter::once(Ok(header))
        .chain(fingerprints)
        .chain(structure)
        .chain(resources)
        .chain(locators))
}

fn resource_frames(
    resource: &Resource,
    sequence: u64,
) -> impl Iterator<Item = Result<Document>> + '_ {
    let header =
        ResourceCreationStart::from_resource(resource).map(ResourceCreationStart::document);
    let target = ObjectRef::Resource(resource.id());
    let fingerprints = resource.fingerprints().iter().map(move |fingerprint| {
        observation(
            target,
            fingerprint.algorithm(),
            fingerprint.version(),
            fingerprint.value(),
            sequence,
        )
    });
    std::iter::once(header).chain(fingerprints)
}

fn observation(
    target: ObjectRef,
    algorithm: &str,
    version: u16,
    bytes: &[u8],
    sequence: u64,
) -> Result<Document> {
    FingerprintObservation::new(
        target,
        checked(FingerprintSnapshot::new(
            algorithm,
            version,
            bytes.to_vec(),
            Some(sequence),
        ))?,
        FingerprintState::Current,
    )?
    .document()
}
