//! End-to-end 0.2 release coverage for compound representations on one logical asset.

use std::{
    fs,
    path::{Path, PathBuf},
};

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ExternalIdentifier,
    FrameRange, IdentifierScheme, MediaRoot, MetadataField, MetadataProperty, MetadataValue,
    ObjectRef, OriginIdentity, OriginalMediaImport, PropertyId, RationalRate,
    RepresentationAvailability, RepresentationImport, RepresentationResolution, Resource,
    ResourceResolutionState, ResourceRole, RevisionContext, RevisionEventKind, SequenceNaming,
    ToolIdentity, VocabularyId,
};
use postproject_media::{
    FileResourceSource, ImageSequenceSource, MediaResolver, MediaSource, prepare_confirmed_locator,
    prepare_media_root, prepare_original_media, prepare_representation,
};
use postproject_storage_sqlite::SqliteProduction;

struct Fixture {
    original: OriginalMediaImport,
    sequence: RepresentationImport,
    ordered: RepresentationImport,
    proxy: RepresentationImport,
    activity: Activity,
    media_root: MediaRoot,
}

fn iptc_property(name: &str) -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("http://iptc.org/std/videometadatahub/1.0").expect("valid vocabulary"),
        PropertyId::new(name).expect("valid property"),
    )
}

fn material_umid() -> ExternalIdentifier {
    ExternalIdentifier::new(
        IdentifierScheme::new("urn:smpte:umid").expect("valid scheme"),
        "060A2B340101010501010D4313000000A1B2C3D4E5F60718293A4B5C6D7E8F90",
        Some("material".to_owned()),
    )
    .expect("valid UMID fixture")
}

fn ebucore_property(name: &str) -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:ebu:metadata-schema:ebucore").expect("valid vocabulary"),
        PropertyId::new(name).expect("valid property"),
    )
}

fn technical_attributes() -> MetadataValue {
    MetadataValue::structure(vec![
        MetadataField::new(
            PropertyId::new("formatLabel").expect("valid field"),
            MetadataValue::string("camera original").expect("valid format label"),
        ),
        MetadataField::new(
            PropertyId::new("bitDepth").expect("valid field"),
            MetadataValue::u64(10),
        ),
    ])
    .expect("valid technical metadata")
}

#[test]
fn one_asset_round_trips_compound_media_and_proxy_provenance() {
    let directory = tempfile::tempdir().expect("create fixture directory");
    let production_path = directory.path().join("production.pproj");
    let fixture = prepare_fixture(directory.path());
    persist_fixture(&production_path, &fixture);
    let relocated = relocate_media(directory.path());
    fs::remove_file(relocated.join("plates/shot010.1003.exr")).expect("remove a declared frame");
    assert_reopened(&production_path, &fixture, &relocated);
}

fn relocate_media(root: &Path) -> PathBuf {
    let relocated = root.join("relocated");
    fs::create_dir(&relocated).expect("create relocation directory");
    for directory in ["originals", "plates", "spans"] {
        fs::rename(root.join(directory), relocated.join(directory))
            .expect("relocate media directory");
    }
    relocated
}

fn prepare_fixture(root: &Path) -> Fixture {
    let original_directory = root.join("originals");
    let proxy_directory = root.join("proxies");
    fs::create_dir(&original_directory).expect("create original directory");
    fs::create_dir(&proxy_directory).expect("create proxy directory");
    let source_path = original_directory.join("source.mov");
    let proxy_path = proxy_directory.join("proxy.mp4");
    fs::write(&source_path, b"camera original").expect("write original");
    fs::write(&proxy_path, b"editorial proxy").expect("write proxy");
    let original = prepare_original_media(
        &source_path,
        Some("Shot 010".to_owned()),
        Some("camera ingest".to_owned()),
    )
    .expect("prepare original");
    let media_root = prepare_media_root(&original_directory, Some("Originals".to_owned()), 0)
        .expect("prepare original media root");
    let asset_id = original.asset().id();
    let original_id = original.representation().id();

    let sequence_directory = root.join("plates");
    fs::create_dir(&sequence_directory).expect("create sequence directory");
    for frame in [1001, 1003, 1004] {
        fs::write(
            sequence_directory.join(format!("shot010.{frame:04}.exr")),
            format!("frame {frame}"),
        )
        .expect("write sequence frame");
    }
    let sequence = prepare_representation(
        asset_id,
        postproject_core::RepresentationKind::Derived,
        ImageSequenceSource::new(
            &sequence_directory,
            SequenceNaming::new("shot010.", ".exr", 4).expect("valid sequence pattern"),
            FrameRange::new(1001, 1004, 1).expect("valid frame range"),
            RationalRate::new(24_000, 1_001).expect("valid rate"),
            vec![1002],
        ),
    )
    .expect("prepare sequence");

    let span_directory = root.join("spans");
    fs::create_dir(&span_directory).expect("create span directory");
    let part_paths = [
        span_directory.join("span-1.mxf"),
        span_directory.join("span-2.mxf"),
    ];
    fs::write(&part_paths[0], b"span one").expect("write first span");
    fs::write(&part_paths[1], b"span two").expect("write second span");
    let part_role = ResourceRole::new("org.postproject:essence-part").expect("valid role");
    let part_sources = part_paths
        .iter()
        .map(|path| FileResourceSource::new(path, part_role.clone(), true))
        .collect::<Vec<_>>();
    let ordered = prepare_representation(
        asset_id,
        postproject_core::RepresentationKind::Original,
        MediaSource::OrderedParts(part_sources),
    )
    .expect("prepare ordered parts");

    let proxy = prepare_representation(
        asset_id,
        postproject_core::RepresentationKind::Proxy,
        &proxy_path,
    )
    .expect("prepare proxy");
    let proxy_id = proxy.representation().id();
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("org.postproject:transcode").expect("valid activity kind"),
        vec![ActivityInput::new(original_id, None)],
        vec![ActivityOutput::new(proxy_id, None)],
    )
    .expect("prepare activity")
    .with_tool(
        ToolIdentity::new(
            "FFmpeg",
            Some("8.0".to_owned()),
            Some("https://ffmpeg.org/".to_owned()),
        )
        .expect("prepare tool identity"),
    );

    Fixture {
        original,
        sequence,
        ordered,
        proxy,
        activity,
        media_root,
    }
}

fn persist_fixture(production_path: &Path, fixture: &Fixture) {
    let mut production =
        SqliteProduction::create(production_path, None).expect("create production");
    let mut transaction = production.begin_transaction().expect("begin workflow");
    transaction
        .set_revision_context(
            RevisionContext::new(
                Some(
                    OriginIdentity::new("Acceptance workflow", Some("1".to_owned()), None)
                        .expect("valid origin"),
                ),
                Some("Import source and derived media".to_owned()),
            )
            .expect("valid revision context"),
        )
        .expect("stage revision context");
    transaction
        .import_original(&fixture.original)
        .expect("stage original");
    transaction
        .add_media_root(fixture.media_root.clone())
        .expect("stage original media root");
    {
        transaction
            .add_representation(&fixture.sequence)
            .expect("stage sequence");
        transaction
            .add_representation(&fixture.ordered)
            .expect("stage ordered parts");
        transaction
            .add_representation(&fixture.proxy)
            .expect("stage proxy");
        transaction
            .create_activity(&fixture.activity)
            .expect("stage provenance");
        let asset = ObjectRef::Asset(fixture.original.asset().id());
        transaction
            .add_external_identifier(asset, &material_umid())
            .expect("stage material identifier");
        let keywords = iptc_property("keywords");
        for value in ["interview", "night"] {
            transaction
                .add_metadata_value(
                    asset,
                    &keywords,
                    &MetadataValue::string(value).expect("valid keyword"),
                )
                .expect("stage keyword");
        }
        transaction
            .add_metadata_value(
                ObjectRef::Representation(fixture.original.representation().id()),
                &ebucore_property("title"),
                &MetadataValue::language_string("Camera A", "en").expect("valid title"),
            )
            .expect("stage representation title");
        transaction
            .add_metadata_value(
                ObjectRef::Resource(fixture.original.resources()[0].id()),
                &ebucore_property("technicalAttribute"),
                &technical_attributes(),
            )
            .expect("stage resource metadata");
        transaction
            .add_metadata_value(
                ObjectRef::Activity(fixture.activity.id()),
                &ebucore_property("processingParameters"),
                &MetadataValue::structure(vec![MetadataField::new(
                    PropertyId::new("preset").expect("valid field"),
                    MetadataValue::string("editorial-proxy").expect("valid preset"),
                )])
                .expect("valid activity parameters"),
            )
            .expect("stage activity parameters");
    }
    transaction.commit().expect("commit workflow");
    drop(transaction);
    assert_eq!(
        production
            .latest_revision()
            .expect("load workflow revision")
            .expect("workflow revision")
            .sequence(),
        1
    );
}

fn assert_reopened(production_path: &Path, fixture: &Fixture, relocated: &Path) {
    let mut reopened = SqliteProduction::open(production_path).expect("reopen production");
    let asset_id = fixture.original.asset().id();
    let original_id = fixture.original.representation().id();
    let sequence_id = fixture.sequence.representation().id();
    let ordered_id = fixture.ordered.representation().id();
    let proxy_id = fixture.proxy.representation().id();
    let representations = reopened
        .representations(asset_id)
        .expect("load representations");
    assert_eq!(representations.len(), 4);
    assert!(representations.iter().any(|item| item.id() == ordered_id));
    assert_eq!(
        reopened.ancestors(proxy_id).expect("load ancestry"),
        [original_id]
    );
    let producing = reopened
        .activities_producing(proxy_id)
        .expect("load producing activity");
    assert_eq!(producing.len(), 1);
    assert_eq!(producing[0].id(), fixture.activity.id());
    assert!(producing[0].inputs()[0].snapshot().is_some());
    assert!(producing[0].outputs()[0].snapshot().is_some());
    assert_eq!(
        fixture.activity.tool().expect("activity tool").version(),
        Some("8.0")
    );
    assert_revision_feed(&reopened, fixture);
    relink_moved_media(&mut reopened, fixture, relocated);
    assert_relink_revision(&reopened);
    assert_original_online(&reopened, fixture);
    assert_identifiers_and_metadata(&reopened, fixture);

    let stored_sequence = representations
        .iter()
        .find(|item| item.id() == sequence_id)
        .expect("find sequence");
    assert_eq!(stored_sequence.fingerprints().len(), 1);
    let resources = reopened
        .resources(sequence_id)
        .expect("load sequence resource");
    assert_eq!(resources[0].fingerprints().len(), 1);
    let locators = reopened
        .locators(resources[0].id())
        .expect("load sequence locator");
    let resource_resolution = MediaResolver::default()
        .resolve_resource(
            &resources[0],
            stored_sequence.content_structure(),
            &locators,
            &[],
            &[],
        )
        .expect("resolve sequence");
    let resolution = RepresentationResolution::aggregate(
        sequence_id,
        stored_sequence.content_structure(),
        vec![resource_resolution],
    )
    .expect("aggregate sequence availability");
    assert_eq!(
        resolution.availability(),
        RepresentationAvailability::Partial
    );
    assert_eq!(resolution.issues()[0].frames(), &[1002, 1003]);
    assert_eq!(
        reopened
            .resources(ordered_id)
            .expect("load ordered resources")
            .iter()
            .map(Resource::id)
            .collect::<Vec<_>>(),
        fixture
            .ordered
            .resources()
            .iter()
            .map(Resource::id)
            .collect::<Vec<_>>()
    );
}

fn assert_relink_revision(production: &SqliteProduction) {
    let changes = production
        .changes_since(0, 10)
        .expect("load all workflow changes");
    assert_eq!(
        changes
            .iter()
            .map(postproject_core::Revision::sequence)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let events = production
        .events_for_revision(changes[1].id())
        .expect("load relink events");
    assert_eq!(events.len(), 10);
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind(), RevisionEventKind::MediaRootRemoved { .. }))
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind(), RevisionEventKind::LocatorRetired { .. }))
            .count(),
        4
    );
}

fn assert_original_online(production: &SqliteProduction, fixture: &Fixture) {
    let representation = fixture.original.representation();
    let resource = production
        .resources(representation.id())
        .expect("load original resource")
        .remove(0);
    let resolution = MediaResolver::default()
        .resolve_resource(
            &resource,
            representation.content_structure(),
            &production
                .locators(resource.id())
                .expect("load replacement locator"),
            production.production().media_roots(),
            &[],
        )
        .expect("resolve relinked original");
    assert_eq!(
        resolution.state(),
        ResourceResolutionState::OnlineAtKnownLocator
    );
}

fn relink_moved_media(production: &mut SqliteProduction, fixture: &Fixture, relocated: &Path) {
    let original_id = fixture.original.representation().id();
    let original_resource = production
        .resources(original_id)
        .expect("load original resource")
        .remove(0);
    let original_locators = production
        .locators(original_resource.id())
        .expect("load original locators");
    let replacement_root = prepare_media_root(
        relocated.join("originals"),
        Some("Relocated originals".to_owned()),
        0,
    )
    .expect("prepare replacement root");
    let discovered = MediaResolver::default()
        .resolve_resource(
            &original_resource,
            fixture.original.representation().content_structure(),
            &original_locators,
            std::slice::from_ref(&replacement_root),
            &[],
        )
        .expect("discover relocated original");
    assert_eq!(discovered.state(), ResourceResolutionState::ResolvedExact);

    let mut replacements = vec![
        prepare_confirmed_locator(
            original_resource.id(),
            discovered.candidates()[0].uri(),
            None,
            None,
        )
        .expect("prepare original locator"),
    ];
    let sequence_resource = fixture.sequence.resources()[0].id();
    replacements.push(
        prepare_confirmed_locator(
            sequence_resource,
            postproject_media::canonical_file_uri(relocated.join("plates")).expect("sequence URI"),
            None,
            Some(SequenceNaming::new("shot010.", ".exr", 4).expect("valid naming")),
        )
        .expect("prepare sequence locator"),
    );
    for (index, resource) in fixture.ordered.resources().iter().enumerate() {
        replacements.push(
            prepare_confirmed_locator(
                resource.id(),
                postproject_media::canonical_file_uri(
                    relocated
                        .join("spans")
                        .join(format!("span-{}.mxf", index + 1)),
                )
                .expect("span URI"),
                None,
                None,
            )
            .expect("prepare span locator"),
        );
    }

    let mut retired = original_locators;
    for representation in [&fixture.sequence, &fixture.ordered] {
        for resource in representation.resources() {
            retired.extend(
                production
                    .locators(resource.id())
                    .expect("load compound locators"),
            );
        }
    }
    let mut transaction = production
        .begin_transaction()
        .expect("begin relink transaction");
    transaction
        .remove_media_root(fixture.media_root.id())
        .expect("remove old root");
    transaction
        .add_media_root(replacement_root)
        .expect("add replacement root");
    for locator in &replacements {
        transaction
            .add_locator(locator)
            .expect("add replacement locator");
    }
    for locator in retired {
        transaction
            .retire_locator(locator.id())
            .expect("retire old locator");
    }
    transaction.commit().expect("commit relink");
}

fn assert_revision_feed(production: &SqliteProduction, fixture: &Fixture) {
    let changes = production
        .changes_since(0, 10)
        .expect("load workflow changes");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].sequence(), 1);
    assert_eq!(
        changes[0].origin().expect("workflow origin").name(),
        "Acceptance workflow"
    );
    let events = production
        .events_for_revision(changes[0].id())
        .expect("load workflow events");
    assert_eq!(events.len(), 30);
    assert!(events.iter().any(|event| matches!(
        event.kind(),
        RevisionEventKind::ActivityCreated { activity_id, .. }
            if *activity_id == fixture.activity.id()
    )));
    assert!(events.iter().enumerate().all(|(position, event)| {
        event.position() == u32::try_from(position).expect("event position fits u32")
    }));
}

fn assert_identifiers_and_metadata(production: &SqliteProduction, fixture: &Fixture) {
    let asset = ObjectRef::Asset(fixture.original.asset().id());
    assert_eq!(
        production
            .external_identifiers(asset)
            .expect("load material identifier"),
        [material_umid()]
    );
    assert_eq!(
        production
            .metadata_values(asset, &iptc_property("keywords"))
            .expect("load keywords"),
        [
            MetadataValue::string("interview").expect("valid keyword"),
            MetadataValue::string("night").expect("valid keyword"),
        ]
    );
    assert_eq!(
        production
            .metadata_values(
                ObjectRef::Representation(fixture.original.representation().id()),
                &ebucore_property("title"),
            )
            .expect("load representation title"),
        [MetadataValue::language_string("Camera A", "en").expect("valid title")]
    );
    assert_eq!(
        production
            .metadata_values(
                ObjectRef::Resource(fixture.original.resources()[0].id()),
                &ebucore_property("technicalAttribute"),
            )
            .expect("load resource metadata"),
        [technical_attributes()]
    );
    assert_eq!(
        production
            .metadata_values(
                ObjectRef::Activity(fixture.activity.id()),
                &ebucore_property("processingParameters"),
            )
            .expect("load activity parameters")
            .len(),
        1
    );
}
