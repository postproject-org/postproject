//! Storage-level resource identity and access values.

use crate::{
    Asset, Error, ErrorKind, LocatorId, MediaRoot, Representation, ResourceId, Result,
    SequenceNaming, Timestamp, uri::normalize_uri,
};

/// Cheap filesystem facts observed for one resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileFacts {
    size_bytes: u64,
    modified_at: Option<Timestamp>,
}

impl FileFacts {
    /// Creates file facts from a byte size and optional modification time.
    #[must_use]
    pub const fn new(size_bytes: u64, modified_at: Option<Timestamp>) -> Self {
        Self {
            size_bytes,
            modified_at,
        }
    }

    /// Returns the observed file size.
    #[must_use]
    pub const fn size_bytes(self) -> u64 {
        self.size_bytes
    }

    /// Returns the observed modification time, when available.
    #[must_use]
    pub const fn modified_at(self) -> Option<Timestamp> {
        self.modified_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FingerprintData {
    algorithm: String,
    version: u16,
    value: Vec<u8>,
}

impl FingerprintData {
    fn new(algorithm: impl Into<String>, version: u16, value: Vec<u8>) -> Result<Self> {
        let algorithm = algorithm.into();
        if algorithm.is_empty()
            || algorithm.len() > 64
            || !algorithm
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "fingerprint algorithm must be 1-64 ASCII letters, digits, '-' or '_'",
            ));
        }
        if value.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "fingerprint value must not be empty",
            ));
        }
        Ok(Self {
            algorithm,
            version,
            value,
        })
    }
}

macro_rules! typed_fingerprint {
    ($(#[$metadata:meta])* $name:ident) => {
        $(#[$metadata])*
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(FingerprintData);

        impl $name {
            /// Creates typed fingerprint evidence.
            ///
            /// # Errors
            ///
            /// Returns an error when the algorithm or value is invalid.
            pub fn new(
                algorithm: impl Into<String>,
                version: u16,
                value: Vec<u8>,
            ) -> Result<Self> {
                FingerprintData::new(algorithm, version, value).map(Self)
            }

            /// Returns the algorithm identifier.
            #[must_use]
            pub fn algorithm(&self) -> &str {
                &self.0.algorithm
            }

            /// Returns the algorithm format version.
            #[must_use]
            pub const fn version(&self) -> u16 {
                self.0.version
            }

            /// Returns the opaque fingerprint bytes.
            #[must_use]
            pub fn value(&self) -> &[u8] {
                &self.0.value
            }
        }
    };
}

typed_fingerprint!(
    /// Versioned identity evidence derived from one storage resource.
    ResourceFingerprint
);
typed_fingerprint!(
    /// Versioned, structure-aware identity evidence for a representation.
    RepresentationFingerprint
);

/// One immutable fingerprint value captured at a semantic revision.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FingerprintSnapshot {
    algorithm: String,
    version: u16,
    value: Vec<u8>,
    observed_revision_sequence: Option<u64>,
}

impl FingerprintSnapshot {
    /// Creates a validated snapshot of one fingerprint domain.
    ///
    /// `observed_revision_sequence` is absent for fingerprints migrated from a
    /// schema that did not record observation revisions.
    ///
    /// # Errors
    ///
    /// Returns an error when the fingerprint algorithm or value is invalid.
    pub fn new(
        algorithm: impl Into<String>,
        version: u16,
        value: Vec<u8>,
        observed_revision_sequence: Option<u64>,
    ) -> Result<Self> {
        let fingerprint = FingerprintData::new(algorithm, version, value)?;
        Ok(Self {
            algorithm: fingerprint.algorithm,
            version: fingerprint.version,
            value: fingerprint.value,
            observed_revision_sequence,
        })
    }

    /// Returns the algorithm identifier.
    #[must_use]
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// Returns the algorithm format version.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Returns the opaque fingerprint bytes.
    #[must_use]
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Returns the revision that recorded this value, when known.
    #[must_use]
    pub const fn observed_revision_sequence(&self) -> Option<u64> {
        self.observed_revision_sequence
    }
}

/// A storage-level component used to realize a representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    id: ResourceId,
    fingerprints: Vec<ResourceFingerprint>,
    file_facts: Option<FileFacts>,
}

impl Resource {
    /// Creates a resource from content evidence known at observation time.
    #[must_use]
    pub fn new(
        id: ResourceId,
        fingerprints: Vec<ResourceFingerprint>,
        file_facts: Option<FileFacts>,
    ) -> Self {
        Self {
            id,
            fingerprints,
            file_facts,
        }
    }

    /// Returns the resource's stable identity.
    #[must_use]
    pub const fn id(&self) -> ResourceId {
        self.id
    }

    /// Returns stored content identity evidence.
    #[must_use]
    pub fn fingerprints(&self) -> &[ResourceFingerprint] {
        &self.fingerprints
    }

    /// Returns cheap stored file facts, when available.
    #[must_use]
    pub const fn file_facts(&self) -> Option<FileFacts> {
        self.file_facts
    }

    /// Returns this resource with `fingerprint` as the current observation in
    /// its algorithm/version domain, replacing any value in that domain.
    #[must_use]
    pub fn with_observed_fingerprint(&self, fingerprint: &ResourceFingerprint) -> Self {
        let mut fingerprints = self
            .fingerprints
            .iter()
            .filter(|current| {
                current.algorithm() != fingerprint.algorithm()
                    || current.version() != fingerprint.version()
            })
            .cloned()
            .collect::<Vec<_>>();
        fingerprints.push(fingerprint.clone());
        Self::new(self.id, fingerprints, self.file_facts)
    }
}

/// Canonical current locator identity used to find known media.
///
/// A single-file identity has no sequence naming. An image-sequence identity
/// combines its directory URI with the exact naming descriptor, so different
/// sequences in one directory remain distinct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatorIdentity {
    uri: String,
    sequence_naming: Option<SequenceNaming>,
}

impl LocatorIdentity {
    /// Creates a canonical locator identity.
    ///
    /// # Errors
    ///
    /// Returns an error when `uri` is invalid or relative.
    pub fn new(uri: impl Into<String>, sequence_naming: Option<SequenceNaming>) -> Result<Self> {
        Ok(Self {
            uri: normalize_uri(uri, "locator identity")?,
            sequence_naming,
        })
    }

    /// Returns the canonical UTF-8 URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the exact sequence naming, when this identifies an image
    /// sequence.
    #[must_use]
    pub const fn sequence_naming(&self) -> Option<&SequenceNaming> {
        self.sequence_naming.as_ref()
    }
}

/// One owning path from known storage evidence to logical production media.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownMediaMatch {
    asset: Asset,
    representation: Representation,
    resource: Resource,
}

impl KnownMediaMatch {
    /// Creates a known-media ownership match.
    #[doc(hidden)]
    #[must_use]
    pub fn new(asset: Asset, representation: Representation, resource: Resource) -> Self {
        Self {
            asset,
            representation,
            resource,
        }
    }

    /// Returns the owning logical asset.
    #[must_use]
    pub const fn asset(&self) -> &Asset {
        &self.asset
    }

    /// Returns the parent representation that uses the resource.
    #[must_use]
    pub const fn representation(&self) -> &Representation {
        &self.representation
    }

    /// Returns the matching storage resource.
    #[must_use]
    pub const fn resource(&self) -> &Resource {
        &self.resource
    }
}

/// The last observed availability of a resource locator.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum LocatorAvailability {
    /// Availability has not been checked.
    Unknown,
    /// The locator resolved when last checked.
    Online,
    /// The locator did not resolve when last checked.
    Offline,
}

/// A URI identifying one access route to a resource.
///
/// A locator of an image-sequence resource names its directory and carries
/// the [`SequenceNaming`] of the files there; no other locator has a naming.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Locator {
    id: LocatorId,
    resource_id: ResourceId,
    uri: String,
    last_seen: Option<Timestamp>,
    availability: LocatorAvailability,
    media_root: Option<String>,
    sequence_naming: Option<SequenceNaming>,
}

impl Locator {
    /// Creates a locator with a syntactically valid absolute URI.
    ///
    /// # Errors
    ///
    /// Returns an error when `uri` is invalid or relative.
    pub fn new(
        id: LocatorId,
        resource_id: ResourceId,
        uri: impl Into<String>,
        last_seen: Option<Timestamp>,
        availability: LocatorAvailability,
    ) -> Result<Self> {
        let uri = normalize_uri(uri, "locator")?;
        Ok(Self {
            id,
            resource_id,
            uri,
            last_seen,
            availability,
            media_root: None,
            sequence_naming: None,
        })
    }

    /// Records how the files of the image sequence at this locator are named.
    #[must_use]
    pub fn with_sequence_naming(mut self, naming: SequenceNaming) -> Self {
        self.sequence_naming = Some(naming);
        self
    }

    /// Associates this locator with the logical root used to discover it.
    ///
    /// # Errors
    ///
    /// Returns an invalid-argument error for an invalid logical root name.
    pub fn with_media_root(mut self, name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        MediaRoot::validate_name(&name)?;
        self.media_root = Some(name);
        Ok(self)
    }

    /// Returns the locator's stable identity.
    #[must_use]
    pub const fn id(&self) -> LocatorId {
        self.id
    }

    /// Returns the resource made accessible by this locator.
    #[must_use]
    pub const fn resource_id(&self) -> ResourceId {
        self.resource_id
    }

    /// Returns the UTF-8 URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns when the locator was last observed online.
    #[must_use]
    pub const fn last_seen(&self) -> Option<Timestamp> {
        self.last_seen
    }

    /// Returns its last observed availability.
    #[must_use]
    pub const fn availability(&self) -> LocatorAvailability {
        self.availability
    }

    /// Returns the logical media root that located this URI, when recorded.
    #[must_use]
    pub fn media_root(&self) -> Option<&str> {
        self.media_root.as_deref()
    }

    /// Returns the naming of the sequence files at this locator, present
    /// exactly for a locator of an image-sequence resource.
    #[must_use]
    pub const fn sequence_naming(&self) -> Option<&SequenceNaming> {
        self.sequence_naming.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locator_belongs_to_a_resource() {
        let resource_id = ResourceId::new();
        let locator = Locator::new(
            LocatorId::new(),
            resource_id,
            "file:///media/clip.mov",
            None,
            LocatorAvailability::Unknown,
        )
        .expect("valid locator");

        assert_eq!(locator.resource_id(), resource_id);
        assert_eq!(locator.uri(), "file:///media/clip.mov");
    }

    #[test]
    fn fingerprint_domains_are_explicit() {
        let resource = ResourceFingerprint::new("blake3", 1, vec![1]).expect("valid");
        let representation =
            RepresentationFingerprint::new("tree-blake3", 1, vec![2]).expect("valid");

        assert_eq!(resource.algorithm(), "blake3");
        assert_eq!(representation.algorithm(), "tree-blake3");
        assert!(ResourceFingerprint::new("contains spaces", 1, vec![1]).is_err());
        assert!(RepresentationFingerprint::new("valid", 1, Vec::new()).is_err());
    }
}
