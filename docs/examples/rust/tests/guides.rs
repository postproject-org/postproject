//! Runs every Rust listing included in the `PostProject` integrator guides.
//!
//! Each `// [name]` ... `// [/name]` region is included verbatim by the
//! documentation build, so keep regions self-contained and readable.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityOutputQuery,
    ActivityRole, ArtifactEvaluationLimits, AssetId, Dependency, DependencyKind,
    DependencyQueryLimits, DependencyTarget, Error, ErrorKind, ExternalIdentifier, FrameRange,
    HostObjectBinding, IdentifierScheme, Job, JobClaimId, JobId, JobKind, JobQuery, JobStateKind,
    MediaRoot, MediaRootId, MetadataMatch, MetadataProperty, MetadataQuery, MetadataValue,
    ObjectRef, OriginIdentity, ProductionId, PropertyId, ProvenanceQueryLimits, QueryPageRequest,
    RationalRate, Representation, RepresentationId, RepresentationKind, RepresentationResolution,
    RequestedJobOutput, Result, Revision, RevisionContext, RevisionEvent, RevisionEventFilter,
    RevisionEventType, RevisionId, RevisionWaitOutcome, SequenceNaming, StaleArtifactQuery,
    ToolIdentity, VocabularyId,
};
use postproject_media::{
    ExecutionOutcome, ExecutionRequest, Executor, FfmpegExecutor, GENERATE_PROXY_JOB_KIND,
    ImageSequenceSource, MediaResolver, MediaRootMapping, PROXY_720P_PROFILE,
    prepare_confirmed_locator, prepare_original_media, prepare_representation,
};
use postproject_storage_sqlite::SqliteProduction;

// [create-production]
fn create_production(path: &Path, media: &Path) -> Result<(SqliteProduction, AssetId)> {
    let mut production = SqliteProduction::create(path, Some("Documentary".to_owned()))?;

    let import = prepare_original_media(media, Some("Camera A".to_owned()), None)?;
    let asset_id = import.asset().id();
    {
        let mut transaction = production.begin_transaction()?;
        transaction.set_revision_context(RevisionContext::new(
            Some(OriginIdentity::new(
                "com.example.editor",
                Some("0.4.0".to_owned()),
                None,
            )?),
            Some("Import camera original".to_owned()),
        )?)?;
        transaction.import_original(&import)?;
        transaction.commit()?;
    }

    let representations = production.representations(asset_id)?;
    println!("representations: {}", representations.len());
    Ok((production, asset_id))
}
// [/create-production]

// [external-identifiers]
fn tag_camera_serial(production: &mut SqliteProduction, asset_id: AssetId) -> Result<()> {
    let target = ObjectRef::Asset(asset_id);
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("com.example.camera.serial")?,
        "A-0007",
        None,
    )?;
    {
        let mut transaction = production.begin_transaction()?;
        transaction.add_external_identifier(target, &identifier)?;
        transaction.commit()?;
    }

    let attached = production.external_identifiers(target)?;
    let matches = production.find_by_external_identifier(
        identifier.scheme(),
        identifier.value(),
        identifier.qualifier(),
    )?;
    assert_eq!(attached, vec![identifier]);
    assert_eq!(matches, vec![target]);
    Ok(())
}
// [/external-identifiers]

// [metadata]
fn add_title(production: &mut SqliteProduction, asset_id: AssetId) -> Result<()> {
    let title = MetadataProperty::new(
        VocabularyId::new(
            "https://iptc.org/std/videometadatahub/recommendation/iptc-vmhub-1.7-schema.json",
        )?,
        PropertyId::new("title")?,
    );
    let target = ObjectRef::Asset(asset_id);
    {
        let mut transaction = production.begin_transaction()?;
        transaction.add_metadata_value(
            target,
            &title,
            &MetadataValue::language_string("Interview", "en-US")?,
        )?;
        transaction.commit()?;
    }

    let on_asset = production.metadata(target)?;
    let everywhere = production.query_by_metadata_property(&title)?;
    assert_eq!(on_asset.len(), 1);
    assert_eq!(everywhere.len(), 1);
    Ok(())
}
// [/metadata]

// [media-root]
fn add_rushes_root(production: &mut SqliteProduction) -> Result<()> {
    let root = MediaRoot::new(
        MediaRootId::new(),
        "rushes",
        Some("Camera originals".to_owned()),
        None,
        0,
        true,
    )?;
    let mut transaction = production.begin_transaction()?;
    transaction.add_media_root(root)?;
    transaction.commit()
}
// [/media-root]

// [resolve-asset]
fn resolve_asset(
    production: &SqliteProduction,
    asset_id: AssetId,
    rushes_directory: &Path,
) -> Result<Vec<RepresentationResolution>> {
    let mappings = [MediaRootMapping::new("rushes", rushes_directory)?];
    let roots = production.media_roots()?;
    let resolver = MediaResolver::default();

    let mut resolutions = Vec::new();
    for representation in production.representations(asset_id)? {
        let mut resources = Vec::new();
        for resource in production.resources(representation.id())? {
            let locators = production.locators(resource.id())?;
            resources.push(resolver.resolve_resource(
                &resource,
                representation.content_structure(),
                &locators,
                &roots,
                &mappings,
            )?);
        }
        let resolution = RepresentationResolution::aggregate(
            representation.id(),
            representation.content_structure(),
            resources,
        )?;
        println!("availability: {:?}", resolution.availability());
        for resource in resolution.resources() {
            for candidate in resource.candidates() {
                println!("candidate: {}", candidate.uri());
            }
        }
        resolutions.push(resolution);
    }
    Ok(resolutions)
}
// [/resolve-asset]

// [confirm-locator]
fn confirm_unique_candidates(
    production: &mut SqliteProduction,
    resolutions: &[RepresentationResolution],
) -> Result<()> {
    let mut transaction = production.begin_transaction()?;
    for resolution in resolutions {
        for resource in resolution.resources() {
            // Several candidates need a person to choose; never pick one here.
            let [candidate] = resource.candidates() else {
                continue;
            };
            // Record the logical root the candidate was found under and, for
            // an image sequence, the naming of its files there.
            let locator = prepare_confirmed_locator(
                resource.resource_id(),
                candidate.uri(),
                candidate.media_root(),
                candidate.sequence_naming().cloned(),
            )?;
            transaction.add_locator(&locator)?;
        }
    }
    transaction.commit()
}
// [/confirm-locator]

// [image-sequence]
fn add_render_sequence(
    production: &mut SqliteProduction,
    asset_id: AssetId,
    directory: &Path,
) -> Result<RepresentationId> {
    let source = ImageSequenceSource::new(
        directory,
        SequenceNaming::new("shot010.", ".exr", 4)?,
        FrameRange::new(1001, 1004, 1)?,
        RationalRate::new(24000, 1001)?,
        vec![1003],
    );
    let import = prepare_representation(asset_id, RepresentationKind::Derived, source)?;
    let sequence_id = import.representation().id();
    {
        let mut transaction = production.begin_transaction()?;
        transaction.add_representation(&import)?;
        transaction.commit()?;
    }

    let stored = production
        .representations(asset_id)?
        .into_iter()
        .find(|representation| representation.id() == sequence_id)
        .expect("committed representation");
    println!("structure: {:?}", stored.content_structure().kind());
    Ok(sequence_id)
}
// [/image-sequence]

// [provenance]
fn record_render(
    production: &mut SqliteProduction,
    source_id: RepresentationId,
    render_id: RepresentationId,
) -> Result<()> {
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("org.postproject:render")?,
        vec![ActivityInput::new(
            source_id,
            Some(ActivityRole::new("org.postproject:primary")?),
        )],
        vec![ActivityOutput::new(render_id, None)],
    )?
    .with_tool(ToolIdentity::new(
        "Example Renderer",
        Some("2.1".to_owned()),
        Some("https://example.com/renderer".to_owned()),
    )?);
    {
        let mut transaction = production.begin_transaction()?;
        transaction.create_activity(&activity)?;
        transaction.commit()?;
    }

    let consuming = production.activities_consuming(source_id)?;
    let producing = production.activities_producing(render_id)?;
    assert_eq!(consuming[0].id(), activity.id());
    assert_eq!(producing[0].id(), activity.id());
    assert!(producing[0].inputs()[0].snapshot().is_some());
    assert_eq!(production.ancestors(render_id)?, vec![source_id]);
    assert_eq!(production.descendants(source_id)?, vec![render_id]);
    Ok(())
}
// [/provenance]

// [artifact-knowledge]
fn inspect_artifact(production: &SqliteProduction, artifact_id: RepresentationId) -> Result<()> {
    let evaluation =
        production.evaluate_artifact(artifact_id, ArtifactEvaluationLimits::new(64, 1_000)?)?;
    println!("artifact state: {:?}", evaluation.state());
    for reason in evaluation.reasons() {
        println!("reason: {reason:?}");
    }

    let reproducibility = production.artifact_reproducibility(artifact_id)?;
    println!(
        "reproducible: {}, missing conditions: {}",
        reproducibility.is_reproducible(),
        reproducibility.issues().len()
    );
    Ok(())
}
// [/artifact-knowledge]

// [dependency-queries]
fn record_and_query_dependencies(
    production: &mut SqliteProduction,
    source_id: RepresentationId,
    target_asset_id: AssetId,
    resolved_id: RepresentationId,
) -> Result<()> {
    let dependency = Dependency::new(
        None,
        DependencyKind::new("org.example:character-reference")?,
        DependencyTarget::Asset(target_asset_id),
        Some(resolved_id),
        true,
        "characters/lead.usd",
    )?;
    {
        let base = production.read_session()?.decision_base();
        let mut transaction = production.begin_edit(base)?;
        transaction.record_dependency_set(source_id, &[dependency])?;
        transaction.commit()?;
    }

    let limits = DependencyQueryLimits::new(4, 1_000)?;
    let request = QueryPageRequest::new(100, None)?;
    let dependencies = production.dependencies(source_id, limits, &request)?;
    for item in dependencies.items() {
        println!("dependency {:?} at depth {}", item.target(), item.depth());
    }
    assert!(!dependencies.traversal_truncated());

    let dependents =
        production.dependents(DependencyTarget::Asset(target_asset_id), limits, &request)?;
    assert_eq!(
        dependents.items()[0].target(),
        DependencyTarget::Representation(source_id)
    );
    Ok(())
}
// [/dependency-queries]

// [job-query-pages]
fn request_and_page_jobs(
    production: &mut SqliteProduction,
    input_id: RepresentationId,
    output_asset_id: AssetId,
) -> Result<()> {
    let kind = JobKind::new("org.example:generate-proxy")?;
    let output = RequestedJobOutput::new(output_asset_id, RepresentationKind::Proxy, None)?;
    let requested = [
        Job::new(JobId::new(), kind.clone(), vec![input_id], output.clone())?,
        Job::new(JobId::new(), kind.clone(), vec![input_id], output)?,
    ];
    {
        let mut transaction = production.begin_transaction()?;
        for job in &requested {
            transaction.request_job(job)?;
        }
        transaction.commit()?;
    }
    assert_eq!(production.job(requested[0].id())?, requested[0]);

    let query = JobQuery::new(Some(JobStateKind::Requested), Some(kind));
    let mut cursor = None;
    let mut count = 0;
    loop {
        let page = production.jobs(&query, &QueryPageRequest::new(1, cursor)?)?;
        count += page.items().len();
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(count, 2);
    Ok(())
}
// [/job-query-pages]

// [media-structure-pages]
fn print_recorded_locators(production: &SqliteProduction) -> Result<()> {
    // Follow each nested next_cursor the same way in large productions.
    let first_page = || QueryPageRequest::new(100, None);
    let mut cursor = None;
    loop {
        let assets = production.assets_page(&QueryPageRequest::new(100, cursor)?)?;
        for asset in assets.items() {
            for representation in production
                .representations_page(asset.id(), &first_page()?)?
                .items()
            {
                for resource in production
                    .resources_page(representation.id(), &first_page()?)?
                    .items()
                {
                    for locator in production
                        .locators_page(resource.id(), &first_page()?)?
                        .items()
                    {
                        let root = locator.media_root().unwrap_or("-");
                        println!("{} (root: {root})", locator.uri());
                    }
                }
            }
        }
        cursor = assets.next_cursor().cloned();
        if cursor.is_none() {
            return Ok(());
        }
    }
}
// [/media-structure-pages]

// [knowledge-only-media]
fn list_media_knowledge(production: &SqliteProduction) -> Result<Vec<RepresentationId>> {
    // Both queries read recorded knowledge; neither touches the filesystem.
    let request = QueryPageRequest::new(100, None)?;
    for representation_id in production.unresolved_media(&request)?.items() {
        println!("no recorded locator: {representation_id}");
    }

    let under_rushes = production.representations_under_media_root("rushes", &request)?;
    Ok(under_rushes
        .items()
        .iter()
        .map(Representation::id)
        .collect())
}
// [/knowledge-only-media]

// [point-reads]
fn read_known_objects(
    production: &SqliteProduction,
    asset_id: AssetId,
    representation_id: RepresentationId,
) -> Result<()> {
    // A host reference names one object; read it without scanning the production.
    let asset = production.asset(asset_id)?;
    let representation = production.representation(representation_id)?;
    println!(
        "{}: {:?}",
        asset.display_name().unwrap_or("unnamed"),
        representation.kind()
    );
    let first_resource =
        production.resources_page(representation_id, &QueryPageRequest::new(1, None)?)?;
    let users = production.representations_using_resource(
        first_resource.items()[0].id(),
        &QueryPageRequest::new(100, None)?,
    )?;
    assert!(
        users
            .items()
            .iter()
            .any(|item| item.id() == representation_id)
    );
    Ok(())
}
// [/point-reads]

// [metadata-query-pages]
fn find_interview_titles(production: &SqliteProduction) -> Result<Vec<ObjectRef>> {
    let title = MetadataProperty::new(
        VocabularyId::new(
            "https://iptc.org/std/videometadatahub/recommendation/iptc-vmhub-1.7-schema.json",
        )?,
        PropertyId::new("title")?,
    );
    let query = MetadataQuery::new(
        title,
        Some(MetadataValue::language_string("Interview", "en-US")?),
    )?;
    let page = production.metadata_query(&query, &QueryPageRequest::new(100, None)?)?;
    for matched in page.items() {
        println!("{:?}: {:?}", matched.target(), matched.assertion());
    }
    Ok(page.items().iter().map(MetadataMatch::target).collect())
}
// [/metadata-query-pages]

// [provenance-query-pages]
fn query_render_lineage(
    production: &SqliteProduction,
    source_id: RepresentationId,
    render_id: RepresentationId,
) -> Result<()> {
    let request = QueryPageRequest::new(100, None)?;
    let producing = production.activities_producing_page(render_id, &request)?;
    let consuming = production.activities_consuming_page(source_id, &request)?;
    assert_eq!(producing.items(), consuming.items());

    let by_kind = production.activity_outputs(
        &ActivityOutputQuery::Kind(ActivityKind::new("org.postproject:render")?),
        &request,
    )?;
    let by_tool = production.activity_outputs(
        &ActivityOutputQuery::Tool(ToolIdentity::new(
            "Example Renderer",
            Some("2.1".to_owned()),
            Some("https://example.com/renderer".to_owned()),
        )?),
        &request,
    )?;
    assert_eq!(by_kind.items(), [render_id]);
    assert_eq!(by_tool.items(), [render_id]);

    let limits = ProvenanceQueryLimits::new(8, 1_000)?;
    let ancestors = production.ancestors_page(render_id, limits, &request)?;
    for item in ancestors.items() {
        println!(
            "ancestor {} at depth {}",
            item.representation_id(),
            item.depth()
        );
    }
    assert!(!ancestors.traversal_truncated());

    let descendants = production.descendants_page(source_id, limits, &request)?;
    assert_eq!(descendants.items()[0].representation_id(), render_id);
    Ok(())
}
// [/provenance-query-pages]

// [stale-artifact-pages]
fn stale_descendants(
    production: &SqliteProduction,
    source_id: RepresentationId,
) -> Result<Vec<RepresentationId>> {
    let query = StaleArtifactQuery::new(Some(source_id), ArtifactEvaluationLimits::new(64, 1_000)?);
    let mut stale = Vec::new();
    let mut cursor = None;
    loop {
        let page = production.stale_artifacts(query, &QueryPageRequest::new(100, cursor)?)?;
        // A page bounds the candidates examined, so it may hold fewer stale
        // results, or none, and still carry a continuation.
        stale.extend_from_slice(page.items());
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            return Ok(stale);
        }
    }
}
// [/stale-artifact-pages]

// [changed-objects]
fn objects_changed_after(production: &SqliteProduction, sequence: u64) -> Result<Vec<ObjectRef>> {
    let mut changed = Vec::new();
    let mut cursor = None;
    loop {
        let page =
            production.objects_changed_since(sequence, &QueryPageRequest::new(100, cursor)?)?;
        changed.extend_from_slice(page.items());
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            return Ok(changed);
        }
    }
}
// [/changed-objects]

// [reference-executor]
fn execute_proxy(ffmpeg: &Path, input: &Path, target_root: &Path) -> Result<PathBuf> {
    let request = ExecutionRequest::new(
        JobId::new(),
        JobClaimId::new(),
        JobKind::new(GENERATE_PROXY_JOB_KIND)?,
        PROXY_720P_PROFILE,
        input,
        target_root,
    )?;
    let mut renew_claim = || Ok(());
    match FfmpegExecutor::with_executable(ffmpeg).execute(&request, &mut renew_claim)? {
        ExecutionOutcome::Completed { output, .. } => Ok(output),
        outcome => Err(Error::new(
            ErrorKind::Io,
            format!("reference executor did not complete: {outcome:?}"),
        )),
    }
}
// [/reference-executor]

fn handle_event(event: &RevisionEvent) {
    println!("event {}: {:?}", event.position(), event.kind());
}

// [revision-feed]
fn process_changes(production: &SqliteProduction, mut cursor: u64) -> Result<u64> {
    loop {
        let page = production.changes_since(cursor, 100)?;
        for revision in &page {
            for event in production.events_for_revision(revision.id())? {
                handle_event(&event);
            }
            // Persist the cursor only after the whole revision is processed.
            cursor = revision.sequence();
        }
        if page.len() < 100 {
            return Ok(cursor);
        }
    }
}
// [/revision-feed]

// [revision-filter]
fn new_media_revisions(
    production: &SqliteProduction,
    cursor: u64,
) -> Result<(Vec<RevisionId>, u64)> {
    let filter = RevisionEventFilter::new([
        RevisionEventType::RepresentationAdded,
        RevisionEventType::JobSucceeded,
    ])?;
    let page = production.changes_since_filtered(cursor, &filter, 100)?;
    let revisions = page.revisions().iter().map(Revision::id).collect();
    // Continue from the through sequence, which skips unrelated revisions.
    Ok((revisions, page.through_sequence()))
}
// [/revision-filter]

// [revision-wait]
fn wait_for_changes(production: &SqliteProduction, cursor: u64) -> Result<Vec<Revision>> {
    let mut waiter = production.revision_waiter()?;
    // waiter.canceller() may be handed to another thread to stop the wait.
    match waiter.wait_for_revisions(cursor, 100, Duration::from_secs(5))? {
        RevisionWaitOutcome::Revisions(revisions) => Ok(revisions),
        // Timed out, the production was closed, or the waiter was cancelled.
        _ => Ok(Vec::new()),
    }
}
// [/revision-wait]

// [host-binding]
fn bind_representation(
    production_id: ProductionId,
    representation_id: RepresentationId,
) -> Result<String> {
    let binding =
        HostObjectBinding::new(production_id, ObjectRef::Representation(representation_id))?;
    let stored = binding.to_string();

    let reopened = HostObjectBinding::from_str(&stored)?;
    assert_eq!(reopened, binding);
    Ok(stored)
}
// [/host-binding]

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/fixtures/sample-media.dat")
}

#[cfg(unix)]
fn fake_ffmpeg(directory: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.join("ffmpeg-fake");
    fs::write(
        &path,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo 'ffmpeg version guide-fake'; exit 0; fi\nfor last do :; done\nprintf proxy > \"$last\"\n",
    )
    .expect("fake ffmpeg");
    let mut permissions = fs::metadata(&path).expect("fake metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("fake executable");
    path
}

#[cfg(windows)]
fn fake_ffmpeg(directory: &Path) -> PathBuf {
    let path = directory.join("ffmpeg-fake.cmd");
    fs::write(
        &path,
        "@echo off\r\nif \"%~1\"==\"-version\" (echo ffmpeg version guide-fake& exit /b 0)\r\nset \"last=\"\r\n:args\r\nif \"%~1\"==\"\" goto run\r\nset \"last=%~1\"\r\nshift\r\ngoto args\r\n:run\r\n>\"%last%\" echo proxy\r\n",
    )
    .expect("fake ffmpeg");
    path
}

#[test]
fn guide_examples_run_in_order() -> Result<()> {
    let work = tempfile::tempdir().expect("temporary directory");
    let rushes = work.path().join("rushes");
    let moved = work.path().join("moved");
    let renders = work.path().join("renders/shot010");
    for directory in [&rushes, &moved, &renders] {
        fs::create_dir_all(directory).expect("work directory");
    }
    fs::copy(fixture(), rushes.join("A001.mov")).expect("media fixture");
    for frame in [1001, 1002, 1004] {
        fs::write(
            renders.join(format!("shot010.{frame}.exr")),
            format!("frame {frame}"),
        )
        .expect("sequence frame");
    }

    let (mut production, asset_id) = create_production(
        &work.path().join("production.pproj"),
        &rushes.join("A001.mov"),
    )?;
    let original_id = production.representations(asset_id)?[0].id();
    tag_camera_serial(&mut production, asset_id)?;
    add_title(&mut production, asset_id)?;

    add_rushes_root(&mut production)?;
    fs::rename(rushes.join("A001.mov"), moved.join("A001.mov")).expect("move media");
    let resolutions = resolve_asset(&production, asset_id, &moved)?;
    assert_eq!(resolutions[0].resources()[0].candidates().len(), 1);
    confirm_unique_candidates(&mut production, &resolutions)?;

    let before_render = production.latest_revision()?.expect("revision").sequence();
    let sequence_id = add_render_sequence(&mut production, asset_id, &renders)?;
    record_render(&mut production, original_id, sequence_id)?;
    inspect_artifact(&production, sequence_id)?;
    record_and_query_dependencies(&mut production, sequence_id, asset_id, original_id)?;
    request_and_page_jobs(&mut production, original_id, asset_id)?;

    print_recorded_locators(&production)?;
    assert_eq!(list_media_knowledge(&production)?, vec![original_id]);
    read_known_objects(&production, asset_id, original_id)?;
    assert_eq!(
        find_interview_titles(&production)?,
        vec![ObjectRef::Asset(asset_id)]
    );
    query_render_lineage(&production, original_id, sequence_id)?;
    assert_eq!(stale_descendants(&production, original_id)?, []);
    assert!(
        objects_changed_after(&production, before_render)?
            .contains(&ObjectRef::Representation(sequence_id))
    );

    let proxy_root = work.path().join("proxies");
    fs::create_dir(&proxy_root).expect("proxy root");
    let proxy = execute_proxy(
        &fake_ffmpeg(work.path()),
        &moved.join("A001.mov"),
        &proxy_root,
    )?;
    assert!(proxy.is_file());

    let cursor = process_changes(&production, 0)?;
    assert_eq!(
        Some(cursor),
        production
            .latest_revision()?
            .map(|revision| revision.sequence())
    );
    let (media_revisions, through) = new_media_revisions(&production, 0)?;
    assert_ne!(media_revisions, []);
    assert_eq!(through, cursor);
    assert_ne!(wait_for_changes(&production, 0)?, []);

    let binding = bind_representation(production.production().id(), sequence_id)?;
    assert!(binding.starts_with("https://postproject.org/ref/v1/"));
    Ok(())
}
