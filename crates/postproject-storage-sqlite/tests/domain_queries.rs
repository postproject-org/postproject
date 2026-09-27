//! Bounded domain-query integration coverage.

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityOutputQuery,
    ArtifactEvaluationLimits, Asset, AssetId, ContentStructure, Locator, LocatorAvailability,
    LocatorId, MediaRoot, MediaRootId, MetadataProperty, MetadataQuery, MetadataValue, ObjectRef,
    OriginalMediaImport, PropertyId, ProvenanceQueryLimits, QueryPageRequest, Representation,
    RepresentationFingerprint, RepresentationId, RepresentationKind, Resource, ResourceFingerprint,
    ResourceId, StaleArtifactQuery, Timestamp, ToolIdentity, VocabularyId,
};
use postproject_storage_sqlite::SqliteProduction;
use tempfile::tempdir;

fn imported(label: u8, root: Option<&str>) -> OriginalMediaImport {
    let asset_id = AssetId::from_bytes([label; 16]);
    let representation_id = RepresentationId::from_bytes([label; 16]);
    let resource_id = ResourceId::from_bytes([label; 16]);
    let asset = Asset::new(
        asset_id,
        Timestamp::from_unix_micros(i64::from(label)),
        Some(format!("asset-{label}")),
        None,
    );
    let resource_fingerprint =
        ResourceFingerprint::new("test", 1, vec![label]).expect("resource fingerprint");
    let representation_fingerprint =
        RepresentationFingerprint::new("test", 1, vec![label]).expect("representation fingerprint");
    let representation = Representation::new(
        representation_id,
        asset_id,
        RepresentationKind::Original,
        ContentStructure::single_resource(resource_id),
        vec![representation_fingerprint],
    );
    let resource = Resource::new(resource_id, vec![resource_fingerprint], None);
    let locator = Locator::new(
        LocatorId::from_bytes([label; 16]),
        resource_id,
        format!("file:///media/{label}.mov"),
        None,
        LocatorAvailability::Online,
    )
    .expect("locator");
    let locator = root
        .map_or(Ok(locator.clone()), |name| locator.with_media_root(name))
        .expect("locator root");
    OriginalMediaImport::new(asset, representation, vec![resource], vec![locator]).expect("import")
}

fn page(limit: u32) -> QueryPageRequest {
    QueryPageRequest::new(limit, None).expect("page")
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end fixture verifies cursor interactions across the named query family"
)]
fn bounded_queries_cover_media_metadata_provenance_staleness_and_changes() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("queries.pproj");
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    let root =
        MediaRoot::new(MediaRootId::new(), "rushes", None, None, 0, true).expect("media root");
    let source = imported(1, Some("rushes"));
    let proxy = imported(2, Some("rushes"));
    let render = imported(3, None);
    let property = MetadataProperty::new(
        VocabularyId::new("com.example.query").expect("vocabulary"),
        PropertyId::new("tag").expect("property"),
    );
    let matching_value = MetadataValue::string("matching").expect("value");
    let transcode = Activity::new(
        ActivityId::from_bytes([10; 16]),
        ActivityKind::new("org.postproject:transcode").expect("kind"),
        vec![ActivityInput::new(source.representation().id(), None)],
        vec![ActivityOutput::new(proxy.representation().id(), None)],
    )
    .expect("activity")
    .with_tool(ToolIdentity::new("FFmpeg", Some("8".to_owned()), None).expect("tool"));
    let composite = Activity::new(
        ActivityId::from_bytes([11; 16]),
        ActivityKind::new("org.postproject:composite").expect("kind"),
        vec![ActivityInput::new(proxy.representation().id(), None)],
        vec![ActivityOutput::new(render.representation().id(), None)],
    )
    .expect("activity");
    {
        let mut transaction = production.begin_transaction().expect("begin setup");
        transaction.add_media_root(root).expect("add root");
        for import in [&source, &proxy, &render] {
            transaction.import_original(import).expect("import media");
        }
        transaction
            .add_metadata_value(
                ObjectRef::Asset(source.asset().id()),
                &property,
                &matching_value,
            )
            .expect("source metadata");
        transaction
            .add_metadata_value(
                ObjectRef::Asset(proxy.asset().id()),
                &property,
                &MetadataValue::string("other").expect("other value"),
            )
            .expect("proxy metadata");
        transaction
            .create_activity(&transcode)
            .expect("transcode activity");
        transaction
            .create_activity(&composite)
            .expect("composite activity");
        transaction.commit().expect("commit setup");
    }
    let setup_revision = production
        .latest_revision()
        .expect("latest revision")
        .expect("setup revision")
        .sequence();

    let first_assets = production.assets_page(&page(2)).expect("first assets");
    assert_eq!(first_assets.items().len(), 2);
    let second_assets = production
        .assets_page(
            &QueryPageRequest::new(2, first_assets.next_cursor().cloned()).expect("next page"),
        )
        .expect("second assets");
    assert_eq!(second_assets.items().len(), 1);
    assert_eq!(
        production
            .representations_page(source.asset().id(), &page(1))
            .expect("representations")
            .items()[0]
            .id(),
        source.representation().id()
    );
    assert_eq!(
        production
            .resources_page(source.representation().id(), &page(1))
            .expect("resources")
            .items()[0]
            .id(),
        source.resources()[0].id()
    );
    assert_eq!(
        production
            .locators_page(source.resources()[0].id(), &page(1))
            .expect("locators")
            .items()[0]
            .media_root(),
        Some("rushes")
    );
    assert_eq!(
        production
            .representations_under_media_root("rushes", &page(10))
            .expect("root representations")
            .items()
            .len(),
        2
    );

    let metadata = production
        .metadata_query(
            &MetadataQuery::new(property.clone(), Some(matching_value)).expect("query"),
            &page(10),
        )
        .expect("metadata query");
    assert_eq!(metadata.items().len(), 1);
    assert_eq!(
        metadata.items()[0].target(),
        ObjectRef::Asset(source.asset().id())
    );

    let outputs = production
        .activity_outputs(
            &ActivityOutputQuery::Tool(
                ToolIdentity::new("FFmpeg", Some("8".to_owned()), None).expect("tool"),
            ),
            &page(10),
        )
        .expect("tool outputs");
    assert_eq!(outputs.items(), &[proxy.representation().id()]);
    assert_eq!(
        production
            .activities_producing_page(proxy.representation().id(), &page(10))
            .expect("producers")
            .items()[0]
            .id(),
        transcode.id()
    );
    let descendants = production
        .descendants_page(
            source.representation().id(),
            ProvenanceQueryLimits::new(8, 16).expect("limits"),
            &page(10),
        )
        .expect("descendants");
    assert_eq!(descendants.items().len(), 2);
    assert_eq!(descendants.items()[0].depth(), 1);
    assert_eq!(descendants.items()[1].depth(), 2);

    {
        let mut transaction = production.begin_transaction().expect("begin change");
        transaction
            .record_representation_fingerprint(
                source.representation().id(),
                &RepresentationFingerprint::new("test", 1, vec![99]).expect("changed"),
            )
            .expect("record observation");
        transaction
            .retire_locator(render.locators()[0].id())
            .expect("retire locator");
        transaction.commit().expect("commit change");
    }
    assert_eq!(
        production
            .unresolved_media(&page(10))
            .expect("unresolved")
            .items(),
        &[render.representation().id()]
    );
    let stale = production
        .stale_artifacts(
            StaleArtifactQuery::new(None, ArtifactEvaluationLimits::DEFAULT),
            &page(10),
        )
        .expect("stale artifacts");
    assert!(stale.items().contains(&proxy.representation().id()));
    assert!(stale.items().contains(&render.representation().id()));

    let changed = production
        .objects_changed_since(setup_revision, &page(10))
        .expect("changed objects");
    assert!(
        changed
            .items()
            .contains(&ObjectRef::Representation(source.representation().id()))
    );
    assert!(
        changed
            .items()
            .contains(&ObjectRef::Resource(render.resources()[0].id()))
    );
}

#[test]
fn cursors_are_scoped_to_their_query() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("cursor.pproj");
    let mut production = SqliteProduction::create(path, None).expect("create production");
    let first = imported(4, None);
    let second = imported(5, None);
    let mut transaction = production.begin_transaction().expect("begin");
    transaction.import_original(&first).expect("first import");
    transaction.import_original(&second).expect("second import");
    transaction.commit().expect("commit");
    drop(transaction);

    let assets = production.assets_page(&page(1)).expect("asset page");
    let wrong = QueryPageRequest::new(1, assets.next_cursor().cloned()).expect("page request");
    assert!(production.unresolved_media(&wrong).is_err());
}

#[test]
fn point_reads_return_one_object_or_not_found() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("points.pproj");
    let mut production = SqliteProduction::create(path, None).expect("create production");
    let first = imported(6, None);
    let second = imported(7, None);
    let mut transaction = production.begin_transaction().expect("begin");
    transaction.import_original(&first).expect("first import");
    transaction.import_original(&second).expect("second import");
    transaction.commit().expect("commit");
    drop(transaction);

    assert_eq!(
        production.asset(first.asset().id()).expect("asset"),
        *first.asset()
    );
    assert_eq!(
        production
            .representation(second.representation().id())
            .expect("representation"),
        *second.representation()
    );
    let users = production
        .representations_using_resource(ResourceId::from_bytes([7; 16]), &page(1))
        .expect("representations using resource");
    assert_eq!(users.items(), [second.representation().clone()]);
    assert!(users.next_cursor().is_none());

    let absent = [9; 16];
    for error in [
        production.asset(AssetId::from_bytes(absent)).map(drop),
        production
            .representation(RepresentationId::from_bytes(absent))
            .map(drop),
        production
            .representations_using_resource(ResourceId::from_bytes(absent), &page(1))
            .map(drop),
    ] {
        assert_eq!(
            error.expect_err("absent object").kind(),
            postproject_core::ErrorKind::NotFound
        );
    }
    let assets = production.assets_page(&page(1)).expect("asset page");
    let foreign = QueryPageRequest::new(1, assets.next_cursor().cloned()).expect("page request");
    assert!(
        production
            .representations_using_resource(ResourceId::from_bytes([6; 16]), &foreign)
            .is_err()
    );
}
