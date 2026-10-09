use super::*;
use postproject_core::{ErrorKind, RepresentationImport};
use postproject_protocol::{Document, Limits, RejectionKind};

fn proxy(asset: AssetId) -> RepresentationImport {
    let (_, representation, resources, locators) = prepared_import().into_parts();
    RepresentationImport::new(
        Representation::new(
            representation.id(),
            asset,
            RepresentationKind::Proxy,
            representation.content_structure().clone(),
            vec![],
        ),
        resources,
        locators,
    )
    .unwrap()
}

#[test]
fn complete_decoded_aggregates_publish_in_one_revision_and_recover_after_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let original = prepared_import();
    let proxy = proxy(original.asset().id());
    let request = proposal(
        &source,
        None,
        vec![
            Command::ImportOriginal(original.clone()),
            Command::AddRepresentation(proxy.clone()),
        ],
    );
    let bytes = request.document().unwrap().canonical_bytes().unwrap();
    let request =
        Proposal::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap();
    let outcome = source.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Accepted(receipt)
        if receipt.revision().unwrap().sequence() == 1),
        "{outcome:?}"
    );
    assert_eq!(
        source.asset(original.asset().id()).unwrap(),
        *original.asset()
    );
    for representation in [original.representation(), proxy.representation()] {
        assert_eq!(
            source.representation(representation.id()).unwrap(),
            *representation
        );
    }
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    assert_eq!(source.submit_proposal(&request).unwrap(), outcome);
    assert_eq!(source.changes_since(0, 10).unwrap().len(), 1);
    let mut reader = source.record_reader(1).unwrap();
    let manifest = reader.manifest().clone();
    assert_eq!(manifest.effect_count(), 2);
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
        mirror.asset(original.asset().id()).unwrap(),
        *original.asset()
    );
    for media in [original.representation(), proxy.representation()] {
        assert_eq!(mirror.representation(media.id()).unwrap(), *media);
        assert_eq!(
            mirror.resources(media.id()).unwrap(),
            source.resources(media.id()).unwrap()
        );
        for resource in mirror.resources(media.id()).unwrap() {
            assert_eq!(
                mirror.locators(resource.id()).unwrap(),
                source.locators(resource.id()).unwrap()
            );
        }
    }
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
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

#[test]
fn later_missing_asset_rolls_back_all_prepared_import_rows_and_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let original = prepared_import();
    let request = proposal(
        &source,
        None,
        vec![
            Command::ImportOriginal(original.clone()),
            Command::AddRepresentation(proxy(AssetId::new())),
        ],
    );
    let head = source.exchange_head().unwrap();
    let outcome = source.submit_proposal(&request).unwrap();
    assert!(
        matches!(outcome.status(), OutcomeStatus::Rejected(rejection)
        if rejection.kind() == RejectionKind::Domain(ErrorKind::NotFound)),
        "{outcome:?}"
    );
    assert_eq!(source.exchange_head().unwrap(), head);
    assert_eq!(
        source.asset(original.asset().id()).unwrap_err().kind(),
        ErrorKind::NotFound
    );
    assert!(source.changes_since(0, 10).unwrap().is_empty());
    let connection = rusqlite::Connection::open(&path).unwrap();
    for table in [
        "assets",
        "representations",
        "resources",
        "locators",
        "exchange_records",
    ] {
        assert_eq!(
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0,
            "{table}"
        );
    }
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    assert_eq!(source.submit_proposal(&request).unwrap(), outcome);
}
