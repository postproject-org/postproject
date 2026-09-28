//! Content observation and verification for one stored resource.

use std::{collections::BTreeSet, path::Path};

use postproject_core::{
    ContentStructure, Error, ErrorKind, FileFacts, Locator, MAX_QUERY_PAGE_SIZE, ProductionRead,
    QueryPageRequest, Representation, RepresentationFingerprint, RepresentationId, Resource,
    ResourceFingerprint, ResourceId, Result, SequenceNaming,
};

use crate::{
    SEQUENCE_FINGERPRINT_ALGORITHM, SEQUENCE_FINGERPRINT_VERSION, canonical_file_uri,
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

/// How observed content relates to what the production recorded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ContentObservationOutcome {
    /// The content matches a stored fingerprint in a domain this crate
    /// computes. Recording the observation adds no fingerprint.
    Unchanged,
    /// The content differs from the stored fingerprints this crate computes.
    Changed,
    /// No fingerprint in a domain this crate computes was stored, so the
    /// observation records the first one.
    FirstObservation,
}

/// Fingerprints recomputed for one resource and every representation using it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentObservation {
    outcome: ContentObservationOutcome,
    resource: ResourceFingerprint,
    file_facts: Option<FileFacts>,
    representations: Vec<(RepresentationId, RepresentationFingerprint)>,
}

impl ContentObservation {
    /// Returns how the observed content relates to the stored fingerprints.
    #[must_use]
    pub const fn outcome(&self) -> ContentObservationOutcome {
        self.outcome
    }

    /// Returns the new resource fingerprint.
    #[must_use]
    pub const fn resource(&self) -> &ResourceFingerprint {
        &self.resource
    }

    /// Returns the file's size and modification time when the resource is a
    /// file. Recording them keeps size-filtered discovery current.
    #[must_use]
    pub const fn file_facts(&self) -> Option<FileFacts> {
        self.file_facts
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
/// fingerprinted as the files named by `sequence_naming` in the directory at
/// `path`, using the structure's descriptor. The naming is required exactly
/// for an image-sequence resource.
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidArgument`] when the resource is not part of
/// `structure` or `sequence_naming` is supplied for a file or missing for a
/// sequence, and the fingerprinting errors of [`fingerprint_file`] or
/// [`fingerprint_image_sequence`] for unreadable or unstable content.
pub fn fingerprint_resource_content(
    resource_id: ResourceId,
    structure: &ContentStructure,
    path: impl AsRef<Path>,
    sequence_naming: Option<&SequenceNaming>,
) -> Result<ResourceFingerprint> {
    observe_content(resource_id, structure, path, sequence_naming)
        .map(|(fingerprint, _)| fingerprint)
}

fn observe_content(
    resource_id: ResourceId,
    structure: &ContentStructure,
    path: impl AsRef<Path>,
    sequence_naming: Option<&SequenceNaming>,
) -> Result<(ResourceFingerprint, Option<FileFacts>)> {
    if let Some((descriptor, naming)) = content_naming(resource_id, structure, sequence_naming)? {
        let report = fingerprint_image_sequence(path, naming, descriptor)?;
        return Ok((report.fingerprint().clone(), None));
    }
    let report = fingerprint_file(path)?;
    Ok((report.fingerprint().clone(), Some(report.facts())))
}

/// Checks that a resource belongs to `structure` and that a sequence naming is
/// supplied exactly for its image-sequence resource, returning the sequence's
/// descriptor and naming.
fn content_naming<'a>(
    resource_id: ResourceId,
    structure: &'a ContentStructure,
    sequence_naming: Option<&'a SequenceNaming>,
) -> Result<
    Option<(
        &'a postproject_core::ImageSequenceDescriptor,
        &'a SequenceNaming,
    )>,
> {
    if !structure.resource_ids().contains(&resource_id) {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "resource does not belong to the supplied content structure",
        ));
    }
    let descriptor = structure
        .image_sequence_descriptor()
        .filter(|descriptor| descriptor.resource_id() == resource_id);
    match (descriptor, sequence_naming) {
        (Some(descriptor), Some(naming)) => Ok(Some((descriptor, naming))),
        (None, None) => Ok(None),
        (Some(_), None) => Err(Error::new(
            ErrorKind::InvalidArgument,
            "an image-sequence resource needs the sequence naming of its files",
        )),
        (None, Some(_)) => Err(Error::new(
            ErrorKind::InvalidArgument,
            "only an image-sequence resource has a sequence naming",
        )),
    }
}

/// Returns the sequence naming recorded for the directory at `path`.
///
/// Among `locators`, those whose URI names `path` are considered. Their one
/// naming is returned; without such a locator, or for locators without a
/// naming, the result is `None`.
///
/// # Errors
///
/// Returns the path errors of [`canonical_file_uri`], and
/// [`ErrorKind::InvalidArgument`] when the locators at `path` record several
/// namings, so the caller must name the files explicitly.
pub fn recorded_sequence_naming(
    locators: &[Locator],
    path: impl AsRef<Path>,
) -> Result<Option<SequenceNaming>> {
    let uri = canonical_file_uri(path)?;
    let namings = locators
        .iter()
        .filter(|locator| locator.uri() == uri)
        .filter_map(Locator::sequence_naming)
        .collect::<BTreeSet<_>>();
    match namings.len() {
        0 => Ok(None),
        1 => Ok(namings.into_iter().next().cloned()),
        _ => Err(Error::new(
            ErrorKind::InvalidArgument,
            "several sequence namings are recorded for this directory; name the files explicitly",
        )),
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
    sequence_naming: Option<&SequenceNaming>,
) -> Result<ContentVerification> {
    content_naming(resource.id(), structure, sequence_naming)?;
    let comparable = comparable_fingerprints(resource, structure);
    if comparable.is_empty() {
        return Ok(ContentVerification::NotComparable);
    }
    let present = fingerprint_resource_content(resource.id(), structure, path, sequence_naming)?;
    Ok(if comparable.contains(&&present) {
        ContentVerification::Matches
    } else {
        ContentVerification::Differs
    })
}

/// Returns a resource's stored fingerprints in the domains this crate computes
/// for its place in `structure`.
fn comparable_fingerprints<'a>(
    resource: &'a Resource,
    structure: &ContentStructure,
) -> Vec<&'a ResourceFingerprint> {
    let sequence = structure
        .image_sequence_descriptor()
        .is_some_and(|descriptor| descriptor.resource_id() == resource.id());
    resource
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
        .collect()
}

/// Fingerprints present content for a resource and recomputes every
/// representation that uses it.
///
/// `representations` supplies each representation using the resource together
/// with its complete stored resources. The new value replaces the stored one in
/// its domain before representation fingerprints are recomputed, so recording
/// the returned observation leaves no representation pending recomputation.
/// The outcome compares the new value with the stored resource's fingerprints
/// as [`verify_resource_content`] does. Recording an unchanged observation adds
/// no fingerprint and, unless a representation awaits recomputation, no
/// revision.
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
    sequence_naming: Option<&SequenceNaming>,
) -> Result<ContentObservation> {
    let Some((first, _)) = representations.first() else {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "resource is not used by any representation",
        ));
    };
    let (resource, file_facts) = observe_content(
        resource_id,
        first.content_structure(),
        path,
        sequence_naming,
    )?;
    let outcome = representations
        .iter()
        .find_map(|(_, resources)| {
            resources
                .iter()
                .find(|candidate| candidate.id() == resource_id)
        })
        .map_or(ContentObservationOutcome::FirstObservation, |stored| {
            let comparable = comparable_fingerprints(stored, first.content_structure());
            if comparable.is_empty() {
                ContentObservationOutcome::FirstObservation
            } else if comparable.contains(&&resource) {
                ContentObservationOutcome::Unchanged
            } else {
                ContentObservationOutcome::Changed
            }
        });
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
        outcome,
        resource,
        file_facts,
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
            verify_resource_content(resource, structure, &path, None).expect("verify"),
            ContentVerification::Matches
        );
        fs::write(&path, b"replaced").expect("replace media");
        assert_eq!(
            verify_resource_content(resource, structure, &path, None).expect("verify"),
            ContentVerification::Differs
        );
        let foreign = Resource::new(
            resource.id(),
            vec![ResourceFingerprint::new("example-host-md5", 1, vec![1; 16]).expect("foreign")],
            resource.file_facts(),
        );
        assert_eq!(
            verify_resource_content(&foreign, structure, &path, None).expect("verify"),
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
            None,
        )
        .expect("observe content");

        let expected_resource = fingerprint_file(&path).expect("fingerprint");
        assert_eq!(observation.resource(), expected_resource.fingerprint());
        assert_eq!(observation.file_facts(), Some(expected_resource.facts()));
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

    #[test]
    fn observation_reports_unchanged_changed_and_first_content() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = directory.path().join("clip.mov");
        fs::write(&path, b"original").expect("write media");
        let prepared = prepare_original_media(&path, None, None).expect("prepare import");
        let representation = prepared.representation().clone();
        let resource = prepared.resources()[0].clone();
        let outcome = |resources: Vec<Resource>| {
            observe_resource_content(
                resource.id(),
                &[(representation.clone(), resources)],
                &path,
                None,
            )
            .expect("observe content")
            .outcome()
        };

        assert_eq!(
            outcome(vec![resource.clone()]),
            ContentObservationOutcome::Unchanged
        );
        let foreign = Resource::new(
            resource.id(),
            vec![ResourceFingerprint::new("example-host-md5", 1, vec![1; 16]).expect("foreign")],
            resource.file_facts(),
        );
        assert_eq!(
            outcome(vec![foreign]),
            ContentObservationOutcome::FirstObservation
        );
        fs::write(&path, b"replaced").expect("replace media");
        assert_eq!(
            outcome(vec![resource.clone()]),
            ContentObservationOutcome::Changed
        );
    }
}
