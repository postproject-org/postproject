//! Deterministic, bounded filesystem media resolution.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use postproject_core::{
    CancellationToken, Confidence, ContentStructure, Error, ErrorKind, EvidenceKind, FileFacts,
    ImageSequenceDescriptor, Locator, MAX_SEQUENCE_EXCEPTIONS, MediaRoot, ResolutionCandidate,
    ResolutionEvidence, Resource, ResourceFingerprint, ResourceResolution, ResourceResolutionState,
    Result,
};
use walkdir::WalkDir;

use crate::{
    FULL_FINGERPRINT_ALGORITHM, InspectionOutcome, MediaInspector, SEQUENCE_FINGERPRINT_ALGORITHM,
    TechnicalMetadata, canonical_file_uri, fingerprint::is_file_fingerprint_domain,
    fingerprint_file, fingerprint_image_sequence, local_file_path,
};

/// One machine's directory mapping for a production-portable root name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaRootMapping {
    name: String,
    directory: PathBuf,
}

impl MediaRootMapping {
    /// Creates a mapping to an existing local directory.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the name is invalid, the path cannot be
    /// inspected, or the path is not a directory.
    pub fn new(name: impl Into<String>, directory: impl AsRef<Path>) -> Result<Self> {
        let name = name.into();
        MediaRoot::validate_name(&name)?;
        let directory = directory.as_ref();
        let metadata = fs::metadata(directory).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read mapped root {}: {error}", directory.display()),
            )
        })?;
        if !metadata.is_dir() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("mapped root is not a directory: {}", directory.display()),
            ));
        }
        let directory = directory.canonicalize().map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(
                    "cannot canonicalize mapped root {}: {error}",
                    directory.display()
                ),
            )
        })?;
        Ok(Self { name, directory })
    }

    /// Returns the logical root name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns this machine's canonical directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

struct SequenceCandidate {
    candidate: ResolutionCandidate,
    missing_frames: Vec<i64>,
}

/// Where discovery searches when a resource is not at a known locator.
///
/// Logical media roots are production knowledge and are located through this
/// machine's mappings (ADR 0017). Search directories are unnamed,
/// machine-local places, such as a project folder or a former location, that
/// are never recorded in the production.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchScope {
    media_roots: Vec<MediaRoot>,
    root_mappings: Vec<MediaRootMapping>,
    search_directories: Vec<PathBuf>,
}

impl SearchScope {
    /// Creates a scope over the production's roots and this machine's mappings.
    #[must_use]
    pub fn new(media_roots: Vec<MediaRoot>, root_mappings: Vec<MediaRootMapping>) -> Self {
        Self {
            media_roots,
            root_mappings,
            search_directories: Vec::new(),
        }
    }

    /// Adds an unnamed directory searched after the mapped roots.
    ///
    /// A directory that is unavailable when searched is reported as
    /// discovery evidence rather than failing resolution.
    #[must_use]
    pub fn with_search_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.search_directories.push(directory.into());
        self
    }

    /// Returns the production's media roots.
    #[must_use]
    pub fn media_roots(&self) -> &[MediaRoot] {
        &self.media_roots
    }

    /// Returns this machine's root mappings.
    #[must_use]
    pub fn root_mappings(&self) -> &[MediaRootMapping] {
        &self.root_mappings
    }

    /// Returns the unnamed search directories in search order.
    #[must_use]
    pub fn search_directories(&self) -> &[PathBuf] {
        &self.search_directories
    }
}

/// One resource to resolve, with the knowledge resolution needs about it.
#[derive(Clone, Copy)]
pub struct ResolutionItem<'a> {
    resource: &'a Resource,
    structure: &'a ContentStructure,
    known_locators: &'a [Locator],
    technical_evidence: Option<(&'a TechnicalMetadata, &'a dyn MediaInspector)>,
}

impl<'a> ResolutionItem<'a> {
    /// Describes a resource of `structure` and its recorded locators.
    #[must_use]
    pub const fn new(
        resource: &'a Resource,
        structure: &'a ContentStructure,
        known_locators: &'a [Locator],
    ) -> Self {
        Self {
            resource,
            structure,
            known_locators,
            technical_evidence: None,
        }
    }

    /// Scores discovered candidates against an inspection persisted at import.
    ///
    /// Technical matches are partial identity evidence only. They can order
    /// candidates but never turn an ambiguous result into an automatic choice.
    /// Missing or failed inspection degrades to the other available evidence.
    #[must_use]
    pub fn with_technical_evidence(
        mut self,
        expected: &'a TechnicalMetadata,
        inspector: &'a dyn MediaInspector,
    ) -> Self {
        self.technical_evidence = Some((expected, inspector));
        self
    }
}

/// Resource limits and cancellation applied to one resolver operation.
#[derive(Clone, Debug)]
pub struct ResolverOptions {
    /// Maximum directory depth, where a searched directory itself has depth zero.
    pub max_depth: usize,
    /// Maximum number of entries visited below each searched directory. A
    /// directory with more entries is searched partially and reported with
    /// [`EvidenceKind::SearchTruncated`].
    pub max_entries_per_directory: usize,
    /// Cost tier for resources found at a known locator.
    pub verification: VerificationMode,
    /// Token polled while scanning and hashing.
    pub cancellation: Option<CancellationToken>,
}

/// Cost tier requested for one resolution call.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum VerificationMode {
    /// Check that the locator and declared members are present.
    #[default]
    Presence,
    /// Recompute stored content fingerprints for present resources.
    Content,
}

impl Default for ResolverOptions {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_entries_per_directory: 100_000,
            verification: VerificationMode::Presence,
            cancellation: None,
        }
    }
}

/// Resolves unavailable representations under configured filesystem roots.
#[derive(Clone, Debug, Default)]
pub struct MediaResolver {
    options: ResolverOptions,
}

impl MediaResolver {
    /// Creates a resolver after validating its resource limits.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] if either limit is zero.
    pub fn new(options: ResolverOptions) -> Result<Self> {
        if options.max_depth == 0 || options.max_entries_per_directory == 0 {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "resolver depth and entry limits must be greater than zero",
            ));
        }
        Ok(Self { options })
    }

    /// Resolves resources without mutating production state.
    ///
    /// Known locators are checked first. Every directory in `scope` is then
    /// walked at most once for the whole call, and the resulting index is
    /// shared by every resource that needs discovery, so resolving many
    /// offline resources together costs one scan. Traversal does not follow
    /// symlinks, is ordered by filename, is bounded per directory, filters by
    /// stored size before hashing, and returns all equally credible matches.
    /// Results are in item order.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] if a known locator belongs to a
    /// different resource or a resource is not part of its structure,
    /// [`ErrorKind::Cancelled`] when the options' token is cancelled, and
    /// otherwise only when a result value cannot be constructed. Filesystem
    /// discovery failures are represented as evidence in the results.
    pub fn resolve(
        &self,
        items: &[ResolutionItem<'_>],
        scope: &SearchScope,
    ) -> Result<Vec<ResourceResolution>> {
        let mut index = LazyIndex::default();
        items
            .iter()
            .map(|item| {
                self.check_cancelled()?;
                self.resolve_item(item, scope, &mut index)
            })
            .collect()
    }

    /// Resolves one resource under roots and mappings at the presence tier.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Self::resolve`].
    pub fn resolve_resource(
        &self,
        resource: &Resource,
        structure: &ContentStructure,
        known_locators: &[Locator],
        media_roots: &[MediaRoot],
        root_mappings: &[MediaRootMapping],
    ) -> Result<ResourceResolution> {
        let scope = SearchScope::new(media_roots.to_vec(), root_mappings.to_vec());
        self.resolve(
            &[ResolutionItem::new(resource, structure, known_locators)],
            &scope,
        )?
        .pop()
        .ok_or_else(|| Error::new(ErrorKind::Internal, "resolution returned no result"))
    }

    fn check_cancelled(&self) -> Result<()> {
        self.options
            .cancellation
            .as_ref()
            .map_or(Ok(()), CancellationToken::check)
    }

    fn resolve_item(
        &self,
        item: &ResolutionItem<'_>,
        scope: &SearchScope,
        index: &mut LazyIndex,
    ) -> Result<ResourceResolution> {
        validate_resolution_inputs(item.resource, item.structure, item.known_locators)?;
        if let Some(resolution) = self.resolve_known_resource(item, scope, index)? {
            return Ok(resolution);
        }
        let index = index.get(self, scope)?;
        self.resolve_discovered_file(item, index)
    }

    fn resolve_known_resource(
        &self,
        item: &ResolutionItem<'_>,
        scope: &SearchScope,
        index: &mut LazyIndex,
    ) -> Result<Option<ResourceResolution>> {
        let resource = item.resource;
        if let Some(sequence) = item
            .structure
            .image_sequence_descriptor()
            .filter(|sequence| sequence.resource_id() == resource.id())
        {
            let (candidate, missing_frames) =
                match online_sequence_candidate(item.known_locators, sequence) {
                    Ok(Some(result)) => result,
                    Ok(None) => {
                        let index = index.get(self, scope)?;
                        return self
                            .resolve_moved_sequence(resource, sequence, index)
                            .map(Some);
                    }
                    Err(detail) => return error_resolution(resource.id(), detail).map(Some),
                };
            let resolution = ResourceResolution::new(
                resource.id(),
                ResourceResolutionState::OnlineAtKnownLocator,
                vec![candidate],
                Vec::new(),
            )?
            .with_missing_frames(missing_frames)?;
            if self.options.verification == VerificationMode::Content {
                self.check_cancelled()?;
                return verify_known_sequence(resource, sequence, &resolution).map(Some);
            }
            return Ok(Some(resolution));
        }
        if let Some(candidate) = online_known_candidate(item.known_locators)? {
            if self.options.verification == VerificationMode::Content {
                self.check_cancelled()?;
                return verify_known_file(resource, &candidate).map(Some);
            }
            return ResourceResolution::new(
                resource.id(),
                ResourceResolutionState::OnlineAtKnownLocator,
                vec![candidate],
                Vec::new(),
            )
            .map(Some);
        }
        Ok(None)
    }

    fn resolve_discovered_file(
        &self,
        item: &ResolutionItem<'_>,
        index: &IndexState,
    ) -> Result<ResourceResolution> {
        let resource = item.resource;
        let index = match index {
            IndexState::Ready(index) => index,
            IndexState::Failed(detail) => return error_resolution(resource.id(), detail.clone()),
        };
        let original_path = item
            .known_locators
            .iter()
            .find_map(|locator| local_file_path(locator.uri()).ok());
        let original_name = original_path.as_deref().and_then(Path::file_name);
        let fingerprints = FileFingerprints::classify(resource.fingerprints());
        let mut discovered = index.discover(
            resource.file_facts(),
            !fingerprints.comparable.is_empty(),
            original_name,
            original_path.as_deref(),
        );

        let mut candidates = Vec::new();
        for (uri, (root, cheap_evidence)) in discovered.candidates {
            self.check_cancelled()?;
            let path = local_file_path(&uri).map_err(|error| {
                Error::new(
                    ErrorKind::Internal,
                    format!("discovered URI cannot be converted back to a path: {error}"),
                )
            })?;
            let technical_match = fingerprints.comparable.is_empty()
                && item
                    .technical_evidence
                    .is_some_and(|(expected, inspector)| {
                        matches!(
                            inspector.inspect(&path),
                            Ok(InspectionOutcome::Inspected(actual)) if actual == *expected
                        )
                    });
            match verify_candidate(&path, &uri, cheap_evidence, &fingerprints, technical_match) {
                Ok(Some(candidate)) => candidates.push(match root {
                    Some(root) => candidate.with_media_root(root)?,
                    None => candidate,
                }),
                Ok(None) => {}
                Err(detail) => discovered.diagnostics.push(ResolutionEvidence::new(
                    EvidenceKind::DiscoveryError,
                    Some(format!("{uri}: {detail}")),
                )),
            }
        }

        let state = match candidates.len() {
            0 if !discovered.diagnostics.is_empty() => ResourceResolutionState::Error,
            0 => ResourceResolutionState::Offline,
            1 if candidates[0].confidence() == Confidence::CERTAIN => {
                ResourceResolutionState::ResolvedExact
            }
            1 => ResourceResolutionState::ResolvedProbable,
            _ => ResourceResolutionState::Ambiguous,
        };
        let mut evidence = discovered.diagnostics;
        if state == ResourceResolutionState::Ambiguous {
            evidence.push(ResolutionEvidence::new(
                EvidenceKind::ConflictingCandidate,
                Some(format!(
                    "{} candidates have matching identity evidence",
                    candidates.len()
                )),
            ));
        }
        ResourceResolution::new(resource.id(), state, candidates, evidence)
    }

    fn resolve_moved_sequence(
        &self,
        resource: &Resource,
        descriptor: &ImageSequenceDescriptor,
        index: &IndexState,
    ) -> Result<ResourceResolution> {
        let index = match index {
            IndexState::Ready(index) => index,
            IndexState::Failed(detail) => return error_resolution(resource.id(), detail.clone()),
        };
        let mut diagnostics = index.diagnostics.clone();
        let mut found = BTreeMap::new();
        for (root, directory) in index.sequence_directories(descriptor) {
            self.check_cancelled()?;
            match verify_sequence_directory(
                directory,
                root.as_deref(),
                descriptor,
                resource.fingerprints(),
            ) {
                Ok(Some(candidate)) => {
                    found
                        .entry(candidate.candidate.uri().to_owned())
                        .or_insert(candidate);
                }
                Ok(None) => {}
                Err(detail) => diagnostics.push(ResolutionEvidence::new(
                    EvidenceKind::DiscoveryError,
                    Some(format!("{}: {detail}", directory.display())),
                )),
            }
        }
        let found = found.into_values().collect::<Vec<_>>();
        let state = match found.len() {
            0 if !diagnostics.is_empty() => ResourceResolutionState::Error,
            0 => ResourceResolutionState::Offline,
            1 => ResourceResolutionState::ResolvedProbable,
            _ => ResourceResolutionState::Ambiguous,
        };
        if state == ResourceResolutionState::Ambiguous {
            diagnostics.push(ResolutionEvidence::new(
                EvidenceKind::ConflictingCandidate,
                Some(format!("{} sequence directories match", found.len())),
            ));
        }
        let missing_frames = if found.len() == 1 {
            found[0].missing_frames.clone()
        } else {
            Vec::new()
        };
        ResourceResolution::new(
            resource.id(),
            state,
            found.into_iter().map(|found| found.candidate).collect(),
            diagnostics,
        )?
        .with_missing_frames(missing_frames)
    }
}

/// The directory index of one resolution call, built on first use.
#[derive(Default)]
struct LazyIndex(Option<IndexState>);

impl LazyIndex {
    fn get(&mut self, resolver: &MediaResolver, scope: &SearchScope) -> Result<&IndexState> {
        if self.0.is_none() {
            self.0 = Some(DirectoryIndex::build(resolver, scope)?);
        }
        self.0
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::Internal, "directory index was not built"))
    }
}

enum IndexState {
    Ready(DirectoryIndex),
    /// The scope itself is unusable, for example a root mapped twice.
    Failed(String),
}

/// Every regular file below the searched directories, in search order.
struct DirectoryIndex {
    files: Vec<IndexedFile>,
    diagnostics: Vec<ResolutionEvidence>,
}

struct IndexedFile {
    root: Option<String>,
    path: PathBuf,
    size: u64,
}

struct Discovery {
    candidates: BTreeMap<String, (Option<String>, Vec<ResolutionEvidence>)>,
    diagnostics: Vec<ResolutionEvidence>,
}

impl DirectoryIndex {
    fn build(resolver: &MediaResolver, scope: &SearchScope) -> Result<IndexState> {
        let search = match searchable_directories(scope) {
            Ok(search) => search,
            Err(detail) => return Ok(IndexState::Failed(detail)),
        };
        let mut index = Self {
            files: Vec::new(),
            diagnostics: search.diagnostics,
        };
        for directory in search.directories {
            index.scan(resolver, &directory)?;
        }
        Ok(IndexState::Ready(index))
    }

    fn scan(&mut self, resolver: &MediaResolver, directory: &SearchDirectory) -> Result<()> {
        let options = &resolver.options;
        let mut entries_seen = 0_usize;
        for entry in WalkDir::new(&directory.path)
            .follow_links(false)
            .max_depth(options.max_depth)
            .sort_by_file_name()
        {
            resolver.check_cancelled()?;
            entries_seen = entries_seen.saturating_add(1);
            if entries_seen > options.max_entries_per_directory {
                self.diagnostics.push(ResolutionEvidence::new(
                    EvidenceKind::SearchTruncated,
                    Some(format!(
                        "{}: entry limit {} reached",
                        directory.label, options.max_entries_per_directory
                    )),
                ));
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    self.diagnostics.push(ResolutionEvidence::new(
                        if directory.root.is_some() {
                            EvidenceKind::MediaRootUnavailable
                        } else {
                            EvidenceKind::DiscoveryError
                        },
                        Some(format!("{}: {error}", directory.label)),
                    ));
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let Some(metadata) = ok_or_discovery_error(
                &mut self.diagnostics,
                entry
                    .metadata()
                    .map_err(|error| format!("inspect {}: {error}", entry.path().display())),
            ) else {
                continue;
            };
            self.files.push(IndexedFile {
                root: directory.root.clone(),
                path: entry.into_path(),
                size: metadata.len(),
            });
        }
        Ok(())
    }

    fn discover(
        &self,
        facts: Option<FileFacts>,
        has_comparable_fingerprint: bool,
        original_name: Option<&OsStr>,
        original_path: Option<&Path>,
    ) -> Discovery {
        let mut discovered = BTreeMap::new();
        let mut diagnostics = self.diagnostics.clone();
        for file in &self.files {
            if facts.is_some_and(|facts| file.size != facts.size_bytes()) {
                continue;
            }
            let filename_matches =
                original_name.is_some_and(|name| file.path.file_name() == Some(name));
            if (!has_comparable_fingerprint || facts.is_none()) && !filename_matches {
                continue;
            }
            let Some(uri) = ok_or_discovery_error(
                &mut diagnostics,
                canonical_file_uri(&file.path)
                    .map_err(|error| format!("{}: {error}", file.path.display())),
            ) else {
                continue;
            };
            let mut evidence = Vec::new();
            if let Some(root) = &file.root {
                evidence.push(ResolutionEvidence::new(
                    EvidenceKind::MediaRootRelation,
                    Some(root.clone()),
                ));
            }
            if facts.is_some() {
                evidence.push(ResolutionEvidence::new(EvidenceKind::FileSizeMatch, None));
            }
            if filename_matches {
                evidence.push(ResolutionEvidence::new(EvidenceKind::FileNameMatch, None));
            }
            add_relative_path_evidence(&mut evidence, original_path, &file.path);
            discovered
                .entry(uri)
                .or_insert((file.root.clone(), evidence));
        }
        Discovery {
            candidates: discovered,
            diagnostics,
        }
    }

    /// Returns directories holding the sequence's first expected frame, each
    /// once, with the root it was found under.
    fn sequence_directories(
        &self,
        descriptor: &ImageSequenceDescriptor,
    ) -> Vec<(&Option<String>, &Path)> {
        let frames = descriptor.frames();
        let mut frame = frames.start();
        while descriptor.is_known_missing(frame) && frame != frames.end() {
            frame += i64::from(frames.step());
        }
        let first = descriptor.pattern().filename(frame);
        let mut seen = BTreeSet::new();
        self.files
            .iter()
            .filter(|file| file.path.file_name() == Some(OsStr::new(&first)))
            .filter_map(|file| {
                let parent = file.path.parent()?;
                seen.insert(parent).then_some((&file.root, parent))
            })
            .collect()
    }
}

/// Records a failure to examine one discovered entry without abandoning the scan.
fn ok_or_discovery_error<T>(
    diagnostics: &mut Vec<ResolutionEvidence>,
    result: std::result::Result<T, String>,
) -> Option<T> {
    result
        .map_err(|detail| {
            diagnostics.push(ResolutionEvidence::new(
                EvidenceKind::DiscoveryError,
                Some(detail),
            ));
        })
        .ok()
}

fn validate_resolution_inputs(
    resource: &Resource,
    structure: &ContentStructure,
    known_locators: &[Locator],
) -> Result<()> {
    if !structure.resource_ids().contains(&resource.id()) {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "resource does not belong to the supplied content structure",
        ));
    }
    if known_locators
        .iter()
        .any(|locator| locator.resource_id() != resource.id())
    {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "known locator belongs to a different resource",
        ));
    }
    Ok(())
}

struct SearchDirectory {
    root: Option<String>,
    label: String,
    path: PathBuf,
}

struct SearchableDirectories {
    directories: Vec<SearchDirectory>,
    diagnostics: Vec<ResolutionEvidence>,
}

/// Orders enabled mapped roots by priority, then the unnamed search
/// directories, reporting unmapped and unavailable ones as evidence.
fn searchable_directories(
    scope: &SearchScope,
) -> std::result::Result<SearchableDirectories, String> {
    let mut by_name = BTreeMap::new();
    for mapping in scope.root_mappings() {
        if by_name
            .insert(mapping.name(), mapping.directory())
            .is_some()
        {
            return Err(format!(
                "media root {} has more than one machine mapping",
                mapping.name()
            ));
        }
    }
    let mut ordered = scope
        .media_roots()
        .iter()
        .filter(|root| root.is_enabled())
        .collect::<Vec<_>>();
    ordered.sort_by_key(|root| (root.priority(), root.id()));
    let mut directories = Vec::new();
    let mut diagnostics = Vec::new();
    for root in ordered {
        let legacy = root
            .legacy_uri()
            .map(local_file_path)
            .transpose()
            .map_err(|error| format!("legacy media root {} is invalid: {error}", root.name()))?;
        let Some(path) = by_name.get(root.name()).copied().or(legacy.as_deref()) else {
            diagnostics.push(ResolutionEvidence::new(
                EvidenceKind::MediaRootUnmapped,
                Some(root.name().to_owned()),
            ));
            continue;
        };
        if path.is_dir() {
            directories.push(SearchDirectory {
                root: Some(root.name().to_owned()),
                label: root.name().to_owned(),
                path: path.to_path_buf(),
            });
        } else {
            diagnostics.push(ResolutionEvidence::new(
                EvidenceKind::MediaRootUnavailable,
                Some(format!("{}: {}", root.name(), path.display())),
            ));
        }
    }
    for path in scope.search_directories() {
        if path.is_dir() {
            directories.push(SearchDirectory {
                root: None,
                label: path.display().to_string(),
                path: path.clone(),
            });
        } else {
            diagnostics.push(ResolutionEvidence::new(
                EvidenceKind::DiscoveryError,
                Some(format!(
                    "search directory is unavailable: {}",
                    path.display()
                )),
            ));
        }
    }
    Ok(SearchableDirectories {
        directories,
        diagnostics,
    })
}

fn verify_sequence_directory(
    directory: &Path,
    root_name: Option<&str>,
    descriptor: &ImageSequenceDescriptor,
    fingerprints: &[ResourceFingerprint],
) -> std::result::Result<Option<SequenceCandidate>, String> {
    let missing_frames = sequence_missing_frames(directory, descriptor)?;
    if !missing_frames.is_empty() {
        return Ok(None);
    }
    let expected = fingerprints.iter().find(|fingerprint| {
        fingerprint.algorithm() == SEQUENCE_FINGERPRINT_ALGORITHM
            && fingerprint.version() == crate::SEQUENCE_FINGERPRINT_VERSION
    });
    let mut evidence = root_name
        .map(|root| ResolutionEvidence::new(EvidenceKind::MediaRootRelation, Some(root.to_owned())))
        .into_iter()
        .collect::<Vec<_>>();
    evidence.push(ResolutionEvidence::new(
        EvidenceKind::FileNameMatch,
        Some(format!(
            "{}%0{}d{}",
            descriptor.pattern().prefix(),
            descriptor.pattern().padding(),
            descriptor.pattern().suffix()
        )),
    ));
    let confidence = if let Some(expected) = expected {
        let report =
            fingerprint_image_sequence(directory, descriptor).map_err(|error| error.to_string())?;
        if report.fingerprint() != expected {
            return Ok(None);
        }
        evidence.push(ResolutionEvidence::new(
            EvidenceKind::PartialFingerprintMatch,
            Some(format!(
                "{} version {}",
                expected.algorithm(),
                expected.version()
            )),
        ));
        Confidence::from_basis_points(9_500).map_err(|error| error.to_string())?
    } else {
        if let Some(evidence_item) = not_verified_evidence(fingerprints) {
            evidence.push(evidence_item);
        }
        Confidence::from_basis_points(7_000).map_err(|error| error.to_string())?
    };
    let uri = canonical_file_uri(directory).map_err(|error| error.to_string())?;
    let mut candidate =
        ResolutionCandidate::new(uri, confidence, evidence).map_err(|error| error.to_string())?;
    if let Some(root) = root_name {
        candidate = candidate
            .with_media_root(root)
            .map_err(|error| error.to_string())?;
    }
    Ok(Some(SequenceCandidate {
        candidate,
        missing_frames,
    }))
}

fn verify_known_file(
    resource: &Resource,
    presence_candidate: &ResolutionCandidate,
) -> Result<ResourceResolution> {
    let fingerprints = FileFingerprints::classify(resource.fingerprints());
    if fingerprints.comparable.is_empty() {
        return unverified_known_resolution(resource, presence_candidate);
    }
    let path = local_file_path(presence_candidate.uri())?;
    let evidence = vec![ResolutionEvidence::new(
        EvidenceKind::KnownLocatorAvailable,
        None,
    )];
    match verify_candidate(
        &path,
        presence_candidate.uri(),
        evidence,
        &fingerprints,
        false,
    ) {
        Ok(Some(candidate)) => ResourceResolution::new(
            resource.id(),
            ResourceResolutionState::OnlineAtKnownLocator,
            vec![candidate],
            Vec::new(),
        ),
        Ok(None) => verification_failure(
            resource.id(),
            "content at the known locator differs from its stored fingerprint",
        ),
        Err(detail) => verification_failure(resource.id(), detail),
    }
}

fn verify_known_sequence(
    resource: &Resource,
    descriptor: &ImageSequenceDescriptor,
    presence: &ResourceResolution,
) -> Result<ResourceResolution> {
    let Some(expected) = resource.fingerprints().iter().find(|fingerprint| {
        fingerprint.algorithm() == SEQUENCE_FINGERPRINT_ALGORITHM
            && fingerprint.version() == crate::SEQUENCE_FINGERPRINT_VERSION
    }) else {
        let Some(candidate) = presence.candidates().first() else {
            return verification_failure(
                resource.id(),
                "sequence presence result has no candidate",
            );
        };
        return unverified_known_resolution(resource, candidate)?
            .with_missing_frames(presence.missing_frames().to_vec());
    };
    let Some(candidate) = presence.candidates().first() else {
        return verification_failure(resource.id(), "sequence presence result has no candidate");
    };
    let path = local_file_path(candidate.uri())?;
    let report = match fingerprint_image_sequence(&path, descriptor) {
        Ok(report) => report,
        Err(error) => return verification_failure(resource.id(), error.to_string()),
    };
    if report.fingerprint() != expected {
        return verification_failure(
            resource.id(),
            "sequence content differs from its stored collection fingerprint",
        );
    }
    let verified = ResolutionCandidate::new(
        candidate.uri(),
        Confidence::from_basis_points(9_500)?,
        vec![
            ResolutionEvidence::new(EvidenceKind::KnownLocatorAvailable, None),
            ResolutionEvidence::new(
                EvidenceKind::PartialFingerprintMatch,
                Some(format!(
                    "{} version {}",
                    expected.algorithm(),
                    expected.version()
                )),
            ),
        ],
    )?;
    ResourceResolution::new(
        resource.id(),
        ResourceResolutionState::OnlineAtKnownLocator,
        vec![verified],
        Vec::new(),
    )?
    .with_missing_frames(presence.missing_frames().to_vec())
}

/// Reports a present known locator whose content could not be checked because
/// no stored fingerprint lies in a domain the resolver can compute. This is not
/// a mismatch: nothing contradicts the stored evidence.
fn unverified_known_resolution(
    resource: &Resource,
    presence_candidate: &ResolutionCandidate,
) -> Result<ResourceResolution> {
    let mut evidence = vec![ResolutionEvidence::new(
        EvidenceKind::KnownLocatorAvailable,
        None,
    )];
    evidence.push(
        not_verified_evidence(resource.fingerprints()).unwrap_or_else(|| {
            ResolutionEvidence::new(
                EvidenceKind::FingerprintNotVerified,
                Some("resource has no stored fingerprint".to_owned()),
            )
        }),
    );
    let candidate = ResolutionCandidate::new(
        presence_candidate.uri(),
        presence_candidate.confidence(),
        evidence,
    )?;
    ResourceResolution::new(
        resource.id(),
        ResourceResolutionState::OnlineAtKnownLocator,
        vec![candidate],
        Vec::new(),
    )
}

/// Describes stored fingerprint domains that were retained but not checked.
fn not_verified_evidence(foreign: &[ResourceFingerprint]) -> Option<ResolutionEvidence> {
    if foreign.is_empty() {
        return None;
    }
    let domains = foreign
        .iter()
        .map(|fingerprint| {
            format!(
                "{} version {}",
                fingerprint.algorithm(),
                fingerprint.version()
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    Some(ResolutionEvidence::new(
        EvidenceKind::FingerprintNotVerified,
        Some(domains),
    ))
}

/// A resource's stored file fingerprints split by whether the resolver can
/// compute their algorithm/version domain.
struct FileFingerprints {
    comparable: Vec<ResourceFingerprint>,
    foreign: Vec<ResourceFingerprint>,
}

impl FileFingerprints {
    fn classify(fingerprints: &[ResourceFingerprint]) -> Self {
        let (comparable, foreign) = fingerprints
            .iter()
            .cloned()
            .partition(is_file_fingerprint_domain);
        Self {
            comparable,
            foreign,
        }
    }
}

fn verification_failure(
    resource_id: postproject_core::ResourceId,
    detail: impl Into<String>,
) -> Result<ResourceResolution> {
    ResourceResolution::new(
        resource_id,
        ResourceResolutionState::Error,
        Vec::new(),
        vec![ResolutionEvidence::new(
            EvidenceKind::FingerprintMismatch,
            Some(detail.into()),
        )],
    )
}

fn matching_parent_components(original: &Path, candidate: &Path) -> usize {
    original
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .rev()
        .zip(
            candidate
                .parent()
                .into_iter()
                .flat_map(Path::components)
                .rev(),
        )
        .take_while(|(left, right)| left == right)
        .count()
}

fn add_relative_path_evidence(
    evidence: &mut Vec<ResolutionEvidence>,
    original: Option<&Path>,
    candidate: &Path,
) {
    let Some(components) = original
        .map(|original| matching_parent_components(original, candidate))
        .filter(|components| *components > 0)
    else {
        return;
    };
    evidence.push(ResolutionEvidence::new(
        EvidenceKind::RelativePathSimilarity,
        Some(format!("{components} matching parent path components")),
    ));
}

fn online_known_candidate(known_locators: &[Locator]) -> Result<Option<ResolutionCandidate>> {
    let mut online = Vec::new();
    for locator in known_locators {
        let Ok(path) = local_file_path(locator.uri()) else {
            continue;
        };
        if path.is_file() {
            online.push(ResolutionCandidate::new(
                locator.uri(),
                Confidence::CERTAIN,
                vec![ResolutionEvidence::new(
                    EvidenceKind::KnownLocatorAvailable,
                    None,
                )],
            )?);
        }
    }
    online.sort_by(|left, right| left.uri().cmp(right.uri()));
    Ok(online.into_iter().next())
}

fn online_sequence_candidate(
    known_locators: &[Locator],
    descriptor: &ImageSequenceDescriptor,
) -> std::result::Result<Option<(ResolutionCandidate, Vec<i64>)>, String> {
    let mut online = known_locators
        .iter()
        .filter_map(|locator| {
            let path = local_file_path(locator.uri()).ok()?;
            path.is_dir().then_some((locator.uri(), path))
        })
        .collect::<Vec<_>>();
    online.sort_by_key(|(uri, _)| *uri);
    // A directory holding none of the sequence's frames is where the sequence
    // was, not where it is: the sequence is searched for like any moved media.
    let Some((uri, path)) = online
        .into_iter()
        .find(|(_, path)| directory_holds_a_frame(path, descriptor))
    else {
        return Ok(None);
    };

    let missing_frames = sequence_missing_frames(&path, descriptor)?;

    let candidate = ResolutionCandidate::new(
        uri,
        Confidence::CERTAIN,
        vec![ResolutionEvidence::new(
            EvidenceKind::KnownLocatorAvailable,
            None,
        )],
    )
    .map_err(|error| error.to_string())?;
    Ok(Some((candidate, missing_frames)))
}

fn directory_holds_a_frame(path: &Path, descriptor: &ImageSequenceDescriptor) -> bool {
    let frames = descriptor.frames();
    let mut frame = frames.start();
    loop {
        if !descriptor.is_known_missing(frame)
            && path.join(descriptor.pattern().filename(frame)).is_file()
        {
            return true;
        }
        if frame == frames.end() {
            return false;
        }
        frame += i64::from(frames.step());
    }
}

fn sequence_missing_frames(
    path: &Path,
    descriptor: &ImageSequenceDescriptor,
) -> std::result::Result<Vec<i64>, String> {
    let names = fs::read_dir(path)
        .map_err(|error| format!("list image-sequence directory {}: {error}", path.display()))?
        .map(|entry| {
            entry.map(|entry| entry.file_name()).map_err(|error| {
                format!("read image-sequence directory {}: {error}", path.display())
            })
        })
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    let mut missing_frames = Vec::new();
    let frames = descriptor.frames();
    let mut frame = frames.start();
    loop {
        if !descriptor.is_known_missing(frame)
            && !names.contains(OsStr::new(&descriptor.pattern().filename(frame)))
        {
            missing_frames.push(frame);
            if missing_frames.len() > MAX_SEQUENCE_EXCEPTIONS {
                return Err(format!(
                    "image sequence has more than {MAX_SEQUENCE_EXCEPTIONS} missing frames"
                ));
            }
        }
        if frame == frames.end() {
            break;
        }
        frame += i64::from(frames.step());
    }

    Ok(missing_frames)
}

fn verify_candidate(
    path: &Path,
    uri: &str,
    mut evidence: Vec<ResolutionEvidence>,
    fingerprints: &FileFingerprints,
    technical_match: bool,
) -> std::result::Result<Option<ResolutionCandidate>, String> {
    if !fingerprints.comparable.is_empty() {
        let report = fingerprint_file(path).map_err(|error| error.to_string())?;
        let Some(expected) = fingerprints.comparable.iter().find(|expected| {
            expected.algorithm() == report.fingerprint().algorithm()
                && expected.version() == report.fingerprint().version()
        }) else {
            return Ok(None);
        };
        if report.fingerprint() != expected {
            return Ok(None);
        }
        let full = expected.algorithm() == FULL_FINGERPRINT_ALGORITHM;
        evidence.push(ResolutionEvidence::new(
            if full {
                EvidenceKind::FullHashMatch
            } else {
                EvidenceKind::PartialFingerprintMatch
            },
            Some(format!(
                "{} version {}",
                expected.algorithm(),
                expected.version()
            )),
        ));
        if full {
            evidence.push(ResolutionEvidence::new(
                EvidenceKind::ExactFingerprintMatch,
                None,
            ));
        }
        let confidence = if full {
            Confidence::CERTAIN
        } else {
            Confidence::from_basis_points(9_500).map_err(|error| error.to_string())?
        };
        return ResolutionCandidate::new(uri, confidence, evidence)
            .map(Some)
            .map_err(|error| error.to_string());
    }

    let filename_match = evidence
        .iter()
        .any(|item| item.kind() == EvidenceKind::FileNameMatch);
    if !filename_match {
        return Ok(None);
    }
    if let Some(item) = not_verified_evidence(&fingerprints.foreign) {
        evidence.push(item);
    }
    if technical_match {
        evidence.push(ResolutionEvidence::new(
            EvidenceKind::PartialFingerprintMatch,
            Some("technical media profile matched".to_owned()),
        ));
    }
    let confidence = Confidence::from_basis_points({
        let base = if evidence
            .iter()
            .any(|item| item.kind() == EvidenceKind::FileSizeMatch)
        {
            7_000
        } else {
            5_000
        };
        let relative_path = if evidence
            .iter()
            .any(|item| item.kind() == EvidenceKind::RelativePathSimilarity)
        {
            500
        } else {
            0
        };
        let technical = if technical_match { 1_000 } else { 0 };
        base + relative_path + technical
    })
    .map_err(|error| error.to_string())?;
    ResolutionCandidate::new(uri, confidence, evidence)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn error_resolution(
    resource_id: postproject_core::ResourceId,
    detail: String,
) -> Result<ResourceResolution> {
    ResourceResolution::new(
        resource_id,
        ResourceResolutionState::Error,
        Vec::new(),
        vec![ResolutionEvidence::new(
            EvidenceKind::DiscoveryError,
            Some(detail),
        )],
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use postproject_core::{
        ContentStructure, FrameRange, ImageSequenceDescriptor, ImageSequencePattern, LocatorId,
        MediaRoot, MediaRootId, RationalRate, RepresentationAvailability, RepresentationId,
        RepresentationResolution, ResourceId, ResourceResolutionState,
    };

    use super::*;
    use crate::{
        ImageSequenceSource, prepare_media_root, prepare_original_media, prepare_representation,
    };

    #[test]
    fn known_online_locator_wins_without_root_scan() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = directory.path().join("clip.mov");
        fs::write(&path, b"media").expect("write media");
        let prepared = prepare_original_media(&path, None, None).expect("prepare import");

        let resolution = MediaResolver::default()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[],
                &[],
            )
            .expect("resolve known locator");

        assert_eq!(
            resolution.state(),
            ResourceResolutionState::OnlineAtKnownLocator
        );
        assert_eq!(
            resolution.candidates()[0].uri(),
            prepared.locators()[0].uri()
        );
    }

    #[test]
    fn content_verification_detects_replacement_at_a_known_locator() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = directory.path().join("clip.mov");
        fs::write(&path, b"original content").expect("write original");
        let prepared = prepare_original_media(&path, None, None).expect("prepare import");
        fs::write(&path, b"replaced content").expect("replace in place");

        let presence = MediaResolver::default()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[],
                &[],
            )
            .expect("presence resolution");
        assert_eq!(
            presence.state(),
            ResourceResolutionState::OnlineAtKnownLocator
        );

        let verified = content_resolver()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[],
                &[],
            )
            .expect("verification result");
        assert_eq!(verified.state(), ResourceResolutionState::Error);
        assert_eq!(
            verified.evidence()[0].kind(),
            EvidenceKind::FingerprintMismatch
        );
    }

    #[test]
    fn sequence_directory_is_online_and_recorded_gaps_are_partial() {
        let directory = tempfile::tempdir().expect("create directory");
        let sequence_directory = directory.path().join("plate");
        fs::create_dir(&sequence_directory).expect("create sequence directory");
        fs::write(sequence_directory.join("plate.0001.exr"), b"frame 1")
            .expect("write first frame");
        fs::write(sequence_directory.join("plate.0003.exr"), b"frame 3")
            .expect("write third frame");
        let resource_id = ResourceId::new();
        let descriptor = ImageSequenceDescriptor::new(
            resource_id,
            ImageSequencePattern::new("plate.", ".exr", 4).expect("valid pattern"),
            FrameRange::new(1, 3, 1).expect("valid frame range"),
            RationalRate::new(24, 1).expect("valid rate"),
            vec![2],
        )
        .expect("valid sequence");
        let structure = ContentStructure::image_sequence(descriptor);
        let resource = Resource::new(resource_id, Vec::new(), None);
        let locator = Locator::new(
            LocatorId::new(),
            resource_id,
            canonical_file_uri(&sequence_directory).expect("sequence URI"),
            None,
            postproject_core::LocatorAvailability::Online,
        )
        .expect("valid locator");

        let resolved = MediaResolver::default()
            .resolve_resource(&resource, &structure, &[locator], &[], &[])
            .expect("resolve known sequence directory");
        assert_eq!(
            resolved.state(),
            ResourceResolutionState::OnlineAtKnownLocator
        );
        let aggregate = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &structure,
            vec![resolved],
        )
        .expect("aggregate sequence");
        assert_eq!(
            aggregate.availability(),
            RepresentationAvailability::Partial
        );
        assert_eq!(aggregate.issues()[0].frames(), &[2]);
    }

    #[test]
    fn sequence_directory_inventory_reports_unrecorded_gaps() {
        let directory = tempfile::tempdir().expect("create directory");
        fs::write(directory.path().join("plate.1001.exr"), b"frame 1001")
            .expect("write first frame");
        fs::write(directory.path().join("plate.1003.exr"), b"frame 1003")
            .expect("write third frame");
        let resource_id = ResourceId::new();
        let descriptor = ImageSequenceDescriptor::new(
            resource_id,
            ImageSequencePattern::new("plate.", ".exr", 4).expect("valid pattern"),
            FrameRange::new(1001, 1004, 1).expect("valid frame range"),
            RationalRate::new(24, 1).expect("valid rate"),
            Vec::new(),
        )
        .expect("valid sequence");
        let structure = ContentStructure::image_sequence(descriptor);
        let resource = Resource::new(resource_id, Vec::new(), None);
        let locator = Locator::new(
            LocatorId::new(),
            resource_id,
            canonical_file_uri(directory.path()).expect("sequence URI"),
            None,
            postproject_core::LocatorAvailability::Online,
        )
        .expect("valid locator");

        let resolved = MediaResolver::default()
            .resolve_resource(&resource, &structure, &[locator], &[], &[])
            .expect("resolve known sequence directory");
        assert_eq!(resolved.missing_frames(), &[1002, 1004]);
        let aggregate = RepresentationResolution::aggregate(
            RepresentationId::new(),
            &structure,
            vec![resolved],
        )
        .expect("aggregate sequence");

        assert_eq!(
            aggregate.availability(),
            RepresentationAvailability::Partial
        );
        assert_eq!(aggregate.issues()[0].frames(), &[1002, 1004]);
    }

    #[test]
    fn finds_a_relocated_sequence_as_one_resource() {
        let temporary = tempfile::tempdir().expect("create directory");
        let original = temporary.path().join("original");
        let mapped_root = temporary.path().join("mapped");
        fs::create_dir(&original).expect("create original sequence");
        fs::create_dir(&mapped_root).expect("create mapped root");
        for frame in 1001..=1003 {
            fs::write(
                original.join(format!("plate.{frame}.exr")),
                frame.to_string(),
            )
            .expect("write frame");
        }
        let prepared = prepare_representation(
            postproject_core::AssetId::new(),
            postproject_core::RepresentationKind::Original,
            ImageSequenceSource::new(
                &original,
                ImageSequencePattern::new("plate.", ".exr", 4).expect("pattern"),
                FrameRange::new(1001, 1003, 1).expect("range"),
                RationalRate::new(24, 1).expect("rate"),
                Vec::new(),
            ),
        )
        .expect("prepare sequence");
        let relocated = mapped_root.join("cards/day-01/plate");
        fs::create_dir_all(relocated.parent().expect("parent")).expect("create parent");
        fs::rename(&original, &relocated).expect("relocate sequence");
        let root = MediaRoot::new(MediaRootId::new(), "rushes", None, None, 0, true)
            .expect("portable root");
        let mapping = MediaRootMapping::new("rushes", &mapped_root).expect("root mapping");

        let resolved = MediaResolver::default()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[root],
                &[mapping],
            )
            .expect("resolve moved sequence");
        assert_eq!(resolved.state(), ResourceResolutionState::ResolvedProbable);
        assert_eq!(resolved.candidates().len(), 1);
        assert_eq!(
            resolved.candidates()[0].uri(),
            canonical_file_uri(&relocated).expect("relocated URI")
        );
    }

    #[test]
    fn sequence_gone_from_its_directory_is_searched_for() {
        let temporary = tempfile::tempdir().expect("create directory");
        let original = temporary.path().join("plates");
        let moved = temporary.path().join("day-01");
        fs::create_dir(&original).expect("create original sequence");
        fs::create_dir(&moved).expect("create new location");
        for frame in 1..=3 {
            fs::write(original.join(format!("shot_{frame:04}.png")), [frame; 8])
                .expect("write frame");
        }
        let prepared = prepare_representation(
            postproject_core::AssetId::new(),
            postproject_core::RepresentationKind::Original,
            ImageSequenceSource::new(
                &original,
                ImageSequencePattern::new("shot_", ".png", 4).expect("pattern"),
                FrameRange::new(1, 3, 1).expect("range"),
                RationalRate::new(24, 1).expect("rate"),
                Vec::new(),
            ),
        )
        .expect("prepare sequence");
        for frame in 1..=3 {
            let name = format!("shot_{frame:04}.png");
            fs::rename(original.join(&name), moved.join(&name)).expect("move frame");
        }
        let root =
            MediaRoot::new(MediaRootId::new(), "work", None, None, 0, true).expect("portable root");
        let mapping = MediaRootMapping::new("work", temporary.path()).expect("root mapping");

        let resolved = content_resolver()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[root],
                &[mapping],
            )
            .expect("resolve moved sequence");

        assert_eq!(resolved.state(), ResourceResolutionState::ResolvedProbable);
        assert_eq!(
            resolved.candidates()[0].uri(),
            canonical_file_uri(&moved).expect("moved URI")
        );
    }

    #[test]
    fn moved_small_file_resolves_by_full_hash() {
        let directory = tempfile::tempdir().expect("create directory");
        let old_directory = directory.path().join("old");
        let new_directory = directory.path().join("new");
        fs::create_dir(&old_directory).expect("create old directory");
        fs::create_dir(&new_directory).expect("create new directory");
        let old_path = old_directory.join("clip.mov");
        let new_path = new_directory.join("renamed.mov");
        fs::write(&old_path, b"media").expect("write media");
        let prepared = prepare_original_media(&old_path, None, None).expect("prepare import");
        fs::rename(&old_path, &new_path).expect("move media");
        let root = prepare_media_root(&new_directory, None, 0).expect("prepare root");

        let resolution = MediaResolver::default()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[root],
                &[],
            )
            .expect("resolve moved media");

        assert_eq!(resolution.state(), ResourceResolutionState::ResolvedExact);
        assert_eq!(resolution.candidates().len(), 1);
        assert!(
            resolution.candidates()[0]
                .evidence()
                .iter()
                .any(|evidence| evidence.kind() == EvidenceKind::FullHashMatch)
        );
    }

    fn content_resolver() -> MediaResolver {
        MediaResolver::new(ResolverOptions {
            verification: VerificationMode::Content,
            ..ResolverOptions::default()
        })
        .expect("valid options")
    }

    fn with_only_foreign_fingerprint(resource: &Resource) -> Resource {
        Resource::new(
            resource.id(),
            vec![
                ResourceFingerprint::new("example-host-md5", 1, vec![0xab; 16])
                    .expect("foreign fingerprint"),
            ],
            resource.file_facts(),
        )
    }

    fn has_evidence(candidate: &ResolutionCandidate, kind: EvidenceKind) -> bool {
        candidate
            .evidence()
            .iter()
            .any(|evidence| evidence.kind() == kind)
    }

    #[test]
    fn foreign_fingerprint_does_not_block_filename_discovery() {
        let directory = tempfile::tempdir().expect("create directory");
        let old_directory = directory.path().join("old");
        let new_directory = directory.path().join("new");
        fs::create_dir(&old_directory).expect("create old directory");
        fs::create_dir(&new_directory).expect("create new directory");
        let old_path = old_directory.join("clip.mov");
        fs::write(&old_path, b"media").expect("write media");
        let prepared = prepare_original_media(&old_path, None, None).expect("prepare import");
        fs::rename(&old_path, new_directory.join("clip.mov")).expect("move media");
        fs::write(new_directory.join("other.mov"), b"media").expect("write same-size file");
        let root = prepare_media_root(&new_directory, None, 0).expect("prepare root");
        let resource = with_only_foreign_fingerprint(&prepared.resources()[0]);

        let resolution = MediaResolver::default()
            .resolve_resource(
                &resource,
                prepared.representation().content_structure(),
                prepared.locators(),
                &[root],
                &[],
            )
            .expect("resolve moved media");

        assert_eq!(
            resolution.state(),
            ResourceResolutionState::ResolvedProbable
        );
        assert_eq!(resolution.candidates().len(), 1);
        let candidate = &resolution.candidates()[0];
        assert!(candidate.uri().ends_with("/new/clip.mov"));
        assert!(candidate.confidence() < Confidence::CERTAIN);
        assert!(has_evidence(candidate, EvidenceKind::FileNameMatch));
        let not_verified = candidate
            .evidence()
            .iter()
            .find(|evidence| evidence.kind() == EvidenceKind::FingerprintNotVerified)
            .expect("not-verified evidence");
        assert_eq!(not_verified.detail(), Some("example-host-md5 version 1"));
    }

    #[test]
    fn content_verification_does_not_report_foreign_fingerprint_as_mismatch() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = directory.path().join("clip.mov");
        fs::write(&path, b"media").expect("write media");
        let prepared = prepare_original_media(&path, None, None).expect("prepare import");
        let resource = with_only_foreign_fingerprint(&prepared.resources()[0]);

        let verified = content_resolver()
            .resolve_resource(
                &resource,
                prepared.representation().content_structure(),
                prepared.locators(),
                &[],
                &[],
            )
            .expect("verify known locator");

        assert_eq!(
            verified.state(),
            ResourceResolutionState::OnlineAtKnownLocator
        );
        let candidate = &verified.candidates()[0];
        assert!(has_evidence(candidate, EvidenceKind::KnownLocatorAvailable));
        assert!(has_evidence(
            candidate,
            EvidenceKind::FingerprintNotVerified
        ));
        assert!(!has_evidence(candidate, EvidenceKind::FingerprintMismatch));
    }

    #[test]
    fn machine_mapping_resolves_a_portable_root() {
        let directory = tempfile::tempdir().expect("create directory");
        let old_directory = directory.path().join("workstation");
        let laptop_directory = directory.path().join("laptop");
        fs::create_dir(&old_directory).expect("create workstation directory");
        fs::create_dir(&laptop_directory).expect("create laptop directory");
        let old_path = old_directory.join("clip.mov");
        fs::write(&old_path, b"portable media").expect("write media");
        let prepared = prepare_original_media(&old_path, None, None).expect("prepare import");
        fs::rename(&old_path, laptop_directory.join("clip.mov")).expect("move media");
        let root = MediaRoot::new(
            MediaRootId::new(),
            "camera-originals",
            Some("Camera originals".to_owned()),
            None,
            0,
            true,
        )
        .expect("create portable root");
        let mapping =
            MediaRootMapping::new("camera-originals", &laptop_directory).expect("map root");

        let resolution = MediaResolver::default()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[root],
                &[mapping],
            )
            .expect("resolve through mapping");

        assert_eq!(resolution.state(), ResourceResolutionState::ResolvedExact);
        assert_eq!(resolution.candidates().len(), 1);
        assert_eq!(
            resolution.candidates()[0].media_root(),
            Some("camera-originals")
        );
    }

    #[test]
    fn unusable_roots_are_reported_without_hiding_reachable_results() {
        let directory = tempfile::tempdir().expect("create directory");
        let old_path = directory.path().join("old.mov");
        let reachable = directory.path().join("reachable");
        fs::create_dir(&reachable).expect("create reachable root");
        fs::write(&old_path, b"media").expect("write original");
        let prepared = prepare_original_media(&old_path, None, None).expect("prepare import");
        fs::rename(&old_path, reachable.join("moved.mov")).expect("move media");
        let unmapped = MediaRoot::new(MediaRootId::new(), "archive", None, None, 0, true)
            .expect("create unmapped root");
        let usable = MediaRoot::new(MediaRootId::new(), "working", None, None, 1, true)
            .expect("create mapped root");
        let mapping = MediaRootMapping::new("working", &reachable).expect("map reachable root");

        let resolution = MediaResolver::default()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[unmapped, usable],
                &[mapping],
            )
            .expect("resolve reachable root");

        assert_eq!(resolution.state(), ResourceResolutionState::ResolvedExact);
        assert!(resolution.evidence().iter().any(|evidence| {
            evidence.kind() == EvidenceKind::MediaRootUnmapped
                && evidence.detail() == Some("archive")
        }));
    }

    #[test]
    fn unmapped_root_is_not_reported_as_missing_media() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = directory.path().join("clip.mov");
        fs::write(&path, b"media").expect("write media");
        let prepared = prepare_original_media(&path, None, None).expect("prepare import");
        fs::remove_file(path).expect("remove known media");
        let root = MediaRoot::new(MediaRootId::new(), "offline-vault", None, None, 0, true)
            .expect("create root");

        let resolution = MediaResolver::default()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[root],
                &[],
            )
            .expect("report unmapped root");

        assert_eq!(resolution.state(), ResourceResolutionState::Error);
        assert_eq!(
            resolution.evidence()[0].kind(),
            EvidenceKind::MediaRootUnmapped
        );
    }

    #[test]
    fn identical_copies_are_ambiguous_and_deterministically_ordered() {
        let directory = tempfile::tempdir().expect("create directory");
        let old_path = directory.path().join("old.mov");
        fs::write(&old_path, b"identical media").expect("write original");
        let prepared = prepare_original_media(&old_path, None, None).expect("prepare import");
        fs::remove_file(&old_path).expect("remove old locator target");
        fs::write(directory.path().join("b.mov"), b"identical media").expect("write b");
        fs::write(directory.path().join("a.mov"), b"identical media").expect("write a");
        let root = prepare_media_root(directory.path(), None, 0).expect("prepare root");

        let resolution = MediaResolver::default()
            .resolve_resource(
                &prepared.resources()[0],
                prepared.representation().content_structure(),
                prepared.locators(),
                &[root],
                &[],
            )
            .expect("resolve ambiguous media");

        assert_eq!(resolution.state(), ResourceResolutionState::Ambiguous);
        assert_eq!(resolution.candidates().len(), 2);
        assert!(resolution.candidates()[0].uri() < resolution.candidates()[1].uri());
    }

    #[test]
    fn relative_path_similarity_orders_filename_only_candidates() {
        let directory = tempfile::tempdir().expect("create directory");
        let original_directory = directory.path().join("old/day01");
        let root_directory = directory.path().join("new");
        fs::create_dir_all(&original_directory).expect("create original directory");
        fs::create_dir_all(root_directory.join("day01")).expect("create similar directory");
        fs::create_dir_all(root_directory.join("other")).expect("create other directory");
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
            postproject_core::LocatorAvailability::Online,
        )
        .expect("locator");
        fs::remove_file(&original).expect("remove original");
        for path in [
            root_directory.join("day01/clip.mov"),
            root_directory.join("other/clip.mov"),
        ] {
            fs::write(path, b"same-size").expect("write candidate");
        }
        let root = prepare_media_root(&root_directory, None, 0).expect("root");

        let resolution = MediaResolver::default()
            .resolve_resource(&resource, &structure, &[locator], &[root], &[])
            .expect("resolve candidates");
        assert_eq!(resolution.state(), ResourceResolutionState::Ambiguous);
        assert!(resolution.candidates()[0].uri().contains("day01"));
        assert!(
            resolution.candidates()[0]
                .evidence()
                .iter()
                .any(|evidence| evidence.kind() == EvidenceKind::RelativePathSimilarity)
        );
        assert!(resolution.candidates()[0].confidence() > resolution.candidates()[1].confidence());
    }

    #[test]
    fn oversized_directory_is_searched_partially_without_hiding_other_directories() {
        let directory = tempfile::tempdir().expect("create directory");
        let old_path = directory.path().join("old").join("clip.mov");
        fs::create_dir(directory.path().join("old")).expect("create old directory");
        fs::write(&old_path, b"media").expect("write original");
        let prepared = prepare_original_media(&old_path, None, None).expect("prepare import");
        fs::remove_file(&old_path).expect("remove old locator target");
        let crowded = directory.path().join("crowded");
        fs::create_dir(&crowded).expect("create crowded directory");
        for index in 0..4 {
            fs::write(crowded.join(format!("other-{index}.mov")), b"other")
                .expect("write unrelated file");
        }
        let nearby = directory.path().join("nearby");
        fs::create_dir(&nearby).expect("create nearby directory");
        fs::write(nearby.join("clip.mov"), b"media").expect("write candidate");
        let resolver = MediaResolver::new(ResolverOptions {
            max_depth: 4,
            max_entries_per_directory: 3,
            ..ResolverOptions::default()
        })
        .expect("valid limits");
        let scope = SearchScope::default()
            .with_search_directory(&crowded)
            .with_search_directory(&nearby);

        let resolutions = resolver
            .resolve(
                &[ResolutionItem::new(
                    &prepared.resources()[0],
                    prepared.representation().content_structure(),
                    prepared.locators(),
                )],
                &scope,
            )
            .expect("resolve");

        let resolution = &resolutions[0];
        assert_eq!(resolution.state(), ResourceResolutionState::ResolvedExact);
        assert!(
            resolution.candidates()[0]
                .uri()
                .ends_with("/nearby/clip.mov")
        );
        assert_eq!(resolution.candidates()[0].media_root(), None);
        assert!(
            resolution
                .evidence()
                .iter()
                .any(|evidence| evidence.kind() == EvidenceKind::SearchTruncated)
        );
    }

    #[test]
    fn one_scan_serves_every_resource_and_cancellation_stops_resolution() {
        let directory = tempfile::tempdir().expect("create directory");
        let mut imports = Vec::new();
        for name in ["a.mov", "b.mov"] {
            let path = directory.path().join(name);
            fs::write(&path, name.as_bytes()).expect("write original");
            imports.push(prepare_original_media(&path, None, None).expect("prepare import"));
        }
        let moved = directory.path().join("moved");
        fs::create_dir(&moved).expect("create moved directory");
        for name in ["a.mov", "b.mov"] {
            fs::rename(directory.path().join(name), moved.join(name)).expect("move media");
        }
        let items = imports
            .iter()
            .map(|import| {
                ResolutionItem::new(
                    &import.resources()[0],
                    import.representation().content_structure(),
                    import.locators(),
                )
            })
            .collect::<Vec<_>>();
        let scope = SearchScope::default().with_search_directory(&moved);

        let resolutions = MediaResolver::default()
            .resolve(&items, &scope)
            .expect("resolve batch");
        assert_eq!(resolutions.len(), 2);
        for (resolution, name) in resolutions.iter().zip(["a.mov", "b.mov"]) {
            assert_eq!(resolution.state(), ResourceResolutionState::ResolvedExact);
            assert!(resolution.candidates()[0].uri().ends_with(name));
        }

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let cancelled = MediaResolver::new(ResolverOptions {
            cancellation: Some(cancellation),
            ..ResolverOptions::default()
        })
        .expect("valid options")
        .resolve(&items, &scope)
        .expect_err("cancelled");
        assert_eq!(cancelled.kind(), ErrorKind::Cancelled);
    }
}
