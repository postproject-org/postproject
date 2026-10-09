//! Scalar media proposals retain native guards, ordered effects and recovery.

use postproject_core::{
    Asset, AssetId, ContentStructure, DecisionBase, ExternalIdentifier, FileFacts,
    IdentifierScheme, Locator, LocatorAvailability, LocatorId, MediaRoot, MediaRootId, ObjectRef,
    OriginalMediaImport, Representation, RepresentationId, RepresentationKind, Resource,
    ResourceId, RevisionContext, Timestamp,
};
use postproject_protocol::{
    ClientId, Command, Extensions, IdentifierAttachment, OutcomeStatus, Proposal, ProtocolBase,
    RequestId,
};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

fn proposal(
    source: &SqliteProduction,
    base: Option<DecisionBase>,
    commands: Vec<Command>,
) -> Proposal {
    let scope = source.exchange_scope().unwrap();
    Proposal::new(
        scope,
        ClientId::new(),
        RequestId::new(),
        base.map(|base| ProtocolBase::new(scope, base).unwrap()),
        RevisionContext::default(),
        commands,
        Extensions::default(),
    )
    .unwrap()
}

fn import(source: &mut SqliteProduction) -> (ResourceId, RepresentationId) {
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(-7), None, None);
    let id = ResourceId::new();
    let aggregate = OriginalMediaImport::new(
        asset.clone(),
        Representation::new(
            RepresentationId::new(),
            asset.id(),
            RepresentationKind::Original,
            ContentStructure::single_resource(id),
            vec![],
        ),
        vec![Resource::new(id, vec![], None)],
        vec![
            Locator::new(
                LocatorId::new(),
                id,
                "file:///missing/original.mov",
                None,
                LocatorAvailability::Unknown,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&aggregate).unwrap();
    edit.commit().unwrap();
    (id, aggregate.representation().id())
}

fn root() -> MediaRoot {
    MediaRoot::new(
        MediaRootId::new(),
        "rushes",
        Some("名".into()),
        None,
        -7,
        true,
    )
    .unwrap()
}

#[test]
fn ordered_media_commands_recover_before_current_guards_and_replay_exactly() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let (resource, representation) = import(&mut source);
    let original_locators = source.locators(resource).unwrap();
    let root = root();
    let locator = Locator::new(
        LocatorId::new(),
        resource,
        "file:///missing/名.mov",
        Some(Timestamp::from_unix_micros(-9)),
        LocatorAvailability::Offline,
    )
    .unwrap();
    let attachment = IdentifierAttachment::new(
        ObjectRef::Resource(resource),
        ExternalIdentifier::new(
            IdentifierScheme::new("unknown:Exact").unwrap(),
            "  名  ",
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let facts = FileFacts::new(
        i64::MAX.unsigned_abs(),
        Some(Timestamp::from_unix_micros(i64::MIN)),
    );
    let base = source.read_session().unwrap().decision_base();
    let request = proposal(
        &source,
        Some(base),
        vec![
            Command::AddMediaRoot(root.clone()),
            Command::SetMediaRootEnabled {
                root_id: root.id(),
                enabled: false,
            },
            Command::AddLocator(locator.clone()),
            Command::AddIdentifier(attachment.clone()),
            Command::RecordResourceFileFacts {
                resource_id: resource,
                facts,
            },
            Command::RemoveIdentifier(attachment),
            Command::RetireLocator(locator.id()),
            Command::RemoveMediaRoot(root.id()),
        ],
    );
    let outcome = source.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Accepted(receipt)
        if receipt.revision().unwrap().sequence() == 2),
        "{outcome:?}"
    );
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    // Removed objects make a new attempt invalid; identity recovery wins.
    assert_eq!(source.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(
        source
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap(),
        Some(outcome)
    );
    assert!(source.media_roots().unwrap().is_empty());
    assert_eq!(source.locators(resource).unwrap(), original_locators);
    assert!(
        source
            .external_identifiers(ObjectRef::Resource(resource))
            .unwrap()
            .is_empty()
    );
    replay(&source, &mut mirror);
    assert_eq!(
        mirror.resources(representation).unwrap()[0].file_facts(),
        Some(facts)
    );
    assert!(mirror.media_roots().unwrap().is_empty());
    assert_eq!(mirror.locators(resource).unwrap(), original_locators);
    assert!(
        mirror
            .external_identifiers(ObjectRef::Resource(resource))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
}

fn replay(source: &SqliteProduction, mirror: &mut SqliteProduction) {
    for sequence in 1..=2 {
        let mut reader = source.record_reader(sequence).unwrap();
        let manifest = reader.manifest().clone();
        assert!(
            mirror
                .apply_record(
                    &manifest,
                    std::iter::from_fn(|| reader.next_chunk().transpose()),
                    ReplayLimits::default()
                )
                .unwrap()
        );
        assert_eq!(
            mirror
                .events_for_revision(manifest.revision().id())
                .unwrap(),
            source
                .events_for_revision(manifest.revision().id())
                .unwrap()
        );
    }
}

#[path = "media_submissions/guards.rs"]
mod guards;
