//! Core media identity and locator value types.

use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    AssetId, ContentStructure, Error, ErrorKind, Locator, MediaRootId, ProductionId,
    RepresentationFingerprint, RepresentationId, Resource, Result, uri::normalize_uri,
};

/// A UTC instant represented as microseconds since the Unix epoch.
///
/// The representation is independent of SQLite and has sufficient precision for
/// filesystem and domain bookkeeping without exposing a third-party time type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Creates a timestamp from signed Unix microseconds.
    #[must_use]
    pub const fn from_unix_micros(micros: i64) -> Self {
        Self(micros)
    }

    /// Returns the signed Unix-microsecond representation.
    #[must_use]
    pub const fn as_unix_micros(self) -> i64 {
        self.0
    }

    /// Reads the current system time.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Internal`] if the platform clock predates the Unix
    /// epoch or cannot be represented in signed microseconds.
    pub fn now() -> Result<Self> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                Error::new(
                    ErrorKind::Internal,
                    format!("system clock is before the Unix epoch: {error}"),
                )
            })?;
        let micros = i64::try_from(duration.as_micros()).map_err(|error| {
            Error::new(
                ErrorKind::Internal,
                format!("system clock is outside the supported range: {error}"),
            )
        })?;
        Ok(Self(micros))
    }
}

/// Production identity and header metadata; mutable collections use read queries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Production {
    id: ProductionId,
    schema_version: u32,
    created_at: Timestamp,
    display_name: Option<String>,
}

impl Production {
    /// Creates an in-memory production value.
    #[must_use]
    pub fn new(
        id: ProductionId,
        schema_version: u32,
        created_at: Timestamp,
        display_name: Option<String>,
    ) -> Self {
        Self {
            id,
            schema_version,
            created_at,
            display_name,
        }
    }

    /// Returns the production's stable identity.
    #[must_use]
    pub const fn id(&self) -> ProductionId {
        self.id
    }

    /// Returns the persistence schema version used to load this production.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns when the production was created.
    #[must_use]
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns the optional user-facing name.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
}

/// Logical identity for one piece of production media.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    id: AssetId,
    created_at: Timestamp,
    display_name: Option<String>,
    import_source: Option<String>,
}

impl Asset {
    /// Creates an asset value. Paths belong to locators, not assets.
    #[must_use]
    pub fn new(
        id: AssetId,
        created_at: Timestamp,
        display_name: Option<String>,
        import_source: Option<String>,
    ) -> Self {
        Self {
            id,
            created_at,
            display_name,
            import_source,
        }
    }

    /// Returns the asset's stable identity.
    #[must_use]
    pub const fn id(&self) -> AssetId {
        self.id
    }

    /// Returns when the asset was created.
    #[must_use]
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns the optional user-facing name.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Returns optional application-supplied import provenance.
    #[must_use]
    pub fn import_source(&self) -> Option<&str> {
        self.import_source.as_deref()
    }
}

/// The semantic role of an asset representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum RepresentationKind {
    /// Source media as imported.
    Original,
    /// A lower-cost representation intended for interactive work.
    Proxy,
    /// A representation optimized for a particular workflow.
    Optimized,
    /// Media derived from another production operation.
    Derived,
}

/// One encoded or derived form of an asset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Representation {
    id: RepresentationId,
    asset_id: AssetId,
    kind: RepresentationKind,
    content_structure: ContentStructure,
    fingerprints: Vec<RepresentationFingerprint>,
}

impl Representation {
    /// Creates a representation value.
    #[must_use]
    pub fn new(
        id: RepresentationId,
        asset_id: AssetId,
        kind: RepresentationKind,
        content_structure: ContentStructure,
        fingerprints: Vec<RepresentationFingerprint>,
    ) -> Self {
        Self {
            id,
            asset_id,
            kind,
            content_structure,
            fingerprints,
        }
    }

    /// Returns the representation's stable identity.
    #[must_use]
    pub const fn id(&self) -> RepresentationId {
        self.id
    }

    /// Returns the owning asset identity.
    #[must_use]
    pub const fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// Returns the representation's semantic role.
    #[must_use]
    pub const fn kind(&self) -> RepresentationKind {
        self.kind
    }

    /// Returns how storage resources realize this representation.
    #[must_use]
    pub const fn content_structure(&self) -> &ContentStructure {
        &self.content_structure
    }

    /// Returns structure-aware identity evidence for this representation.
    #[must_use]
    pub fn fingerprints(&self) -> &[RepresentationFingerprint] {
        &self.fingerprints
    }
}

/// A validated representation and the storage resources that realize it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepresentationImport {
    representation: Representation,
    resources: Vec<Resource>,
    locators: Vec<Locator>,
}

impl RepresentationImport {
    /// Creates an import aggregate whose resource relationships are consistent.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] if a referenced resource is absent
    /// or duplicated, an extra resource is supplied, any resource lacks a
    /// locator, or a locator has a sequence naming although it does not locate
    /// the image-sequence resource, or lacks one although it does.
    pub fn new(
        representation: Representation,
        resources: Vec<Resource>,
        locators: Vec<Locator>,
    ) -> Result<Self> {
        let expected: BTreeSet<_> = representation
            .content_structure()
            .resource_ids()
            .into_iter()
            .collect();
        let supplied: BTreeSet<_> = resources.iter().map(Resource::id).collect();
        if expected != supplied || supplied.len() != resources.len() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "representation resources do not exactly match the content structure",
            ));
        }
        if locators
            .iter()
            .any(|locator| !supplied.contains(&locator.resource_id()))
            || supplied.iter().any(|resource_id| {
                !locators
                    .iter()
                    .any(|item| item.resource_id() == *resource_id)
            })
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "every representation resource must own at least one supplied locator",
            ));
        }
        let sequence_resource = representation
            .content_structure()
            .image_sequence_descriptor()
            .map(crate::ImageSequenceDescriptor::resource_id);
        if locators.iter().any(|locator| {
            locator.sequence_naming().is_some()
                != (Some(locator.resource_id()) == sequence_resource)
        }) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "exactly the locators of an image-sequence resource must carry a sequence naming",
            ));
        }
        Ok(Self {
            representation,
            resources,
            locators,
        })
    }

    /// Returns the representation being imported.
    #[must_use]
    pub const fn representation(&self) -> &Representation {
        &self.representation
    }

    /// Returns the storage resources realizing the representation.
    #[must_use]
    pub fn resources(&self) -> &[Resource] {
        &self.resources
    }

    /// Returns the known access routes for the imported resources.
    #[must_use]
    pub fn locators(&self) -> &[Locator] {
        &self.locators
    }

    /// Splits the aggregate into persistable domain values.
    #[must_use]
    pub fn into_parts(self) -> (Representation, Vec<Resource>, Vec<Locator>) {
        (self.representation, self.resources, self.locators)
    }
}

/// A validated aggregate representing an imported asset and original media.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalMediaImport {
    asset: Asset,
    media: RepresentationImport,
}

impl OriginalMediaImport {
    /// Creates an import aggregate whose ownership relationships are consistent.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] if ownership is inconsistent, a
    /// referenced resource is absent or duplicated, an extra resource is
    /// supplied, any resource lacks a locator, or a sequence naming is
    /// misplaced as [`RepresentationImport::new`] describes.
    pub fn new(
        asset: Asset,
        representation: Representation,
        resources: Vec<Resource>,
        locators: Vec<Locator>,
    ) -> Result<Self> {
        if representation.asset_id() != asset.id() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "import representation does not belong to its asset",
            ));
        }
        if representation.kind() != RepresentationKind::Original {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "initial import representation must be original media",
            ));
        }
        let media = RepresentationImport::new(representation, resources, locators)?;
        Ok(Self { asset, media })
    }

    /// Returns the logical asset.
    #[must_use]
    pub const fn asset(&self) -> &Asset {
        &self.asset
    }

    /// Returns the original representation.
    #[must_use]
    pub const fn representation(&self) -> &Representation {
        self.media.representation()
    }

    /// Returns the storage resources realizing the representation.
    #[must_use]
    pub fn resources(&self) -> &[Resource] {
        self.media.resources()
    }

    /// Returns the known access routes for the imported resources.
    #[must_use]
    pub fn locators(&self) -> &[Locator] {
        self.media.locators()
    }

    /// Splits the aggregate into persistable domain values.
    #[must_use]
    pub fn into_parts(self) -> (Asset, Representation, Vec<Resource>, Vec<Locator>) {
        let (representation, resources, locators) = self.media.into_parts();
        (self.asset, representation, resources, locators)
    }
}

/// An ordered filesystem or URI boundary searched by the resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaRoot {
    id: MediaRootId,
    name: String,
    label: Option<String>,
    legacy_uri: Option<String>,
    priority: i32,
    enabled: bool,
}

impl MediaRoot {
    /// Validates a production-portable logical root name.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when `name` is empty, oversized,
    /// path-shaped, contains control characters, or has surrounding whitespace.
    pub fn validate_name(name: &str) -> Result<()> {
        if name.is_empty()
            || name.len() > 128
            || name.trim() != name
            || name
                .chars()
                .any(|character| character.is_control() || matches!(character, '/' | '\\'))
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "media-root name must be 1-128 UTF-8 bytes without surrounding whitespace, control characters, or path separators",
            ));
        }
        Ok(())
    }

    /// Creates a configured logical media root.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] when `name` is not a bounded,
    /// portable root name or when `legacy_uri` is invalid or relative.
    pub fn new(
        id: MediaRootId,
        name: impl Into<String>,
        label: Option<String>,
        legacy_uri: Option<String>,
        priority: i32,
        enabled: bool,
    ) -> Result<Self> {
        let name = name.into();
        Self::validate_name(&name)?;
        let legacy_uri = legacy_uri
            .map(|uri| normalize_uri(uri, "legacy media-root"))
            .transpose()?;
        Ok(Self {
            id,
            name,
            label,
            legacy_uri,
            priority,
            enabled,
        })
    }

    /// Returns the root's stable identity.
    #[must_use]
    pub const fn id(&self) -> MediaRootId {
        self.id
    }

    /// Returns the production-portable logical root name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the optional user-facing label.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Returns the absolute URI retained while migrating a pre-version-6 root.
    ///
    /// New roots do not carry this machine-local fallback.
    #[must_use]
    pub fn legacy_uri(&self) -> Option<&str> {
        self.legacy_uri.as_deref()
    }

    /// Returns the resolver priority; lower values are considered first.
    #[must_use]
    pub const fn priority(&self) -> i32 {
        self.priority
    }

    /// Returns whether this root participates in resolution.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocatorAvailability, LocatorId, ResourceId};

    #[test]
    fn rejects_invalid_root_names() {
        let root = MediaRoot::new(MediaRootId::new(), "path/name", None, None, 0, true);

        assert_eq!(
            root.expect_err("path-shaped name must fail").kind(),
            ErrorKind::InvalidArgument
        );
    }

    #[test]
    fn import_aggregate_enforces_ownership() {
        let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(0), None, None);
        let resource_id = ResourceId::new();
        let representation = Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Original,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        );
        let resource = Resource::new(resource_id, Vec::new(), None);
        let locator = Locator::new(
            LocatorId::new(),
            resource_id,
            "file:///media.mov",
            None,
            LocatorAvailability::Online,
        )
        .expect("valid locator");

        assert_eq!(
            OriginalMediaImport::new(asset, representation, vec![resource], vec![locator])
                .expect_err("mismatched ownership must fail")
                .kind(),
            ErrorKind::InvalidArgument
        );
    }

    #[test]
    fn representation_import_accepts_non_original_media() {
        let asset_id = AssetId::new();
        let resource_id = ResourceId::new();
        let representation = Representation::new(
            RepresentationId::new(),
            asset_id,
            RepresentationKind::Proxy,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        );
        let resource = Resource::new(resource_id, Vec::new(), None);
        let locator = Locator::new(
            LocatorId::new(),
            resource_id,
            "file:///proxy.mov",
            None,
            LocatorAvailability::Online,
        )
        .expect("valid locator");

        let imported =
            RepresentationImport::new(representation.clone(), vec![resource], vec![locator])
                .expect("valid representation import");

        assert_eq!(imported.representation(), &representation);
        assert_eq!(imported.representation().asset_id(), asset_id);
        assert_eq!(imported.representation().kind(), RepresentationKind::Proxy);
    }

    #[test]
    fn representation_import_requires_a_locator_for_every_resource() {
        let resource_id = ResourceId::new();
        let representation = Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Derived,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        );

        assert_eq!(
            RepresentationImport::new(
                representation,
                vec![Resource::new(resource_id, Vec::new(), None)],
                Vec::new(),
            )
            .expect_err("missing locator must fail")
            .kind(),
            ErrorKind::InvalidArgument
        );
    }

    #[test]
    fn exactly_sequence_locators_carry_a_naming() {
        let resource_id = ResourceId::new();
        let descriptor = crate::ImageSequenceDescriptor::new(
            resource_id,
            crate::FrameRange::new(1, 3, 1).expect("valid range"),
            crate::RationalRate::new(24, 1).expect("valid rate"),
            Vec::new(),
        )
        .expect("valid descriptor");
        let sequence = Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Derived,
            ContentStructure::image_sequence(descriptor),
            Vec::new(),
        );
        let file = Representation::new(
            RepresentationId::new(),
            AssetId::new(),
            RepresentationKind::Derived,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        );
        let resource = Resource::new(resource_id, Vec::new(), None);
        let unnamed = Locator::new(
            LocatorId::new(),
            resource_id,
            "file:///plates/",
            None,
            LocatorAvailability::Online,
        )
        .expect("valid locator");
        let named = unnamed.clone().with_sequence_naming(
            crate::SequenceNaming::new("shot_", ".png", 4).expect("valid naming"),
        );

        let import = |representation: &Representation, locator: &Locator| {
            RepresentationImport::new(
                representation.clone(),
                vec![resource.clone()],
                vec![locator.clone()],
            )
        };
        assert!(import(&sequence, &named).is_ok());
        assert!(import(&sequence, &unnamed).is_err());
        assert!(import(&file, &named).is_err());
        assert!(import(&file, &unnamed).is_ok());
    }
}
