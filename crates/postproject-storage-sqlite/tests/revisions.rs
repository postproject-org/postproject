//! Durable revision journal integration coverage.

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityRole, Asset,
    AssetId, ContentStructure, ErrorKind, ExternalIdentifier, IdentifierScheme, Locator,
    LocatorAvailability, LocatorId, MediaRoot, MediaRootId, MetadataProperty, MetadataValue,
    ObjectRef, OriginIdentity, OriginalMediaImport, PropertyId, Representation, RepresentationId,
    RepresentationKind, Resource, ResourceId, RevisionContext, RevisionEventFilter,
    RevisionEventKind, RevisionEventType, Timestamp, VocabularyId,
};
use postproject_storage_sqlite::SqliteProduction;
use tempfile::tempdir;

fn import(label: u8) -> OriginalMediaImport {
    let asset_id = AssetId::from_bytes([label; 16]);
    let representation_id = RepresentationId::from_bytes([label.wrapping_add(1); 16]);
    let resource_id = ResourceId::from_bytes([label.wrapping_add(2); 16]);
    let locator_id = LocatorId::from_bytes([label.wrapping_add(3); 16]);
    OriginalMediaImport::new(
        Asset::new(
            asset_id,
            Timestamp::from_unix_micros(i64::from(label)),
            Some(format!("Asset {label}")),
            None,
        ),
        Representation::new(
            representation_id,
            asset_id,
            RepresentationKind::Original,
            ContentStructure::single_resource(resource_id),
            Vec::new(),
        ),
        vec![Resource::new(resource_id, Vec::new(), None)],
        vec![
            Locator::new(
                locator_id,
                resource_id,
                format!("file:///media/{label}.mov"),
                None,
                LocatorAvailability::Online,
            )
            .expect("valid locator"),
        ],
    )
    .expect("valid import")
}

#[test]
fn imported_media_creates_a_durable_contextual_revision() {
    let directory = tempdir().expect("create temporary directory");
    let path = directory.path().join("production.pproj");
    let media = import(10);
    let asset_id = media.asset().id();
    let representation_id = media.representation().id();
    let resource_id = media.resources()[0].id();
    let locator_id = media.locators()[0].id();
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    assert_eq!(production.latest_revision().expect("query journal"), None);
    assert_eq!(production.changes_since(0, 10).expect("query feed"), []);

    let transaction_id;
    {
        let mut transaction = production.begin_transaction().expect("begin transaction");
        transaction_id = transaction.id();
        transaction
            .set_revision_context(
                RevisionContext::new(
                    Some(
                        OriginIdentity::new(
                            "Editorial host",
                            Some("2.4.1".to_owned()),
                            Some("https://example.com/editor".to_owned()),
                        )
                        .expect("valid origin"),
                    ),
                    Some("Import camera original".to_owned()),
                )
                .expect("valid revision context"),
            )
            .expect("set revision context");
        transaction.import_original(&media).expect("stage import");
        transaction.commit().expect("commit import");
    }

    let revision = production
        .latest_revision()
        .expect("query latest revision")
        .expect("revision exists");
    assert_eq!(revision.sequence(), 1);
    assert_eq!(revision.transaction_id(), transaction_id);
    assert_eq!(revision.origin().expect("origin").name(), "Editorial host");
    assert_eq!(revision.message(), Some("Import camera original"));
    let page = production.changes_since(0, 1).expect("query page");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0], revision);

    let events = production
        .events_for_revision(revision.id())
        .expect("load revision events");
    assert_eq!(events.len(), 5);
    assert!(matches!(
        events[0].kind(),
        RevisionEventKind::AssetImported { asset_id: id } if *id == asset_id
    ));
    assert!(matches!(
        events[1].kind(),
        RevisionEventKind::RepresentationAdded { asset_id: owner, representation_id: id }
            if *owner == asset_id && *id == representation_id
    ));
    assert!(matches!(
        events[2].kind(),
        RevisionEventKind::ResourceAdded { resource_id: id } if *id == resource_id
    ));
    assert!(matches!(
        events[3].kind(),
        RevisionEventKind::RepresentationResourceAdded {
            representation_id: owner,
            resource_id: id,
            position: 0,
        } if *owner == representation_id && *id == resource_id
    ));
    assert!(matches!(
        events[4].kind(),
        RevisionEventKind::LocatorAdded { resource_id: owner, locator_id: id }
            if *owner == resource_id && *id == locator_id
    ));
    assert!(
        events
            .iter()
            .enumerate()
            .all(|(position, event)| event.revision_id() == revision.id()
                && event.position() == u32::try_from(position).expect("small position"))
    );

    drop(production);
    let reopened = SqliteProduction::open(path).expect("reopen production");
    assert_eq!(
        reopened.latest_revision().expect("query reopened journal"),
        Some(revision)
    );
}

#[test]
fn revision_queries_reject_invalid_bounds_and_missing_ids() {
    let directory = tempdir().expect("create temporary directory");
    let production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");

    assert_eq!(
        production
            .changes_since(0, 0)
            .expect_err("zero page size must fail")
            .kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        production
            .changes_since(0, 1_001)
            .expect_err("excessive page size must fail")
            .kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        production
            .events_for_revision(postproject_core::RevisionId::new())
            .expect_err("missing revision must fail")
            .kind(),
        ErrorKind::NotFound
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered scenario covers the complete semantic event mapping"
)]
fn journal_decodes_every_non_import_event_kind() {
    let directory = tempdir().expect("create temporary directory");
    let path = directory.path().join("production.pproj");
    let source = import(20);
    let output = import(30);
    let source_id = source.representation().id();
    let output_id = output.representation().id();
    let target = ObjectRef::Asset(source.asset().id());
    let mut production = SqliteProduction::create(path, None).expect("create production");
    {
        let mut transaction = production
            .begin_transaction()
            .expect("begin import transaction");
        transaction.import_original(&source).expect("import source");
        transaction.import_original(&output).expect("import output");
        transaction.commit().expect("commit imports");
    }

    let root_id = MediaRootId::new();
    let root = MediaRoot::new(root_id, "media", None, None, 0, true).expect("valid root");
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("com.example.asset").expect("valid scheme"),
        "A001",
        Some("production".to_owned()),
    )
    .expect("valid identifier");
    let property = MetadataProperty::new(
        VocabularyId::new("com.example.editorial").expect("valid vocabulary"),
        PropertyId::new("status").expect("valid property"),
    );
    let value = MetadataValue::string("approved").expect("valid metadata");
    let activity_id = ActivityId::new();
    let input_role = ActivityRole::new("org.postproject:input.primary").expect("valid input role");
    let output_role = ActivityRole::new("org.postproject:output.proxy").expect("valid output role");
    let activity_kind =
        ActivityKind::new("org.postproject:transcode").expect("valid activity kind");
    let activity = Activity::new(
        activity_id,
        activity_kind.clone(),
        vec![ActivityInput::new(source_id, Some(input_role.clone()))],
        vec![ActivityOutput::new(output_id, Some(output_role.clone()))],
    )
    .expect("valid activity");

    {
        let base = production.read_session().unwrap().decision_base();
        let mut transaction = production
            .begin_edit(base)
            .expect("begin mutation transaction");
        transaction.add_media_root(root).expect("add media root");
        transaction
            .add_external_identifier(target, &identifier)
            .expect("add identifier");
        transaction
            .remove_external_identifier(target, &identifier)
            .expect("remove identifier");
        transaction
            .add_metadata_value(target, &property, &value)
            .expect("add metadata");
        transaction
            .remove_metadata_property(target, &property)
            .expect("remove metadata");
        transaction
            .create_activity(&activity)
            .expect("create activity");
        transaction.commit().expect("commit mutations");
    }

    let page = production.changes_since(1, 10).expect("load second page");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].sequence(), 2);
    let events = production
        .events_for_revision(page[0].id())
        .expect("load mutation events");
    assert_eq!(events.len(), 8);
    assert!(matches!(
        events[0].kind(),
        RevisionEventKind::MediaRootAdded { media_root_id } if *media_root_id == root_id
    ));
    assert!(matches!(
        events[1].kind(),
        RevisionEventKind::ExternalIdentifierAdded { target: event_target, identifier: event_id }
            if *event_target == target && event_id == &identifier
    ));
    assert!(matches!(
        events[2].kind(),
        RevisionEventKind::ExternalIdentifierRemoved { target: event_target, identifier: event_id }
            if *event_target == target && event_id == &identifier
    ));
    assert!(matches!(
        events[3].kind(),
        RevisionEventKind::MetadataAddedOrReplaced { target: event_target, property: event_property }
            if *event_target == target && event_property == &property
    ));
    assert!(matches!(
        events[4].kind(),
        RevisionEventKind::MetadataRemoved { target: event_target, property: event_property }
            if *event_target == target && event_property == &property
    ));
    assert!(matches!(
        events[5].kind(),
        RevisionEventKind::ActivityCreated { activity_id: id, kind }
            if *id == activity_id && kind == &activity_kind
    ));
    assert!(matches!(
        events[6].kind(),
        RevisionEventKind::ActivityInputAdded {
            activity_id: id,
            representation_id,
            role: Some(role),
        } if *id == activity_id && *representation_id == source_id && role == &input_role
    ));
    assert!(matches!(
        events[7].kind(),
        RevisionEventKind::ActivityOutputAdded {
            activity_id: id,
            representation_id,
            role: Some(role),
        } if *id == activity_id && *representation_id == output_id && role == &output_role
    ));
}

#[test]
fn only_successful_nonempty_transactions_advance_the_feed() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");

    production
        .begin_transaction()
        .expect("begin empty transaction")
        .commit()
        .expect("commit empty transaction");
    {
        let mut transaction = production.begin_transaction().expect("begin rollback");
        transaction
            .import_original(&import(40))
            .expect("stage import");
        transaction.rollback().expect("roll back import");
    }
    {
        let mut transaction = production
            .begin_transaction()
            .expect("begin failed mutation");
        let missing_identifier = ExternalIdentifier::new(
            IdentifierScheme::new("com.example.missing").expect("valid scheme"),
            "missing",
            None,
        )
        .expect("valid identifier");
        assert_eq!(
            transaction
                .add_external_identifier(ObjectRef::Asset(AssetId::new()), &missing_identifier)
                .expect_err("missing target must fail")
                .kind(),
            ErrorKind::NotFound
        );
        transaction.commit().expect("commit after failed mutation");
    }
    assert_eq!(
        production.changes_since(0, 10).expect("query empty feed"),
        []
    );

    for label in [50, 60, 70] {
        let mut transaction = production.begin_transaction().expect("begin import");
        transaction
            .import_original(&import(label))
            .expect("stage import");
        transaction.commit().expect("commit import");
    }

    let first_page = production.changes_since(0, 2).expect("load first page");
    assert_eq!(
        first_page
            .iter()
            .map(postproject_core::Revision::sequence)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let second_page = production.changes_since(2, 2).expect("load second page");
    assert_eq!(
        second_page
            .iter()
            .map(postproject_core::Revision::sequence)
            .collect::<Vec<_>>(),
        [3]
    );
    assert_eq!(production.changes_since(3, 2).expect("load feed end"), []);
    assert_eq!(
        production
            .changes_since(u64::MAX, 2)
            .expect("load beyond storage range"),
        []
    );
}

fn sequences(revisions: &[postproject_core::Revision]) -> Vec<u64> {
    revisions
        .iter()
        .map(postproject_core::Revision::sequence)
        .collect()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one journal exercises every page boundary of the filtered feed"
)]
fn filtered_pages_select_matching_revisions_and_advance_past_the_rest() {
    let directory = tempdir().expect("create temporary directory");
    let mut production = SqliteProduction::create(directory.path().join("production.pproj"), None)
        .expect("create production");
    let imported = RevisionEventFilter::new([RevisionEventType::AssetImported]).expect("filter");
    let empty = production
        .changes_since_filtered(0, &imported, 10)
        .expect("filter empty journal");
    assert_eq!(empty.revisions(), []);
    assert_eq!(empty.through_sequence(), 0);

    for label in [10, 20, 30] {
        let mut transaction = production.begin_transaction().expect("begin import");
        transaction.import_original(&import(label)).expect("import");
        transaction.commit().expect("commit import");
    }
    let property = MetadataProperty::new(
        VocabularyId::new("com.example.editorial").expect("valid vocabulary"),
        PropertyId::new("status").expect("valid property"),
    );
    {
        let mut transaction = production.begin_transaction().expect("begin metadata");
        transaction
            .add_metadata_value(
                ObjectRef::Asset(AssetId::from_bytes([10; 16])),
                &property,
                &MetadataValue::string("approved").expect("valid metadata"),
            )
            .expect("add metadata");
        transaction.commit().expect("commit metadata");
    }
    {
        let mut transaction = production.begin_transaction().expect("begin root");
        transaction
            .add_media_root(
                MediaRoot::new(MediaRootId::new(), "media", None, None, 0, true)
                    .expect("valid root"),
            )
            .expect("add root");
        transaction.commit().expect("commit root");
    }

    let first = production
        .changes_since_filtered(0, &imported, 2)
        .expect("first imported page");
    assert_eq!(sequences(first.revisions()), [1, 2]);
    assert_eq!(first.through_sequence(), 2);
    let second = production
        .changes_since_filtered(first.through_sequence(), &imported, 2)
        .expect("second imported page");
    assert_eq!(sequences(second.revisions()), [3]);
    assert_eq!(second.through_sequence(), 5);
    let drained = production
        .changes_since_filtered(second.through_sequence(), &imported, 2)
        .expect("drained imported page");
    assert_eq!(drained.revisions(), []);
    assert_eq!(drained.through_sequence(), 5);

    let metadata_or_roots = RevisionEventFilter::new([
        RevisionEventType::MediaRootAdded,
        RevisionEventType::MetadataAddedOrReplaced,
    ])
    .expect("filter");
    let page = production
        .changes_since_filtered(0, &metadata_or_roots, 10)
        .expect("metadata or root page");
    assert_eq!(sequences(page.revisions()), [4, 5]);
    assert_eq!(
        page.revisions(),
        &production.changes_since(3, 2).unwrap()[..]
    );

    let resources = RevisionEventFilter::new([
        RevisionEventType::ResourceAdded,
        RevisionEventType::LocatorAdded,
    ])
    .expect("filter");
    let page = production
        .changes_since_filtered(1, &resources, 10)
        .expect("multi-event revisions appear once");
    assert_eq!(sequences(page.revisions()), [2, 3]);

    let beyond = production
        .changes_since_filtered(99, &imported, 10)
        .expect("cursor beyond journal");
    assert_eq!(beyond.revisions(), []);
    assert_eq!(beyond.through_sequence(), 99);
    let unrepresentable = production
        .changes_since_filtered(u64::MAX, &imported, 10)
        .expect("unrepresentable cursor");
    assert_eq!(unrepresentable.through_sequence(), u64::MAX);

    for limit in [0, postproject_core::MAX_REVISION_PAGE_SIZE + 1] {
        assert_eq!(
            production
                .changes_since_filtered(0, &imported, limit)
                .expect_err("invalid limit")
                .kind(),
            ErrorKind::InvalidArgument
        );
    }

    drop(production);
    let reopened = SqliteProduction::open(directory.path().join("production.pproj"))
        .expect("reopen production");
    assert_eq!(
        sequences(
            reopened
                .changes_since_filtered(0, &metadata_or_roots, 10)
                .expect("filter after reopen")
                .revisions()
        ),
        [4, 5]
    );
}
