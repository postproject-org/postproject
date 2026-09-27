//! Content observation and verification for one stored resource.

use std::path::Path;

use postproject_core::{
    ContentStructure, Error, ErrorKind, MAX_QUERY_PAGE_SIZE, ProductionRead, QueryPageRequest,
    Representation, RepresentationFingerprint, RepresentationId, Resource, ResourceFingerprint,
    ResourceId, Result,
};

use crate::{
    SEQUENCE_FINGERPRINT_ALGORITHM, SEQUENCE_FINGERPRINT_VERSION,
    fingerprint::is_file_fingerprint_domain, fingerprint_file, fingerprint_image_sequence,
    fingerprint_representation,
};

/// Result of comparing present content with a resource's stored fingerprints.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ContentVerification {
    /// The content matches a stored fingerprint in a domain this crate computes.
    Matches,
    /// The content differs from the stored fingerprints this crate can compute.
    Differs,
    /// No stored fingerprint lies in a domain this crate computes, so nothing
    /// was compared. Foreign fingerprints are left for their owner to check.
    NotComparable,
}

/// Fingerprints recomputed for one resource and every representation using it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentObservation {
    resource: ResourceFingerprint,
    representations: Vec<(RepresentationId, RepresentationFingerprint)>,
}

impl ContentObservation {
    /// Returns the new resource fingerprint.
    #[must_use]
    pub const fn resource(&self) -> &ResourceFingerprint {
        &self.resource
    }

    /// Returns the recomputed fingerprint of each representation using the
    /// resource, in the order the representations were supplied.
    #[must_use]
    pub fn representations(&self) -> &[(RepresentationId, RepresentationFingerprint)] {
        &self.representations
    }
}

/// Loads every representation using a resource together with its complete
/// stored resources, as input for [`observe_resource_content`].
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidArgument`] when more than
/// [`MAX_QUERY_PAGE_SIZE`] representations share the resource, and the
/// production's read errors, including an absent resource.
pub fn resource_usage<R: ProductionRead + ?Sized>(
    production: &R,
    resource_id: ResourceId,
) -> Result<Vec<(Representation, Vec<Resource>)>> {
    let page = production.representations_using_resource(
        resource_id,
        &QueryPageRequest::new(MAX_QUERY_PAGE_SIZE, None)?,
    )?;
    if page.next_cursor().is_some() {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("resource is used by more than {MAX_QUERY_PAGE_SIZE} representations"),
        ));
    }
    page.items()
        .iter()
        .map(|representation| {
            production
                .resources(representation.id())
                .map(|resources| (representation.clone(), resources))
        })
        .collect()
}

/// Computes the fingerprint of the present content of one resource.
///
/// A file resource is fingerprinted as a file. An image-sequence resource is
/// fingerprinted as the sequence directory at `path`, using the structure's
/// descriptor.
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidArgument`] when the resource is not part of
/// `structure`, and the fingerprinting errors of [`fingerprint_file`] or
/// [`fingerprint_image_sequence`] for unreadable or unstable content.
pub fn fingerprint_resource_content(
    resource_id: ResourceId,
    structure: &ContentStructure,
    path: impl AsRef<Path>,
) -> Result<ResourceFingerprint> {
    if !structure.resource_ids().contains(&resource_id) {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "resource does not belong to the supplied content structure",
        ));
    }
    match structure
        .image_sequence_descriptor()
        .filter(|descriptor| descriptor.resource_id() == resource_id)
    {
        Some(descriptor) => Ok(fingerprint_image_sequence(path, descriptor)?
            .fingerprint()
            .clone()),
        None => Ok(fingerprint_file(path)?.fingerprint().clone()),
    }
}

/// Compares present content with a resource's stored fingerprints.
///
/// Only domains this crate computes are compared (ADR 0028). The file domain
/// is chosen by size, so present content that selects a domain other than the
/// stored comparable ones differs in size and is reported as
/// [`ContentVerification::Differs`]. Verification never records anything.
///
/// # Errors
///
/// Returns the errors of [`fingerprint_resource_content`].
pub fn verify_resource_content(
    resource: &Resource,
    structure: &ContentStructure,
    path: impl AsRef<Path>,
) -> Result<ContentVerification> {
    let sequence = structure
        .image_sequence_descriptor()
        .is_some_and(|descriptor| descriptor.resource_id() == resource.id());
    let comparable = resource
        .fingerprints()
        .iter()
        .filter(|fingerprint| {
            if sequence {
                fingerprint.algorithm() == SEQUENCE_FINGERPRINT_ALGORITHM
                    && fingerprint.version() == SEQUENCE_FINGERPRINT_VERSION
            } else {
                is_file_fingerprint_domain(fingerprint)
            }
        })
        .collect::<Vec<_>>();
    if comparable.is_empty() {
        return Ok(ContentVerification::NotComparable);
    }
    let present = fingerprint_resource_content(resource.id(), structure, path)?;
    Ok(if comparable.contains(&&present) {
        ContentVerification::Matches
    } else {
        ContentVerification::Differs
    })
}

/// Fingerprints present content for a resource and recomputes every
/// representation that uses it.
///
/// `representations` supplies each representation using the resource together
/// with its complete stored resources. The new value replaces the stored one in
/// its domain before representation fingerprints are recomputed, so recording
/// the returned observation leaves no representation pending recomputation.
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidArgument`] when `representations` is empty or a
/// representation does not use the resource, and the errors of
/// [`fingerprint_resource_content`] and [`fingerprint_representation`].
pub fn observe_resource_content(
    resource_id: ResourceId,
    representations: &[(Representation, Vec<Resource>)],
    path: impl AsRef<Path>,
) -> Result<ContentObservation> {
    let Some((first, _)) = representations.first() else {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "resource is not used by any representation",
        ));
    };
    let resource = fingerprint_resource_content(resource_id, first.content_structure(), path)?;
    let representations = representations
        .iter()
        .map(|(representation, resources)| {
            let mut resources = resources.clone();
            let stored = resources
                .iter_mut()
                .find(|candidate| candidate.id() == resource_id)
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidArgument,
                        "representation does not use the observed resource",
                    )
                })?;
            *stored = stored.with_observed_fingerprint(&resource);
            fingerprint_representation(representation.content_structure(), &resources)
                .map(|fingerprint| (representation.id(), fingerprint))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ContentObservation {
        resource,
        representations,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::prepare_original_media;

    #[test]
    fn verification_distinguishes_match_difference_and_foreign_evidence() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = directory.path().join("clip.mov");
        fs::write(&path, b"original").expect("write media");
        let prepared = prepare_original_media(&path, None, None).expect("prepare import");
        let resource = &prepared.resources()[0];
        let structure = prepared.representation().content_structure();

        assert_eq!(
            verify_resource_content(resource, structure, &path).expect("verify"),
            ContentVerification::Matches
        );
        fs::write(&path, b"replaced").expect("replace media");
        assert_eq!(
            verify_resource_content(resource, structure, &path).expect("verify"),
            ContentVerification::Differs
        );
        let foreign = Resource::new(
            resource.id(),
            vec![ResourceFingerprint::new("example-host-md5", 1, vec![1; 16]).expect("foreign")],
            resource.file_facts(),
        );
        assert_eq!(
            verify_resource_content(&foreign, structure, &path).expect("verify"),
            ContentVerification::NotComparable
        );
    }

    #[test]
    fn observation_recomputes_the_representation_from_the_new_value() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = directory.path().join("clip.mov");
        fs::write(&path, b"original").expect("write media");
        let prepared = prepare_original_media(&path, None, None).expect("prepare import");
        let representation = prepared.representation().clone();
        let resource_id = prepared.resources()[0].id();
        fs::write(&path, b"replaced").expect("replace media");

        let observation = observe_resource_content(
            resource_id,
            &[(representation.clone(), prepared.resources().to_vec())],
            &path,
        )
        .expect("observe content");

        let expected_resource = fingerprint_file(&path).expect("fingerprint");
        assert_eq!(observation.resource(), expected_resource.fingerprint());
        let updated = prepared.resources()[0].with_observed_fingerprint(observation.resource());
        let expected_representation =
            fingerprint_representation(representation.content_structure(), &[updated])
                .expect("representation fingerprint");
        assert_eq!(
            observation.representations(),
            &[(representation.id(), expected_representation)]
        );
        assert_ne!(
            observation.representations()[0].1,
            representation.fingerprints()[0]
        );
    }
}
