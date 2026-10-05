//! Runs the Rust listings of the media structure and resolution guide.
//!
//! Each `// [name]` ... `// [/name]` region is included verbatim by the
//! documentation build, so keep regions self-contained and readable.

use std::fs;
use std::path::{Path, PathBuf};

use postproject_core::{
    AssetId, AvailabilityIssueKind, CancellationToken, ContentStructureKind, EvidenceKind,
    FrameRange, MediaRoot, MediaRootId, QueryPageRequest, RationalRate, RepresentationAvailability,
    RepresentationId, RepresentationKind, RepresentationResolution, ResourceId,
    ResourceResolutionState, ResourceRole, Result, RevisionEventKind, SequenceNaming,
};
use postproject_media::{
    ContentObservationOutcome, ContentVerification, FileResourceSource, ImageSequenceSource,
    InventoryCategory, InventoryItem, InventoryScanner, MediaRecognizer, MediaResolver,
    MediaRootMapping, MediaSource, PRIMARY_ESSENCE_ROLE, RecognizedMedia, ResolutionItem,
    ResolverOptions, SIDECAR_ROLE, SPAN_PART_ROLE, SearchScope, VerificationMode,
    canonical_file_uri, fingerprint_file, local_file_path, observe_resource_content,
    prepare_confirmed_locator, prepare_original_media, prepare_recognized_original_media,
    prepare_representation, resource_usage, verify_resource_content,
};
use postproject_storage_sqlite::SqliteProduction;

#[cfg(unix)]
use postproject_core::ObjectRef;
#[cfg(unix)]
use postproject_media::{FfprobeInspector, InspectionOutcome, MediaInspector, TechnicalMetadata};

// [import-sequence]
fn import_image_strip(production: &mut SqliteProduction, directory: &Path) -> Result<AssetId> {
    // The sequence becomes the new asset's only original representation. The
    // directory and file naming become its locator; frames and rate its
    // descriptor.
    let strip = ImageSequenceSource::new(
        directory,
        SequenceNaming::new("shot010.", ".exr", 4)?,
        FrameRange::new(1001, 1004, 1)?,
        RationalRate::new(24, 1)?,
        vec![1003],
    );
    let import = prepare_original_media(strip, Some("shot010 strip".to_owned()), None)?;
    let asset_id = import.asset().id();
    {
        let mut transaction = production.begin_transaction()?;
        transaction.import_original(&import)?;
        transaction.commit()?;
    }

    let representations = production.representations(asset_id)?;
    let original = &representations[0];
    println!(
        "{} representation, {:?}",
        representations.len(),
        original.content_structure().kind()
    );
    Ok(asset_id)
}
// [/import-sequence]

// [add-representation]
fn add_proxy(
    production: &mut SqliteProduction,
    asset_id: AssetId,
    proxy: &Path,
) -> Result<RepresentationId> {
    // A path converts to a single-file source.
    let import = prepare_representation(asset_id, RepresentationKind::Proxy, proxy)?;
    let mut transaction = production.begin_transaction()?;
    transaction.add_representation(&import)?;
    transaction.commit()?;
    Ok(import.representation().id())
}
// [/add-representation]

// [ordered-parts]
fn add_spanned_clip(
    production: &mut SqliteProduction,
    asset_id: AssetId,
    spanned: &Path,
) -> Result<RepresentationId> {
    // Order is significant: the parts play back as one continuous clip.
    let parts = MediaSource::OrderedParts(vec![
        FileResourceSource::new(
            spanned.join("CLIP0001.MTS"),
            ResourceRole::new(PRIMARY_ESSENCE_ROLE)?,
            true,
        ),
        FileResourceSource::new(
            spanned.join("CLIP0002.MTS"),
            ResourceRole::new(SPAN_PART_ROLE)?,
            true,
        ),
    ]);
    let import = prepare_representation(asset_id, RepresentationKind::Original, parts)?;
    let mut transaction = production.begin_transaction()?;
    transaction.add_representation(&import)?;
    transaction.commit()?;
    Ok(import.representation().id())
}
// [/ordered-parts]

// [package-representation]
fn add_package(
    production: &mut SqliteProduction,
    asset_id: AssetId,
    package: &Path,
) -> Result<RepresentationId> {
    let members = MediaSource::Package(vec![
        FileResourceSource::new(
            package.join("clip.mxf"),
            ResourceRole::new(PRIMARY_ESSENCE_ROLE)?,
            true,
        ),
        // An optional member may be missing without making the package unusable.
        FileResourceSource::new(
            package.join("clip.xml"),
            ResourceRole::new(SIDECAR_ROLE)?,
            false,
        ),
    ]);
    let import = prepare_representation(asset_id, RepresentationKind::Derived, members)?;
    let mut transaction = production.begin_transaction()?;
    transaction.add_representation(&import)?;
    transaction.commit()?;
    Ok(import.representation().id())
}
// [/package-representation]

// [representation-structure]
fn print_structure(production: &SqliteProduction, asset_id: AssetId) -> Result<usize> {
    let mut count = 0;
    let mut cursor = None;
    loop {
        let page = production.representations_page(asset_id, &QueryPageRequest::new(2, cursor)?)?;
        for representation in page.items() {
            count += 1;
            let structure = representation.content_structure();
            println!(
                "{} {:?} {:?}",
                representation.id(),
                representation.kind(),
                structure.kind()
            );
            for fingerprint in representation.fingerprints() {
                println!(
                    "  fingerprint {} v{}",
                    fingerprint.algorithm(),
                    fingerprint.version()
                );
            }
            for member in structure.members().unwrap_or_default() {
                println!(
                    "  member {} as {} (required: {})",
                    member.resource_id(),
                    member.role().as_str(),
                    member.is_required()
                );
            }
            if let Some(sequence) = structure.image_sequence_descriptor() {
                let frames = sequence.frames();
                println!(
                    "  frames {}-{} at {}, known missing {:?}",
                    frames.start(),
                    frames.end(),
                    sequence.rate(),
                    sequence.known_missing_frames()
                );
            }
            for resource in production.resources(representation.id())? {
                println!("  resource {}", resource.id());
                for fingerprint in resource.fingerprints() {
                    println!("    fingerprint {}", fingerprint.algorithm());
                }
                for locator in production.locators(resource.id())? {
                    // A sequence locator names its directory and the files there.
                    match locator.sequence_naming() {
                        Some(naming) => {
                            println!("    locator {} {}", locator.uri(), naming.filename(1001));
                        }
                        None => println!("    locator {}", locator.uri()),
                    }
                }
            }
        }
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            return Ok(count);
        }
    }
}
// [/representation-structure]

// [media-root-lifecycle]
fn cycle_media_root(production: &mut SqliteProduction, name: &str) -> Result<()> {
    let view = production.read_session()?;
    for root in view.read().production().media_roots() {
        println!("root {} enabled: {}", root.name(), root.is_enabled());
    }
    let root_id = view
        .read()
        .production()
        .media_roots()
        .iter()
        .find(|root| root.name() == name)
        .map(MediaRoot::id)
        .expect("the root is configured");

    let base = view.decision_base();
    drop(view);
    let mut transaction = production.begin_edit(base)?;
    // A disabled root stays configured but is skipped during resolution.
    transaction.set_media_root_enabled(root_id, false)?;
    transaction.commit()?;
    drop(transaction);

    let base = production.read_session()?.decision_base();
    let mut transaction = production.begin_edit(base)?;
    transaction.set_media_root_enabled(root_id, true)?;
    transaction.remove_media_root(root_id)?;
    transaction.commit()
}
// [/media-root-lifecycle]

// [retire-locator]
fn move_to_archive(
    production: &mut SqliteProduction,
    resource_id: ResourceId,
    archived: &Path,
) -> Result<()> {
    let superseded: Vec<_> = production.locators(resource_id)?;
    let replacement =
        prepare_confirmed_locator(resource_id, canonical_file_uri(archived)?, None, None)?;
    {
        let mut transaction = production.begin_transaction()?;
        transaction.add_locator(&replacement)?;
        for locator in &superseded {
            transaction.retire_locator(locator.id())?;
        }
        transaction.commit()?;
    }

    let mut cursor = None;
    loop {
        let page = production.locators_page(resource_id, &QueryPageRequest::new(100, cursor)?)?;
        for locator in page.items() {
            println!("{} ({:?})", locator.uri(), locator.availability());
        }
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            return Ok(());
        }
    }
}
// [/retire-locator]

// [locator-uri]
fn is_recorded_locator(path: &Path, recorded_uri: &str) -> Result<bool> {
    // Spell the path as PostProject spells locators instead of building a URI
    // by hand, then compare the strings exactly.
    println!(
        "recorded locator {recorded_uri} is {}",
        local_file_path(recorded_uri)?.display()
    );
    Ok(canonical_file_uri(path)? == recorded_uri)
}
// [/locator-uri]

// [content-fingerprint]
fn print_file_fingerprint(path: &Path) -> Result<()> {
    // The same value import records; computing it records nothing.
    let report = fingerprint_file(path)?;
    let fingerprint = report.fingerprint();
    println!(
        "{} v{}: {} bytes",
        fingerprint.algorithm(),
        fingerprint.version(),
        fingerprint.value().len()
    );
    Ok(())
}
// [/content-fingerprint]

// [fingerprint-observation]
fn observe_changed_original(
    production: &mut SqliteProduction,
    original_id: RepresentationId,
    path: &Path,
) -> Result<()> {
    let since = production
        .latest_revision()?
        .map_or(0, |revision| revision.sequence());
    let resource_id = production.resources(original_id)?[0].id();
    let usage = resource_usage(production, resource_id)?;

    // Verification only reads: it compares the file with the stored value.
    let (representation, resources) = &usage[0];
    let resource = resources
        .iter()
        .find(|resource| resource.id() == resource_id);
    let verification = verify_resource_content(
        resource.expect("representation lists the resource"),
        representation.content_structure(),
        path,
        None,
    )?;
    assert_eq!(verification, ContentVerification::Differs);

    // The new resource value and every representation recomputed from it are
    // recorded in one revision. Recording an unchanged observation adds none.
    // An image-sequence resource also needs the naming of the files at path.
    let observation = observe_resource_content(resource_id, &usage, path, None)?;
    assert_eq!(observation.outcome(), ContentObservationOutcome::Changed);
    {
        let mut transaction = production.begin_transaction()?;
        transaction.record_resource_fingerprint(resource_id, observation.resource())?;
        if let Some(facts) = observation.file_facts() {
            transaction.record_resource_file_facts(resource_id, facts)?;
        }
        for (representation_id, fingerprint) in observation.representations() {
            transaction.record_representation_fingerprint(*representation_id, fingerprint)?;
        }
        transaction.commit()?;
    }

    for revision in production.changes_since(since, 10)? {
        for event in production.events_for_revision(revision.id())? {
            println!("revision {}: {:?}", revision.sequence(), event.kind());
        }
    }
    Ok(())
}
// [/fingerprint-observation]

// [resolve-scope]
fn find_nearby(
    production: &SqliteProduction,
    asset_ids: &[AssetId],
    directory: &Path,
    cancellation: CancellationToken,
) -> Result<Option<String>> {
    // A search directory is an unnamed, machine-local place such as the project
    // folder or where the media used to be; it is never recorded. Each searched
    // directory has its own budget, and another thread may cancel the token.
    let resolver = MediaResolver::new(ResolverOptions {
        max_depth: 16,
        max_entries_per_directory: 50_000,
        verification: VerificationMode::Presence,
        cancellation: Some(cancellation),
    })?;
    let mut inputs = Vec::new();
    for asset_id in asset_ids {
        for representation in production.representations(*asset_id)? {
            for resource in production.resources(representation.id())? {
                let locators = production.locators(resource.id())?;
                inputs.push((representation.clone(), resource, locators));
            }
        }
    }
    let items = inputs
        .iter()
        .map(|(representation, resource, locators)| {
            ResolutionItem::new(resource, representation.content_structure(), locators)
        })
        .collect::<Vec<_>>();
    let scope = SearchScope::default().with_search_directory(directory);
    // All resources are resolved together; each directory is scanned once.
    for resolution in resolver.resolve(&items, &scope)? {
        let discovered = matches!(
            resolution.state(),
            ResourceResolutionState::ResolvedExact | ResourceResolutionState::ResolvedProbable
        );
        // A candidate from a search directory has no media root.
        if let [candidate] = resolution.candidates() {
            if discovered && candidate.media_root().is_none() {
                return Ok(Some(candidate.uri().to_owned()));
            }
        }
    }
    Ok(None)
}
// [/resolve-scope]

// [relink-renamed-sequence]
fn relink_renamed_sequence(
    production: &mut SqliteProduction,
    asset_id: AssetId,
    directory: &Path,
) -> Result<Option<SequenceNaming>> {
    let mut inputs = Vec::new();
    for representation in production.representations(asset_id)? {
        for resource in production.resources(representation.id())? {
            let locators = production.locators(resource.id())?;
            inputs.push((representation.clone(), resource, locators));
        }
    }
    let items = inputs
        .iter()
        .map(|(representation, resource, locators)| {
            ResolutionItem::new(resource, representation.content_structure(), locators)
        })
        .collect::<Vec<_>>();
    let scope = SearchScope::default().with_search_directory(directory);
    for resolution in MediaResolver::default().resolve(&items, &scope)? {
        // A renamed sequence is found by content; the candidate carries the
        // naming its files have now.
        let [candidate] = resolution.candidates() else {
            continue;
        };
        let Some(naming) = candidate.sequence_naming() else {
            continue;
        };
        for evidence in candidate.evidence() {
            println!("{} {:?}", candidate.uri(), evidence.kind());
        }
        let locator = prepare_confirmed_locator(
            resolution.resource_id(),
            candidate.uri(),
            candidate.media_root(),
            Some(naming.clone()),
        )?;
        let mut transaction = production.begin_transaction()?;
        transaction.add_locator(&locator)?;
        transaction.commit()?;
        return Ok(Some(naming.clone()));
    }
    Ok(None)
}
// [/relink-renamed-sequence]

// [resolution-issues]
fn print_resolution_issues(
    production: &SqliteProduction,
    asset_id: AssetId,
) -> Result<Vec<RepresentationResolution>> {
    let resolver = MediaResolver::default();
    let roots = production.production().media_roots();
    let mut resolutions = Vec::new();
    for representation in production.representations(asset_id)? {
        let structure = representation.content_structure();
        let mut resources = Vec::new();
        for resource in production.resources(representation.id())? {
            let locators = production.locators(resource.id())?;
            resources.push(resolver.resolve_resource(
                &resource,
                structure,
                &locators,
                roots,
                &[],
            )?);
        }
        let resolution =
            RepresentationResolution::aggregate(representation.id(), structure, resources)?;
        println!("{}: {:?}", representation.id(), resolution.availability());
        for issue in resolution.issues() {
            println!(
                "  {:?} on {} (required: {}), frames {:?}",
                issue.kind(),
                issue.resource_id(),
                issue.is_required(),
                issue.frames()
            );
        }
        for resource in resolution.resources() {
            println!(
                "  resource {}: {:?}",
                resource.resource_id(),
                resource.state()
            );
            // Resource-level evidence explains a failed search, such as an unmapped root.
            for evidence in resource.evidence() {
                println!("    {:?} {:?}", evidence.kind(), evidence.detail());
            }
            for candidate in resource.candidates() {
                for evidence in candidate.evidence() {
                    println!("    {}: {:?}", candidate.uri(), evidence.kind());
                }
            }
        }
        resolutions.push(resolution);
    }
    Ok(resolutions)
}
// [/resolution-issues]

// [inventory-scan]
fn scan_rushes(
    production: &SqliteProduction,
    rushes: &Path,
    cache: &Path,
) -> Result<Vec<InventoryCategory>> {
    let mappings = [MediaRootMapping::new("rushes", rushes)?];
    // The cache is disposable; it only avoids re-hashing unchanged files.
    let report = InventoryScanner::default().scan(production, &mappings, Some(cache))?;
    for item in report.items() {
        println!(
            "{:?}: {:?} {:?}",
            item.category(),
            item.uri(),
            item.detail()
        );
    }
    println!(
        "{} entries visited, {} fingerprints computed",
        report.stats().entries_visited,
        report.stats().fingerprints_computed
    );
    Ok(report.items().iter().map(InventoryItem::category).collect())
}
// [/inventory-scan]

// [verify-resolution]
fn verify_contents(
    production: &SqliteProduction,
    asset_id: AssetId,
) -> Result<Vec<(ResourceId, ResourceResolutionState)>> {
    // Content mode re-fingerprints files at known locators instead of trusting
    // their presence.
    let resolver = MediaResolver::new(ResolverOptions {
        verification: VerificationMode::Content,
        ..ResolverOptions::default()
    })?;
    let mut inputs = Vec::new();
    for representation in production.representations(asset_id)? {
        for resource in production.resources(representation.id())? {
            let locators = production.locators(resource.id())?;
            inputs.push((representation.clone(), resource, locators));
        }
    }
    let items = inputs
        .iter()
        .map(|(representation, resource, locators)| {
            ResolutionItem::new(resource, representation.content_structure(), locators)
        })
        .collect::<Vec<_>>();
    let scope = SearchScope::new(production.production().media_roots().to_vec(), Vec::new());
    let mut states = Vec::new();
    for resolution in resolver.resolve(&items, &scope)? {
        for evidence in resolution.evidence() {
            println!("{}: {:?}", resolution.resource_id(), evidence.kind());
        }
        states.push((resolution.resource_id(), resolution.state()));
    }
    Ok(states)
}
// [/verify-resolution]

#[cfg(unix)]
// [media-inspection]
fn inspect_original(
    production: &mut SqliteProduction,
    original_id: RepresentationId,
    media: &Path,
    ffprobe: &Path,
) -> Result<bool> {
    let outcome = FfprobeInspector::with_executable(ffprobe).inspect(media)?;
    let metadata = match outcome {
        InspectionOutcome::Inspected(metadata) => metadata,
        // A missing or failing ffprobe is reported, not raised as an error.
        other => {
            println!("no technical metadata: {other:?}");
            return Ok(false);
        }
    };
    let target = ObjectRef::Representation(original_id);
    {
        let mut transaction = production.begin_transaction()?;
        for assertion in metadata.assertions() {
            transaction.add_metadata_value(target, assertion.property(), assertion.value())?;
        }
        transaction.commit()?;
    }

    let stored = TechnicalMetadata::from_assertions(&production.metadata(target)?);
    Ok(stored == Some(metadata))
}
// [/media-inspection]

// [media-recognition]
fn import_with_sidecar(production: &mut SqliteProduction, clip: &Path) -> Result<AssetId> {
    let recognizer = MediaRecognizer::new(RationalRate::new(24, 1)?);
    // A file with a same-named .xml, .xmp, or .json sidecar becomes a package.
    let candidates = recognizer.recognize(clip)?;
    let [media @ RecognizedMedia::Package(members)] = candidates.as_slice() else {
        panic!("expected one package with a sidecar");
    };
    for member in members {
        println!(
            "{} as {} (required: {})",
            member.path().display(),
            member.role(),
            member.is_required()
        );
    }

    let import = prepare_recognized_original_media(media, Some("B001".to_owned()), None)?;
    let mut transaction = production.begin_transaction()?;
    transaction.import_original(&import)?;
    transaction.commit()?;
    Ok(import.asset().id())
}
// [/media-recognition]

fn production_sequence(production: &SqliteProduction) -> Result<u64> {
    Ok(production
        .latest_revision()?
        .map_or(0, |revision| revision.sequence()))
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/fixtures/sample-media.dat")
}

#[cfg(unix)]
fn fake_ffprobe(directory: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.join("ffprobe-fake");
    let output = r#"{"format":{"format_name":"mov,mp4","duration":"1.0"},"streams":[{"index":0,"codec_type":"video","codec_name":"prores","width":1920,"height":1080}]}"#;
    fs::write(&path, format!("#!/bin/sh\nprintf '%s' '{output}'\n")).expect("fake ffprobe");
    let mut permissions = fs::metadata(&path).expect("fake metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("fake executable");
    path
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("parent directory")).expect("work directory");
    fs::write(path, contents).expect("work file");
}

fn prepare_work_directory(work: &Path) {
    fs::create_dir_all(work.join("rushes")).expect("rushes directory");
    fs::copy(fixture(), work.join("rushes/A001.mov")).expect("media fixture");
    for frame in [1001, 1002, 1004] {
        write(
            &work.join(format!("renders/shot010/shot010.{frame}.exr")),
            &format!("frame {frame}"),
        );
    }
    write(&work.join("proxies/A001_proxy.mov"), "proxy of A001");
    write(&work.join("spanned/CLIP0001.MTS"), "first span");
    write(&work.join("spanned/CLIP0002.MTS"), "second span");
    write(&work.join("package/clip.mxf"), "package essence");
    write(&work.join("package/clip.xml"), "<clip/>");
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered scenario verifies every media listing"
)]
fn media_examples_run_in_order() -> Result<()> {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let work = temporary.path();
    prepare_work_directory(work);
    let original_path = work.join("rushes/A001.mov");

    let mut production = SqliteProduction::create(work.join("media.pproj"), None)?;
    let import = prepare_original_media(&original_path, Some("A001".to_owned()), None)?;
    let asset_id = import.asset().id();
    let original_id = import.representation().id();
    {
        let mut transaction = production.begin_transaction()?;
        transaction.import_original(&import)?;
        for name in ["rushes", "archive"] {
            transaction.add_media_root(MediaRoot::new(
                MediaRootId::new(),
                name,
                None,
                None,
                0,
                true,
            )?)?;
        }
        transaction.commit()?;
    }

    let proxy_id = add_proxy(
        &mut production,
        asset_id,
        &work.join("proxies/A001_proxy.mov"),
    )?;
    let spanned_id = add_spanned_clip(&mut production, asset_id, &work.join("spanned"))?;
    let package_id = add_package(&mut production, asset_id, &work.join("package"))?;
    let sequence = ImageSequenceSource::new(
        work.join("renders/shot010"),
        SequenceNaming::new("shot010.", ".exr", 4)?,
        FrameRange::new(1001, 1004, 1)?,
        RationalRate::new(24, 1)?,
        vec![1003],
    );
    let sequence_import = prepare_representation(asset_id, RepresentationKind::Derived, sequence)?;
    let sequence_id = sequence_import.representation().id();
    {
        let mut transaction = production.begin_transaction()?;
        transaction.add_representation(&sequence_import)?;
        transaction.commit()?;
    }
    let strip_id = import_image_strip(&mut production, &work.join("renders/shot010"))?;
    let strip = production.representations(strip_id)?;
    assert_eq!(strip.len(), 1);
    assert_eq!(strip[0].kind(), RepresentationKind::Original);
    assert_eq!(
        strip[0].content_structure().kind(),
        ContentStructureKind::ImageSequence
    );
    let representations = production.representations(asset_id)?;
    let kind_of = |id: RepresentationId| {
        representations
            .iter()
            .find(|representation| representation.id() == id)
            .map(|representation| {
                (
                    representation.kind(),
                    representation.content_structure().kind(),
                )
            })
    };
    assert_eq!(
        kind_of(proxy_id),
        Some((
            RepresentationKind::Proxy,
            ContentStructureKind::SingleResource
        ))
    );
    assert_eq!(
        kind_of(spanned_id),
        Some((
            RepresentationKind::Original,
            ContentStructureKind::OrderedParts
        ))
    );
    assert_eq!(
        kind_of(package_id),
        Some((RepresentationKind::Derived, ContentStructureKind::Package))
    );
    assert_eq!(production.resources(spanned_id)?.len(), 2);
    assert_eq!(production.resources(package_id)?.len(), 2);

    assert_eq!(print_structure(&production, asset_id)?, 5);

    let states = verify_contents(&production, asset_id)?;
    assert_eq!(states.len(), 7);
    assert!(
        states
            .iter()
            .all(|(_, state)| *state == ResourceResolutionState::OnlineAtKnownLocator)
    );
    // Changing a file's bytes behind the production's back is caught by content verification.
    write(&work.join("proxies/A001_proxy.mov"), "a different proxy");
    let proxy_resource = production.resources(proxy_id)?[0].id();
    let states = verify_contents(&production, asset_id)?;
    assert!(states.contains(&(proxy_resource, ResourceResolutionState::Error)));

    write(&original_path, "re-exported camera original");
    let before = production_sequence(&production)?;
    let recorded = production.locators(production.resources(original_id)?[0].id())?;
    assert!(is_recorded_locator(&original_path, recorded[0].uri())?);
    print_file_fingerprint(&original_path)?;
    observe_changed_original(&mut production, original_id, &original_path)?;
    assert_eq!(production_sequence(&production)?, before + 1);
    let observed = fingerprint_file(&original_path)?;
    assert_eq!(
        production.resources(original_id)?[0].fingerprints(),
        [observed.fingerprint().clone()]
    );
    let events: Vec<RevisionEventKind> = production
        .changes_since(before, 10)?
        .iter()
        .flat_map(|revision| {
            production
                .events_for_revision(revision.id())
                .expect("events")
        })
        .map(|event| event.kind().clone())
        .collect();
    assert!(matches!(
        events[0],
        RevisionEventKind::ResourceFingerprintObserved { .. }
    ));
    assert!(matches!(
        events[1],
        RevisionEventKind::RepresentationFingerprintObserved { representation_id, .. }
            if representation_id == original_id
    ));

    let resolutions = print_resolution_issues(&production, asset_id)?;
    let sequence_resolution = resolutions
        .iter()
        .find(|resolution| resolution.representation_id() == sequence_id)
        .expect("sequence resolution");
    assert_eq!(
        sequence_resolution.availability(),
        RepresentationAvailability::Partial
    );
    assert_eq!(
        sequence_resolution.issues()[0].kind(),
        AvailabilityIssueKind::MissingFrames
    );
    assert_eq!(sequence_resolution.issues()[0].frames(), [1003]);
    assert!(
        sequence_resolution.resources()[0].candidates()[0]
            .evidence()
            .iter()
            .any(|evidence| evidence.kind() == EvidenceKind::KnownLocatorAvailable)
    );

    write(&work.join("rushes/A002.mov"), "an unregistered clip");
    let categories = scan_rushes(
        &production,
        &work.join("rushes"),
        &work.join("inventory-cache.json"),
    )?;
    assert!(categories.contains(&InventoryCategory::NewCandidate));
    assert!(work.join("inventory-cache.json").is_file());

    #[cfg(unix)]
    assert!(inspect_original(
        &mut production,
        original_id,
        &original_path,
        &fake_ffprobe(work)
    )?);

    write(&work.join("recognized/B001.mov"), "clip with sidecar");
    write(&work.join("recognized/B001.xml"), "<clip/>");
    let recognized_asset = import_with_sidecar(&mut production, &work.join("recognized/B001.mov"))?;
    let recognized = production.representations(recognized_asset)?;
    assert_eq!(
        recognized[0].content_structure().kind(),
        ContentStructureKind::Package
    );
    assert_eq!(production.resources(recognized[0].id())?.len(), 2);

    cycle_media_root(&mut production, "archive")?;
    let roots: Vec<&str> = production
        .production()
        .media_roots()
        .iter()
        .map(MediaRoot::name)
        .collect();
    assert_eq!(roots, ["rushes"]);

    let archived = work.join("archive/A001.mov");
    fs::create_dir_all(work.join("archive")).expect("archive directory");
    fs::rename(&original_path, &archived).expect("move original");
    let found = find_nearby(
        &production,
        &[asset_id],
        &work.join("archive"),
        CancellationToken::new(),
    )?;
    assert_eq!(found, Some(canonical_file_uri(&archived)?));
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let error = find_nearby(&production, &[asset_id], &work.join("archive"), cancelled)
        .expect_err("cancelled resolution fails");
    assert_eq!(error.kind(), postproject_core::ErrorKind::Cancelled);
    let original_resource = production.resources(original_id)?[0].id();
    move_to_archive(&mut production, original_resource, &archived)?;
    let locators = production.locators(original_resource)?;
    assert_eq!(locators.len(), 1);
    assert_eq!(locators[0].uri(), canonical_file_uri(&archived)?);

    // The strip's frames are graded and renamed into another directory.
    let graded = work.join("graded");
    fs::create_dir_all(&graded).expect("graded directory");
    for frame in [1001, 1002, 1004] {
        fs::rename(
            work.join(format!("renders/shot010/shot010.{frame}.exr")),
            graded.join(format!("shot010-graded_{frame}.exr")),
        )
        .expect("rename frame");
    }
    let naming = relink_renamed_sequence(&mut production, strip_id, &graded)?;
    assert_eq!(
        naming,
        Some(SequenceNaming::new("shot010-graded_", ".exr", 4)?)
    );
    let strip_resource = production.resources(strip[0].id())?[0].id();
    let namings: Vec<_> = production
        .locators(strip_resource)?
        .iter()
        .filter_map(|locator| {
            locator
                .sequence_naming()
                .map(SequenceNaming::prefix)
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(namings.len(), 2);
    assert!(namings.contains(&"shot010.".to_owned()));
    assert!(namings.contains(&"shot010-graded_".to_owned()));
    Ok(())
}
