//! Deterministic representation fingerprints over canonical content structure.

use std::collections::BTreeMap;

use postproject_core::{
    ContentStructure, ContentStructureKind, Error, ErrorKind, RepresentationFingerprint, Resource,
    ResourceId, Result,
};

/// Algorithm identifier for structure-aware representation fingerprints.
pub const REPRESENTATION_FINGERPRINT_ALGORITHM: &str = "pp-blake3-representation";
/// Current canonical representation-fingerprint strategy version.
///
/// Version 2 no longer hashes image-sequence file names (ADR 0038). Version 1
/// values are no longer computed.
pub const REPRESENTATION_FINGERPRINT_VERSION: u16 = 2;

const CONTEXT: &str = "postproject.org representation fingerprint v2";

/// Computes a structure-aware fingerprint from resource fingerprint evidence.
///
/// Internal resource IDs and locators are deliberately excluded: importing the
/// same content into another production must produce the same result. Package
/// membership is order-independent, while ordered parts retain their order.
/// Image sequences hash their compact descriptor, without file names, and do
/// not enumerate frames.
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidArgument`] when resources are missing,
/// duplicated, unreferenced, or lack fingerprint evidence.
pub fn fingerprint_representation(
    structure: &ContentStructure,
    resources: &[Resource],
) -> Result<RepresentationFingerprint> {
    let expected = structure.resource_ids();
    let mut by_id = BTreeMap::new();
    for resource in resources {
        if resource.fingerprints().is_empty() || by_id.insert(resource.id(), resource).is_some() {
            return Err(invalid_resources());
        }
    }
    if expected.len() != by_id.len() || expected.iter().any(|id| !by_id.contains_key(id)) {
        return Err(invalid_resources());
    }

    let mut hasher = blake3::Hasher::new_derive_key(CONTEXT);
    match structure.kind() {
        ContentStructureKind::SingleResource => {
            hasher.update(&[1]);
            hash_resource(
                &mut hasher,
                resource(structure.single_resource_id(), &by_id)?,
            );
        }
        ContentStructureKind::ImageSequence => {
            hasher.update(&[2]);
            let descriptor = structure
                .image_sequence_descriptor()
                .ok_or_else(invalid_structure)?;
            hasher.update(&descriptor.frames().start().to_le_bytes());
            hasher.update(&descriptor.frames().end().to_le_bytes());
            hasher.update(&descriptor.frames().step().to_le_bytes());
            hasher.update(&descriptor.rate().numerator().to_le_bytes());
            hasher.update(&descriptor.rate().denominator().to_le_bytes());
            hash_i64_values(&mut hasher, descriptor.known_missing_frames());
            hash_resource(
                &mut hasher,
                resource(Some(descriptor.resource_id()), &by_id)?,
            );
        }
        ContentStructureKind::OrderedParts | ContentStructureKind::Package => {
            let members = structure.members().ok_or_else(invalid_structure)?;
            let mut tokens = members
                .iter()
                .map(|member| {
                    let mut member_hasher = blake3::Hasher::new_derive_key(CONTEXT);
                    hash_text(&mut member_hasher, member.role().as_str());
                    member_hasher.update(&[u8::from(member.is_required())]);
                    hash_resource(
                        &mut member_hasher,
                        resource(Some(member.resource_id()), &by_id)?,
                    );
                    Ok(*member_hasher.finalize().as_bytes())
                })
                .collect::<Result<Vec<_>>>()?;
            if structure.kind() == ContentStructureKind::Package {
                hasher.update(&[4]);
                tokens.sort_unstable();
            } else {
                hasher.update(&[3]);
            }
            hash_count(&mut hasher, tokens.len());
            for token in tokens {
                hasher.update(&token);
            }
        }
        _ => return Err(invalid_structure()),
    }
    RepresentationFingerprint::new(
        REPRESENTATION_FINGERPRINT_ALGORITHM,
        REPRESENTATION_FINGERPRINT_VERSION,
        hasher.finalize().as_bytes().to_vec(),
    )
}

fn resource<'a>(
    id: Option<ResourceId>,
    resources: &BTreeMap<ResourceId, &'a Resource>,
) -> Result<&'a Resource> {
    id.and_then(|id| resources.get(&id).copied())
        .ok_or_else(invalid_structure)
}

fn hash_resource(hasher: &mut blake3::Hasher, resource: &Resource) {
    let mut fingerprints = resource.fingerprints().iter().collect::<Vec<_>>();
    fingerprints.sort_unstable_by(|left, right| {
        (left.algorithm(), left.version(), left.value()).cmp(&(
            right.algorithm(),
            right.version(),
            right.value(),
        ))
    });
    hash_count(hasher, fingerprints.len());
    for fingerprint in fingerprints {
        hash_text(hasher, fingerprint.algorithm());
        hasher.update(&fingerprint.version().to_le_bytes());
        hash_bytes(hasher, fingerprint.value());
    }
}

fn hash_i64_values(hasher: &mut blake3::Hasher, values: &[i64]) {
    hash_count(hasher, values.len());
    for value in values {
        hasher.update(&value.to_le_bytes());
    }
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hash_bytes(hasher, value.as_bytes());
}

fn hash_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
    hash_count(hasher, value.len());
    hasher.update(value);
}

fn hash_count(hasher: &mut blake3::Hasher, count: usize) {
    hasher.update(&u64::try_from(count).unwrap_or(u64::MAX).to_le_bytes());
}

fn invalid_resources() -> Error {
    Error::new(
        ErrorKind::InvalidArgument,
        "representation resources must exactly match its structure and carry fingerprints",
    )
}

fn invalid_structure() -> Error {
    Error::new(
        ErrorKind::InvalidArgument,
        "representation content structure is inconsistent",
    )
}
