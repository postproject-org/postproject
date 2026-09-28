//! Preparation of filesystem-backed domain values before persistence.

use std::{
    fs,
    path::{Path, PathBuf},
};

use postproject_core::{
    Asset, AssetId, ContentStructure, FrameRange, ImageSequenceDescriptor, ImageSequencePattern,
    Locator, LocatorAvailability, LocatorId, MediaRoot, MediaRootId, OriginalMediaImport,
    RationalRate, Representation, RepresentationId, RepresentationImport, RepresentationKind,
    Resource, ResourceId, ResourceMember, ResourceRole, Result, Timestamp,
};

use crate::{
    canonical_file_uri, fingerprint_file, fingerprint_image_sequence, fingerprint_representation,
};

/// Explicit filesystem description of one compact image sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageSequenceSource {
    directory: PathBuf,
    pattern: ImageSequencePattern,
    frames: FrameRange,
    rate: RationalRate,
    known_missing_frames: Vec<i64>,
}

impl ImageSequenceSource {
    /// Creates a sequence source without scanning its directory.
    #[must_use]
    pub fn new(
        directory: impl Into<PathBuf>,
        pattern: ImageSequencePattern,
        frames: FrameRange,
        rate: RationalRate,
        known_missing_frames: Vec<i64>,
    ) -> Self {
        Self {
            directory: directory.into(),
            pattern,
            frames,
            rate,
            known_missing_frames,
        }
    }
}

/// One explicitly supplied file in an ordered or package representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileResourceSource {
    path: PathBuf,
    role: ResourceRole,
    required: bool,
}

impl FileResourceSource {
    /// Creates a file source with its membership role and requiredness.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, role: ResourceRole, required: bool) -> Self {
        Self {
            path: path.into(),
            role,
            required,
        }
    }
}

/// Content structure of a representation at its present location.
///
/// The same source serves both operations that create a representation:
/// [`prepare_original_media`] creates an asset whose original has the source's
/// structure, and [`prepare_representation`] adds a representation of a chosen
/// kind to an existing asset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaSource {
    /// One regular file forming a single-resource representation.
    File(PathBuf),
    /// One compact image sequence in a directory.
    ImageSequence(ImageSequenceSource),
    /// Ordered, fully required files, such as spanned camera recordings.
    OrderedParts(Vec<FileResourceSource>),
    /// Role-bearing required and optional files, such as a card package.
    Package(Vec<FileResourceSource>),
}

impl From<PathBuf> for MediaSource {
    fn from(path: PathBuf) -> Self {
        Self::File(path)
    }
}

impl From<&PathBuf> for MediaSource {
    fn from(path: &PathBuf) -> Self {
        Self::File(path.clone())
    }
}

impl From<&Path> for MediaSource {
    fn from(path: &Path) -> Self {
        Self::File(path.to_path_buf())
    }
}

impl From<ImageSequenceSource> for MediaSource {
    fn from(source: ImageSequenceSource) -> Self {
        Self::ImageSequence(source)
    }
}

#[derive(Clone, Copy)]
enum CompoundShape {
    OrderedParts,
    Package,
}

/// Inspects a media source and prepares a validated original-media import.
///
/// The new asset's original representation has the source's content
/// structure. This function does not mutate production state. Persist the
/// returned aggregate inside an explicit production transaction.
///
/// # Errors
///
/// Returns errors from path canonicalization, directory inspection,
/// fingerprint calculation, time capture, or domain validation.
pub fn prepare_original_media(
    source: impl Into<MediaSource>,
    display_name: Option<String>,
    import_source: Option<String>,
) -> Result<OriginalMediaImport> {
    let now = Timestamp::now()?;
    let asset = Asset::new(AssetId::new(), now, display_name, import_source);
    let prepared = prepare_representation_at(
        asset.id(),
        RepresentationKind::Original,
        &source.into(),
        now,
    )?;
    let (representation, resources, locators) = prepared.into_parts();
    OriginalMediaImport::new(asset, representation, resources, locators)
}

/// Inspects a media source and prepares a representation for an existing asset.
///
/// The caller chooses the representation kind. An image-sequence source is
/// explicit: this validates its directory, fingerprints a deterministic sample
/// of declared members, and records the caller-supplied frame domain and
/// exceptions. Ordered parts keep their supplied order, and every part must be
/// required. This function does not mutate production state; persist the
/// returned aggregate with a production transaction.
///
/// # Errors
///
/// Returns errors from path canonicalization, directory inspection,
/// fingerprint calculation, time capture, or domain validation.
pub fn prepare_representation(
    asset_id: AssetId,
    kind: RepresentationKind,
    source: impl Into<MediaSource>,
) -> Result<RepresentationImport> {
    prepare_representation_at(asset_id, kind, &source.into(), Timestamp::now()?)
}

fn prepare_representation_at(
    asset_id: AssetId,
    kind: RepresentationKind,
    source: &MediaSource,
    now: Timestamp,
) -> Result<RepresentationImport> {
    match source {
        MediaSource::File(path) => prepare_single_file_representation(asset_id, kind, path, now),
        MediaSource::ImageSequence(sequence) => {
            prepare_image_sequence_representation(asset_id, kind, sequence, now)
        }
        MediaSource::OrderedParts(parts) => prepare_compound_file_representation(
            asset_id,
            kind,
            parts,
            CompoundShape::OrderedParts,
            now,
        ),
        MediaSource::Package(members) => prepare_compound_file_representation(
            asset_id,
            kind,
            members,
            CompoundShape::Package,
            now,
        ),
    }
}

fn prepare_single_file_representation(
    asset_id: AssetId,
    kind: RepresentationKind,
    path: &Path,
    now: Timestamp,
) -> Result<RepresentationImport> {
    let report = fingerprint_file(path)?;
    let uri = canonical_file_uri(path)?;
    let (fingerprint, facts, _) = report.into_parts();
    let resource_id = ResourceId::new();
    let structure = ContentStructure::single_resource(resource_id);
    let resource = Resource::new(resource_id, vec![fingerprint], Some(facts));
    let representation_fingerprint =
        fingerprint_representation(&structure, std::slice::from_ref(&resource))?;
    let representation = Representation::new(
        RepresentationId::new(),
        asset_id,
        kind,
        structure,
        vec![representation_fingerprint],
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource_id,
        uri,
        Some(now),
        LocatorAvailability::Online,
    )?;
    RepresentationImport::new(representation, vec![resource], vec![locator])
}

fn prepare_image_sequence_representation(
    asset_id: AssetId,
    kind: RepresentationKind,
    source: &ImageSequenceSource,
    now: Timestamp,
) -> Result<RepresentationImport> {
    let metadata = fs::metadata(&source.directory).map_err(|error| {
        postproject_core::Error::new(
            postproject_core::ErrorKind::Io,
            format!(
                "cannot read image-sequence directory {}: {error}",
                source.directory.display()
            ),
        )
    })?;
    if !metadata.is_dir() {
        return Err(postproject_core::Error::new(
            postproject_core::ErrorKind::InvalidArgument,
            format!(
                "image-sequence locator is not a directory: {}",
                source.directory.display()
            ),
        ));
    }
    let resource_id = ResourceId::new();
    let descriptor = ImageSequenceDescriptor::new(
        resource_id,
        source.pattern.clone(),
        source.frames,
        source.rate,
        source.known_missing_frames.clone(),
    )?;
    let sequence_fingerprint = fingerprint_image_sequence(&source.directory, &descriptor)?
        .into_parts()
        .0;
    let structure = ContentStructure::image_sequence(descriptor);
    let resource = Resource::new(resource_id, vec![sequence_fingerprint], None);
    let representation_fingerprint =
        fingerprint_representation(&structure, std::slice::from_ref(&resource))?;
    let representation = Representation::new(
        RepresentationId::new(),
        asset_id,
        kind,
        structure,
        vec![representation_fingerprint],
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource_id,
        canonical_file_uri(&source.directory)?,
        Some(now),
        LocatorAvailability::Online,
    )?;
    RepresentationImport::new(representation, vec![resource], vec![locator])
}

fn prepare_compound_file_representation(
    asset_id: AssetId,
    kind: RepresentationKind,
    sources: &[FileResourceSource],
    shape: CompoundShape,
    now: Timestamp,
) -> Result<RepresentationImport> {
    let resource_ids = sources
        .iter()
        .map(|_| ResourceId::new())
        .collect::<Vec<_>>();
    let members = sources
        .iter()
        .zip(&resource_ids)
        .map(|(source, resource_id)| {
            ResourceMember::new(*resource_id, source.role.clone(), source.required)
        })
        .collect();
    let structure = match shape {
        CompoundShape::OrderedParts => ContentStructure::ordered_parts(members)?,
        CompoundShape::Package => ContentStructure::package(members)?,
    };
    let mut resources = Vec::with_capacity(sources.len());
    let mut locators = Vec::with_capacity(sources.len());
    for (source, resource_id) in sources.iter().zip(resource_ids) {
        let report = fingerprint_file(&source.path)?;
        let uri = canonical_file_uri(&source.path)?;
        let (fingerprint, facts, _) = report.into_parts();
        resources.push(Resource::new(resource_id, vec![fingerprint], Some(facts)));
        locators.push(Locator::new(
            LocatorId::new(),
            resource_id,
            uri,
            Some(now),
            LocatorAvailability::Online,
        )?);
    }
    let representation_fingerprint = fingerprint_representation(&structure, &resources)?;
    let representation = Representation::new(
        RepresentationId::new(),
        asset_id,
        kind,
        structure,
        vec![representation_fingerprint],
    );
    RepresentationImport::new(representation, resources, locators)
}

/// Validates a directory and prepares a canonical resolver media root.
///
/// If `path` itself is a symbolic link, canonicalization stores the target
/// directory URI. Descendant symlinks remain excluded by resolver policy.
///
/// # Errors
///
/// Returns an I/O error when metadata cannot be read, an invalid-argument error
/// when `path` is not a directory, or errors from path/URI domain validation.
pub fn prepare_media_root(
    path: impl AsRef<Path>,
    label: Option<String>,
    priority: i32,
) -> Result<MediaRoot> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).map_err(|error| {
        postproject_core::Error::new(
            postproject_core::ErrorKind::Io,
            format!(
                "cannot read media-root metadata {}: {error}",
                path.display()
            ),
        )
    })?;
    if !metadata.is_dir() {
        return Err(postproject_core::Error::new(
            postproject_core::ErrorKind::InvalidArgument,
            format!("media root is not a directory: {}", path.display()),
        ));
    }
    let id = MediaRootId::new();
    let name = label.clone().unwrap_or_else(|| format!("legacy-{id}"));
    MediaRoot::new(
        id,
        name,
        label,
        Some(canonical_file_uri(path)?),
        priority,
        true,
    )
}

/// Prepares a confirmed online locator for transactional persistence.
///
/// The URI is normalized by the domain constructor. This function does not
/// require the URI to use the `file` scheme because future storage transports may
/// confirm other locator types.
///
/// # Errors
///
/// Returns errors from time capture or URI validation.
pub fn prepare_confirmed_locator(
    resource_id: ResourceId,
    uri: impl Into<String>,
) -> Result<Locator> {
    let now = Timestamp::now()?;
    Locator::new(
        LocatorId::new(),
        resource_id,
        uri,
        Some(now),
        LocatorAvailability::Online,
    )
}

/// Prepares a confirmed online locator associated with a logical media root.
///
/// # Errors
///
/// Returns errors from time capture, URI validation, or root-name validation.
pub fn prepare_confirmed_locator_under_root(
    resource_id: ResourceId,
    uri: impl Into<String>,
    root_name: impl Into<String>,
) -> Result<Locator> {
    prepare_confirmed_locator(resource_id, uri)?.with_media_root(root_name)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn prepares_consistent_original_import() {
        let mut file = NamedTempFile::new().expect("create file");
        file.write_all(b"original media").expect("write file");
        let prepared = prepare_original_media(
            file.path(),
            Some("Camera A".to_owned()),
            Some("unit-test".to_owned()),
        )
        .expect("prepare import");

        assert_eq!(prepared.representation().asset_id(), prepared.asset().id());
        assert_eq!(prepared.representation().fingerprints().len(), 1);
        assert_eq!(prepared.resources().len(), 1);
        assert_eq!(prepared.resources()[0].fingerprints().len(), 1);
        assert_eq!(
            prepared.locators()[0].resource_id(),
            prepared.resources()[0].id()
        );
        assert!(prepared.locators()[0].uri().starts_with("file:"));
    }

    #[test]
    fn prepares_non_original_single_file_representation() {
        let mut file = NamedTempFile::new().expect("create file");
        file.write_all(b"proxy media").expect("write file");
        let asset_id = AssetId::new();

        let prepared = prepare_representation(asset_id, RepresentationKind::Proxy, file.path())
            .expect("prepare proxy");

        assert_eq!(prepared.representation().asset_id(), asset_id);
        assert_eq!(prepared.representation().kind(), RepresentationKind::Proxy);
        assert_eq!(prepared.representation().fingerprints().len(), 1);
        assert_eq!(prepared.resources()[0].fingerprints().len(), 1);
        assert_eq!(prepared.locators().len(), 1);
    }

    #[test]
    fn prepares_compact_image_sequence_with_sampled_fingerprints() {
        let directory = tempfile::tempdir().expect("create directory");
        for frame in [1_001, 1_002, 1_005] {
            fs::write(
                directory.path().join(format!("shot.{frame:04}.exr")),
                format!("frame {frame}"),
            )
            .expect("write sampled frame");
        }
        let source = ImageSequenceSource::new(
            directory.path(),
            ImageSequencePattern::new("shot.", ".exr", 4).expect("valid pattern"),
            FrameRange::new(1_001, 1_005, 1).expect("valid range"),
            RationalRate::new(24_000, 1_001).expect("valid rate"),
            vec![1_003],
        );

        let prepared = prepare_representation(AssetId::new(), RepresentationKind::Derived, source)
            .expect("prepare sequence");

        let descriptor = prepared
            .representation()
            .content_structure()
            .image_sequence_descriptor()
            .expect("sequence descriptor");
        assert_eq!(descriptor.known_missing_frames(), &[1_003]);
        assert_eq!(prepared.resources().len(), 1);
        assert_eq!(prepared.resources()[0].fingerprints().len(), 1);
        assert_eq!(prepared.representation().fingerprints().len(), 1);
        assert!(prepared.locators()[0].uri().starts_with("file:"));
    }

    #[test]
    fn prepares_image_sequence_as_original_media() {
        let directory = tempfile::tempdir().expect("create directory");
        for frame in 1..=3 {
            fs::write(
                directory.path().join(format!("strip.{frame:04}.png")),
                format!("frame {frame}"),
            )
            .expect("write frame");
        }
        let frames = FrameRange::new(1, 3, 1).expect("valid range");
        let source = ImageSequenceSource::new(
            directory.path(),
            ImageSequencePattern::new("strip.", ".png", 4).expect("valid pattern"),
            frames,
            RationalRate::new(24, 1).expect("valid rate"),
            Vec::new(),
        );

        let prepared = prepare_original_media(source, Some("Strip".to_owned()), None)
            .expect("prepare sequence import");

        assert_eq!(
            prepared.representation().kind(),
            RepresentationKind::Original
        );
        assert_eq!(prepared.representation().asset_id(), prepared.asset().id());
        let descriptor = prepared
            .representation()
            .content_structure()
            .image_sequence_descriptor()
            .expect("sequence descriptor");
        assert_eq!(descriptor.frames(), frames);
        assert_eq!(prepared.resources().len(), 1);
        assert_eq!(prepared.locators().len(), 1);
    }

    #[test]
    fn prepares_ordered_parts_in_source_order() {
        let directory = tempfile::tempdir().expect("create directory");
        let paths = [
            directory.path().join("part-1.mxf"),
            directory.path().join("part-2.mxf"),
        ];
        for (index, path) in paths.iter().enumerate() {
            fs::write(path, format!("part {index}")).expect("write part");
        }
        let role = ResourceRole::new("example.camera:essence-part").expect("valid role");
        let sources = paths
            .iter()
            .map(|path| FileResourceSource::new(path, role.clone(), true))
            .collect::<Vec<_>>();

        let prepared = prepare_representation(
            AssetId::new(),
            RepresentationKind::Original,
            MediaSource::OrderedParts(sources),
        )
        .expect("prepare ordered parts");

        assert_eq!(
            prepared.representation().content_structure().resource_ids(),
            prepared
                .resources()
                .iter()
                .map(Resource::id)
                .collect::<Vec<_>>()
        );
        assert_eq!(prepared.representation().fingerprints().len(), 1);
        assert_eq!(prepared.locators().len(), 2);
    }

    #[test]
    fn prepares_package_with_optional_members() {
        let directory = tempfile::tempdir().expect("create directory");
        let essence = directory.path().join("essence.mxf");
        let sidecar = directory.path().join("metadata.xml");
        fs::write(&essence, b"essence").expect("write essence");
        fs::write(&sidecar, b"metadata").expect("write sidecar");
        let sources = vec![
            FileResourceSource::new(
                essence,
                ResourceRole::new("org.postproject:essence").expect("valid role"),
                true,
            ),
            FileResourceSource::new(
                sidecar,
                ResourceRole::new("org.postproject:sidecar").expect("valid role"),
                false,
            ),
        ];

        let prepared = prepare_representation(
            AssetId::new(),
            RepresentationKind::Optimized,
            MediaSource::Package(sources),
        )
        .expect("prepare package");

        let members = prepared
            .representation()
            .content_structure()
            .members()
            .expect("package members");
        assert!(members[0].is_required());
        assert!(!members[1].is_required());
        assert_eq!(prepared.representation().fingerprints().len(), 1);
    }

    #[test]
    fn root_must_be_a_directory() {
        let file = NamedTempFile::new().expect("create file");
        assert!(prepare_media_root(file.path(), None, 0).is_err());
    }
}
