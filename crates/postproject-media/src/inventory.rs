//! Non-mutating filesystem inventory with a disposable sidecar cache.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use postproject_core::{
    ContentStructure, Error, ErrorKind, Locator, MediaRoot, ProductionId, ProductionRead,
    RepresentationId, Resource, ResourceFingerprint, ResourceId, Result,
};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{MediaRootMapping, canonical_file_uri, fingerprint_file};

const CACHE_VERSION: u16 = 1;
const MAX_CACHE_BYTES: u64 = 64 * 1024 * 1024;

/// Machine-inspectable category assigned by an inventory scan.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum InventoryCategory {
    /// A persisted resource is available at a known locator.
    KnownOnline,
    /// Some, but not all, required representation content is available.
    Partial,
    /// A persisted resource has no available locator or credible candidate.
    Missing,
    /// Unassociated media was found beneath a configured root.
    NewCandidate,
    /// Content at a known locator no longer matches recorded facts.
    Changed,
    /// More than one storage object matches one known resource.
    DuplicateCandidate,
    /// Relink evidence does not identify a unique candidate.
    AmbiguousRelinkCandidate,
    /// A logical root has no mapping on this machine.
    RootUnmapped,
    /// A mapped root cannot currently be scanned.
    RootUnavailable,
}

/// One deterministic inventory observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryItem {
    category: InventoryCategory,
    representation_id: Option<RepresentationId>,
    resource_id: Option<ResourceId>,
    uri: Option<String>,
    detail: Option<String>,
}

impl InventoryItem {
    fn new(
        category: InventoryCategory,
        representation_id: Option<RepresentationId>,
        resource_id: Option<ResourceId>,
        uri: Option<String>,
        detail: Option<String>,
    ) -> Self {
        Self {
            category,
            representation_id,
            resource_id,
            uri,
            detail,
        }
    }

    /// Returns the result category.
    #[must_use]
    pub const fn category(&self) -> InventoryCategory {
        self.category
    }

    /// Returns the affected representation, when applicable.
    #[must_use]
    pub const fn representation_id(&self) -> Option<RepresentationId> {
        self.representation_id
    }

    /// Returns the affected resource, when applicable.
    #[must_use]
    pub const fn resource_id(&self) -> Option<ResourceId> {
        self.resource_id
    }

    /// Returns the observed storage URI, when applicable.
    #[must_use]
    pub fn uri(&self) -> Option<&str> {
        self.uri.as_deref()
    }

    /// Returns optional explanatory detail.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

/// Work counters that make cache behavior testable without wall-clock timing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InventoryStats {
    /// Filesystem entries visited during this scan.
    pub entries_visited: usize,
    /// Content fingerprints computed during this scan.
    pub fingerprints_computed: usize,
    /// Unchanged fingerprints reused from the sidecar cache.
    pub fingerprint_cache_hits: usize,
    /// Whether an absent, incompatible, or corrupt cache caused a rebuild.
    pub cache_rebuilt: bool,
}

/// Complete non-mutating inventory result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryReport {
    items: Vec<InventoryItem>,
    stats: InventoryStats,
}

impl InventoryReport {
    /// Returns observations in deterministic category/object/URI order.
    #[must_use]
    pub fn items(&self) -> &[InventoryItem] {
        &self.items
    }

    /// Returns scan and cache work counters.
    #[must_use]
    pub const fn stats(&self) -> InventoryStats {
        self.stats
    }
}

/// Bounded scanner for roots and known production media.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryScanner {
    max_depth: usize,
    max_entries: usize,
}

impl Default for InventoryScanner {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_entries: 100_000,
        }
    }
}

#[derive(Clone)]
struct KnownResource {
    representation_id: RepresentationId,
    structure: ContentStructure,
    resource: Resource,
    locators: Vec<Locator>,
}

#[derive(Clone)]
struct ObservedFile {
    uri: String,
    size: u64,
    fingerprint: Option<CachedFingerprint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CacheFile {
    version: u16,
    production_id: String,
    roots: BTreeMap<String, String>,
    entries: Vec<CacheEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CacheEntry {
    uri: String,
    size: u64,
    modified_secs: u64,
    modified_nanos: u32,
    fingerprint: Option<CachedFingerprint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CachedFingerprint {
    algorithm: String,
    version: u16,
    value: Vec<u8>,
}

impl CachedFingerprint {
    fn matches(&self, fingerprint: &ResourceFingerprint) -> bool {
        self.algorithm == fingerprint.algorithm()
            && self.version == fingerprint.version()
            && self.value == fingerprint.value()
    }
}

impl InventoryScanner {
    /// Scans configured roots without mutating the production.
    ///
    /// `cache_path` is machine-local and may be absent, deleted, or corrupt.
    /// Any unusable cache is ignored and replaced after a successful scan.
    ///
    /// # Errors
    ///
    /// Returns an error when production reads fail, scan bounds are exceeded,
    /// or a usable root cannot be represented safely. Individual unmapped and
    /// unavailable roots are report items rather than failed calls.
    pub fn scan<R: ProductionRead>(
        &self,
        production: &R,
        mappings: &[MediaRootMapping],
        cache_path: Option<&Path>,
    ) -> Result<InventoryReport> {
        let known = load_known_resources(production)?;
        let configured_roots = production.media_roots()?;
        let roots = resolve_roots(&configured_roots, mappings);
        let root_signature = root_signature(&roots);
        let (cache, mut stats) =
            load_cache(cache_path, production.production().id(), &root_signature);
        let cached = cache
            .entries
            .iter()
            .map(|entry| (entry.uri.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let mut items = Vec::new();
        let fingerprint_sizes = fingerprint_sizes(&known);
        let (observed, cache_entries) =
            self.scan_roots(roots, &cached, &fingerprint_sizes, &mut stats, &mut items)?;

        classify(&known, &observed, &mut items);
        inspect_partial_structures(&known, &mut items)?;
        items.sort_by(|left, right| {
            (
                left.category,
                left.representation_id,
                left.resource_id,
                left.uri.as_deref(),
                left.detail.as_deref(),
            )
                .cmp(&(
                    right.category,
                    right.representation_id,
                    right.resource_id,
                    right.uri.as_deref(),
                    right.detail.as_deref(),
                ))
        });
        items.dedup();

        if let Some(path) = cache_path {
            let next = CacheFile {
                version: CACHE_VERSION,
                production_id: production.production().id().to_string(),
                roots: root_signature,
                entries: cache_entries,
            };
            persist_cache(path, &next)?;
        }
        Ok(InventoryReport { items, stats })
    }

    fn scan_roots(
        &self,
        roots: Vec<(&MediaRoot, Option<PathBuf>)>,
        cached: &BTreeMap<&str, &CacheEntry>,
        fingerprint_sizes: &BTreeSet<u64>,
        stats: &mut InventoryStats,
        items: &mut Vec<InventoryItem>,
    ) -> Result<(Vec<ObservedFile>, Vec<CacheEntry>)> {
        let mut observed = Vec::new();
        let mut cache_entries = Vec::new();
        for (root, path) in roots {
            let Some(path) = path else {
                items.push(InventoryItem::new(
                    InventoryCategory::RootUnmapped,
                    None,
                    None,
                    None,
                    Some(root.name().to_owned()),
                ));
                continue;
            };
            if !path.is_dir() {
                items.push(InventoryItem::new(
                    InventoryCategory::RootUnavailable,
                    None,
                    None,
                    None,
                    Some(format!("{}: {}", root.name(), path.display())),
                ));
                continue;
            }
            self.scan_root(
                root,
                &path,
                cached,
                fingerprint_sizes,
                stats,
                items,
                &mut observed,
                &mut cache_entries,
            )?;
        }
        Ok((observed, cache_entries))
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_root(
        &self,
        root: &MediaRoot,
        path: &Path,
        cached: &BTreeMap<&str, &CacheEntry>,
        fingerprint_sizes: &BTreeSet<u64>,
        stats: &mut InventoryStats,
        items: &mut Vec<InventoryItem>,
        observed: &mut Vec<ObservedFile>,
        cache_entries: &mut Vec<CacheEntry>,
    ) -> Result<()> {
        for entry in WalkDir::new(path)
            .follow_links(false)
            .max_depth(self.max_depth)
            .sort_by_file_name()
        {
            stats.entries_visited = stats.entries_visited.saturating_add(1);
            if stats.entries_visited > self.max_entries {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("inventory entry limit {} exceeded", self.max_entries),
                ));
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    items.push(InventoryItem::new(
                        InventoryCategory::RootUnavailable,
                        None,
                        None,
                        None,
                        Some(format!("{}: {error}", root.name())),
                    ));
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let metadata = entry.metadata().map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("inspect inventory file {}: {error}", entry.path().display()),
                )
            })?;
            let uri = canonical_file_uri(entry.path())?;
            let (modified_secs, modified_nanos) = modified_parts(&metadata)?;
            let fingerprint = fingerprint_for_entry(
                entry.path(),
                &uri,
                &metadata,
                (modified_secs, modified_nanos),
                cached,
                fingerprint_sizes,
                stats,
            )?;
            cache_entries.push(CacheEntry {
                uri: uri.clone(),
                size: metadata.len(),
                modified_secs,
                modified_nanos,
                fingerprint: fingerprint.clone(),
            });
            observed.push(ObservedFile {
                uri,
                size: metadata.len(),
                fingerprint,
            });
        }
        Ok(())
    }
}

fn root_signature(roots: &[(&MediaRoot, Option<PathBuf>)]) -> BTreeMap<String, String> {
    roots
        .iter()
        .filter_map(|(root, path)| {
            path.as_ref()
                .map(|path| (root.name().to_owned(), path.to_string_lossy().into_owned()))
        })
        .collect()
}

fn fingerprint_sizes(known: &[KnownResource]) -> BTreeSet<u64> {
    known
        .iter()
        .filter(|known| !known.resource.fingerprints().is_empty())
        .filter_map(|known| {
            known
                .resource
                .file_facts()
                .map(postproject_core::FileFacts::size_bytes)
        })
        .collect()
}

fn fingerprint_for_entry(
    path: &Path,
    uri: &str,
    metadata: &fs::Metadata,
    modified: (u64, u32),
    cached: &BTreeMap<&str, &CacheEntry>,
    fingerprint_sizes: &BTreeSet<u64>,
    stats: &mut InventoryStats,
) -> Result<Option<CachedFingerprint>> {
    if !fingerprint_sizes.contains(&metadata.len()) {
        return Ok(None);
    }
    if let Some(cached) = cached.get(uri).filter(|cached| {
        cached.size == metadata.len()
            && cached.modified_secs == modified.0
            && cached.modified_nanos == modified.1
    }) {
        stats.fingerprint_cache_hits += usize::from(cached.fingerprint.is_some());
        return Ok(cached.fingerprint.clone());
    }
    let report = fingerprint_file(path)?;
    stats.fingerprints_computed += 1;
    Ok(Some(CachedFingerprint {
        algorithm: report.fingerprint().algorithm().to_owned(),
        version: report.fingerprint().version(),
        value: report.fingerprint().value().to_vec(),
    }))
}

fn load_known_resources<R: ProductionRead>(production: &R) -> Result<Vec<KnownResource>> {
    let mut known = Vec::new();
    for asset in production.assets()? {
        for representation in production.representations(asset.id())? {
            for resource in production.resources(representation.id())? {
                known.push(KnownResource {
                    representation_id: representation.id(),
                    structure: representation.content_structure().clone(),
                    locators: production.locators(resource.id())?,
                    resource,
                });
            }
        }
    }
    Ok(known)
}

fn resolve_roots<'a>(
    roots: &'a [MediaRoot],
    mappings: &'a [MediaRootMapping],
) -> Vec<(&'a MediaRoot, Option<PathBuf>)> {
    let by_name = mappings
        .iter()
        .map(|mapping| (mapping.name(), mapping.directory()))
        .collect::<BTreeMap<_, _>>();
    roots
        .iter()
        .filter(|root| root.is_enabled())
        .map(|root| {
            let path = by_name
                .get(root.name())
                .map(|path| (*path).to_path_buf())
                .or_else(|| {
                    root.legacy_uri()
                        .and_then(|uri| url::Url::parse(uri).ok())
                        .and_then(|uri| uri.to_file_path().ok())
                });
            (root, path)
        })
        .collect()
}

fn classify(known: &[KnownResource], observed: &[ObservedFile], items: &mut Vec<InventoryItem>) {
    let known_by_uri = known
        .iter()
        .flat_map(|known| {
            known
                .locators
                .iter()
                .map(move |locator| (locator.uri(), known))
        })
        .collect::<BTreeMap<_, _>>();
    let matches_by_resource = classify_observed(known, observed, &known_by_uri, items);
    classify_known(known, observed, &matches_by_resource, items);
}

fn classify_observed<'a>(
    known: &[KnownResource],
    observed: &'a [ObservedFile],
    known_by_uri: &BTreeMap<&str, &KnownResource>,
    items: &mut Vec<InventoryItem>,
) -> BTreeMap<ResourceId, Vec<&'a ObservedFile>> {
    let mut matches_by_resource: BTreeMap<ResourceId, Vec<&ObservedFile>> = BTreeMap::new();
    for file in observed {
        if let Some(known) = known_by_uri.get(file.uri.as_str()) {
            let facts_match = known
                .resource
                .file_facts()
                .is_none_or(|facts| facts.size_bytes() == file.size);
            let fingerprint_match = known.resource.fingerprints().is_empty()
                || file.fingerprint.as_ref().is_some_and(|observed| {
                    known
                        .resource
                        .fingerprints()
                        .iter()
                        .any(|fingerprint| observed.matches(fingerprint))
                });
            items.push(InventoryItem::new(
                if facts_match && fingerprint_match {
                    InventoryCategory::KnownOnline
                } else {
                    InventoryCategory::Changed
                },
                Some(known.representation_id),
                Some(known.resource.id()),
                Some(file.uri.clone()),
                None,
            ));
            continue;
        }

        let matching = known
            .iter()
            .filter(|known| {
                known
                    .resource
                    .file_facts()
                    .is_some_and(|facts| facts.size_bytes() == file.size)
                    && file.fingerprint.as_ref().is_some_and(|observed| {
                        known
                            .resource
                            .fingerprints()
                            .iter()
                            .any(|fingerprint| observed.matches(fingerprint))
                    })
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            items.push(InventoryItem::new(
                InventoryCategory::NewCandidate,
                None,
                None,
                Some(file.uri.clone()),
                None,
            ));
        } else {
            for known in &matching {
                matches_by_resource
                    .entry(known.resource.id())
                    .or_default()
                    .push(file);
            }
            if matching.len() > 1 {
                items.push(InventoryItem::new(
                    InventoryCategory::AmbiguousRelinkCandidate,
                    None,
                    None,
                    Some(file.uri.clone()),
                    Some("candidate matches more than one resource".to_owned()),
                ));
            }
        }
    }
    matches_by_resource
}

fn classify_known(
    known: &[KnownResource],
    observed: &[ObservedFile],
    matches_by_resource: &BTreeMap<ResourceId, Vec<&ObservedFile>>,
    items: &mut Vec<InventoryItem>,
) {
    for known in known {
        let online_locator = known.locators.iter().any(|locator| {
            observed.iter().any(|file| file.uri == locator.uri())
                || locator_path(locator).is_some_and(|path| path.is_file() || path.is_dir())
        });
        let candidates = matches_by_resource
            .get(&known.resource.id())
            .map_or(&[][..], Vec::as_slice);
        if candidates.len() > 1 {
            for candidate in candidates {
                items.push(InventoryItem::new(
                    InventoryCategory::DuplicateCandidate,
                    Some(known.representation_id),
                    Some(known.resource.id()),
                    Some(candidate.uri.clone()),
                    None,
                ));
                items.push(InventoryItem::new(
                    InventoryCategory::AmbiguousRelinkCandidate,
                    Some(known.representation_id),
                    Some(known.resource.id()),
                    Some(candidate.uri.clone()),
                    None,
                ));
            }
        }
        if !online_locator && candidates.is_empty() {
            items.push(InventoryItem::new(
                InventoryCategory::Missing,
                Some(known.representation_id),
                Some(known.resource.id()),
                None,
                None,
            ));
        }
    }
}

fn inspect_partial_structures(
    known: &[KnownResource],
    items: &mut Vec<InventoryItem>,
) -> Result<()> {
    let mut by_representation: BTreeMap<RepresentationId, Vec<&KnownResource>> = BTreeMap::new();
    for resource in known {
        by_representation
            .entry(resource.representation_id)
            .or_default()
            .push(resource);
    }
    for (representation_id, resources) in by_representation {
        let structure = &resources[0].structure;
        if let Some(sequence) = structure.image_sequence_descriptor() {
            let Some((directory, naming)) = resources[0].locators.iter().find_map(|locator| {
                let path = locator_path(locator).filter(|path| path.is_dir())?;
                Some((path, locator.sequence_naming()?))
            }) else {
                continue;
            };
            let names = fs::read_dir(&directory)
                .map_err(|error| {
                    Error::new(
                        ErrorKind::Io,
                        format!("list image sequence {}: {error}", directory.display()),
                    )
                })?
                .filter_map(std::result::Result::ok)
                .map(|entry| entry.file_name())
                .collect::<BTreeSet<_>>();
            let mut missing = Vec::new();
            let mut frame = sequence.frames().start();
            loop {
                if !sequence.is_known_missing(frame)
                    && !names.contains(OsStr::new(&naming.filename(frame)))
                {
                    missing.push(frame);
                }
                if frame == sequence.frames().end() {
                    break;
                }
                frame += i64::from(sequence.frames().step());
            }
            if !missing.is_empty() {
                items.push(InventoryItem::new(
                    InventoryCategory::Partial,
                    Some(representation_id),
                    Some(sequence.resource_id()),
                    None,
                    Some(format!("missing frames: {missing:?}")),
                ));
            }
        } else if let Some(members) = structure.members() {
            let required = members.iter().filter(|member| member.is_required());
            let states = required
                .map(|member| {
                    resources
                        .iter()
                        .find(|known| known.resource.id() == member.resource_id())
                        .is_some_and(|known| {
                            known
                                .locators
                                .iter()
                                .find_map(locator_path)
                                .is_some_and(|path| path.is_file())
                        })
                })
                .collect::<Vec<_>>();
            if states.iter().any(|online| *online) && states.iter().any(|online| !online) {
                items.push(InventoryItem::new(
                    InventoryCategory::Partial,
                    Some(representation_id),
                    None,
                    None,
                    Some("required package or span members are missing".to_owned()),
                ));
            }
        }
    }
    Ok(())
}

fn locator_path(locator: &Locator) -> Option<PathBuf> {
    url::Url::parse(locator.uri()).ok()?.to_file_path().ok()
}

fn modified_parts(metadata: &fs::Metadata) -> Result<(u64, u32)> {
    let modified = metadata
        .modified()
        .map_err(|error| Error::new(ErrorKind::Io, format!("read modification time: {error}")))?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            Error::new(ErrorKind::Io, format!("invalid modification time: {error}"))
        })?;
    Ok((modified.as_secs(), modified.subsec_nanos()))
}

fn empty_cache(production_id: ProductionId, roots: &BTreeMap<String, String>) -> CacheFile {
    CacheFile {
        version: CACHE_VERSION,
        production_id: production_id.to_string(),
        roots: roots.clone(),
        entries: Vec::new(),
    }
}

fn load_cache(
    path: Option<&Path>,
    production_id: ProductionId,
    roots: &BTreeMap<String, String>,
) -> (CacheFile, InventoryStats) {
    let Some(path) = path else {
        return (empty_cache(production_id, roots), InventoryStats::default());
    };
    let loaded = fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.len() <= MAX_CACHE_BYTES)
        .and_then(|_| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<CacheFile>(&bytes).ok())
        .filter(|cache| {
            cache.version == CACHE_VERSION
                && cache.production_id == production_id.to_string()
                && cache.roots == *roots
        });
    match loaded {
        Some(cache) => (cache, InventoryStats::default()),
        None => (
            empty_cache(production_id, roots),
            InventoryStats {
                cache_rebuilt: true,
                ..InventoryStats::default()
            },
        ),
    }
}

fn persist_cache(path: &Path, cache: &CacheFile) -> Result<()> {
    let bytes = serde_json::to_vec(cache).map_err(|error| {
        Error::new(
            ErrorKind::Internal,
            format!("encode inventory cache: {error}"),
        )
    })?;
    if bytes.len() as u64 > MAX_CACHE_BYTES {
        return Err(Error::new(
            ErrorKind::Io,
            "inventory cache exceeds the 64 MiB limit",
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(
                    "create inventory cache directory {}: {error}",
                    parent.display()
                ),
            )
        })?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("write inventory cache {}: {error}", temporary.display()),
        )
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("replace inventory cache {}: {error}", path.display()),
        )
    })
}
