//! Known-media lookup and explicit-adoption integration coverage.

use postproject_core::{
    Asset, AssetId, ContentStructure, ExternalIdentifier, FrameRange, IdentifierScheme,
    ImageSequenceDescriptor, Locator, LocatorAvailability, LocatorId, LocatorIdentity, ObjectRef,
    OriginalMediaImport, QueryPageRequest, RationalRate, Representation, RepresentationFingerprint,
    RepresentationId, RepresentationKind, Resource, ResourceFingerprint, ResourceId,
    SequenceNaming, Timestamp,
};
use postproject_storage_sqlite::SqliteProduction;

fn page(limit: u32) -> QueryPageRequest {
    QueryPageRequest::new(limit, None).expect("page request")
}

fn single_file(label: u8, uri: &str, fingerprint: &[u8]) -> OriginalMediaImport {
    let asset_id = AssetId::from_bytes([label; 16]);
    let representation_id = RepresentationId::from_bytes([label; 16]);
    let resource_id = ResourceId::from_bytes([label; 16]);
    OriginalMediaImport::new(
        Asset::new(
            asset_id,
            Timestamp::from_unix_micros(i64::from(label)),
            Some(format!("asset-{label}")),
            None,
        ),
        Representation::new(
            representation_id,
            asset_id,
            RepresentationKind::Original,
            ContentStructure::single_resource(resource_id),
            vec![
                RepresentationFingerprint::new("test-tree", 1, vec![label])
                    .expect("representation fingerprint"),
            ],
        ),
        vec![Resource::new(
            resource_id,
            vec![
                ResourceFingerprint::new("foreign-host", 7, fingerprint.to_vec())
                    .expect("resource fingerprint"),
            ],
            None,
        )],
        vec![
            Locator::new(
                LocatorId::from_bytes([label; 16]),
                resource_id,
                uri,
                None,
                LocatorAvailability::Online,
            )
            .expect("locator"),
        ],
    )
    .expect("single-file import")
}

fn sequence(label: u8, uri: &str, naming: SequenceNaming) -> OriginalMediaImport {
    let asset_id = AssetId::from_bytes([label; 16]);
    let representation_id = RepresentationId::from_bytes([label; 16]);
    let resource_id = ResourceId::from_bytes([label; 16]);
    let descriptor = ImageSequenceDescriptor::new(
        resource_id,
        FrameRange::new(1, 2, 1).expect("frames"),
        RationalRate::new(24, 1).expect("rate"),
        Vec::new(),
    )
    .expect("sequence descriptor");
    OriginalMediaImport::new(
        Asset::new(
            asset_id,
            Timestamp::from_unix_micros(i64::from(label)),
            None,
            None,
        ),
        Representation::new(
            representation_id,
            asset_id,
            RepresentationKind::Original,
            ContentStructure::image_sequence(descriptor),
            Vec::new(),
        ),
        vec![Resource::new(resource_id, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::from_bytes([label; 16]),
                resource_id,
                uri,
                None,
                LocatorAvailability::Online,
            )
            .expect("sequence locator")
            .with_sequence_naming(naming),
        ],
    )
    .expect("sequence import")
}

#[test]
fn another_handle_finds_candidates_and_adopts_one_explicitly() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("known.pproj");
    let uri = "file:///shared/rushes/A001.mov";
    let first = single_file(1, uri, b"same-content");
    let ambiguous = single_file(2, uri, b"same-content");
    let mut writer = SqliteProduction::create(&path, None).expect("create production");
    {
        let mut transaction = writer.begin_transaction().expect("begin import");
        transaction.import_original(&first).expect("import media");
        transaction.commit().expect("commit import");
    }
    let mut adopter = SqliteProduction::open(&path).expect("open second handle");
    let locator = LocatorIdentity::new(uri, None).expect("locator identity");
    let fingerprint = ResourceFingerprint::new("foreign-host", 7, b"same-content".to_vec())
        .expect("fingerprint identity");

    let by_locator = adopter
        .find_known_media_by_locator(&locator, &page(10))
        .expect("find by locator");
    let by_fingerprint = adopter
        .find_known_media_by_fingerprint(&fingerprint, &page(10))
        .expect("find by fingerprint");
    for matches in [by_locator.items(), by_fingerprint.items()] {
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].asset().id(), first.asset().id());
        assert_eq!(
            matches[0].representation().id(),
            first.representation().id()
        );
        assert_eq!(matches[0].resource().id(), first.resources()[0].id());
    }

    let scheme = IdentifierScheme::new("org.example.second-host").expect("scheme");
    let identifier =
        ExternalIdentifier::new(scheme.clone(), "bin-item-42", None).expect("external identifier");
    {
        let mut transaction = adopter.begin_transaction().expect("begin adoption");
        transaction
            .add_external_identifier(ObjectRef::Asset(first.asset().id()), &identifier)
            .expect("attach host identifier");
        transaction.commit().expect("commit adoption");
    }
    assert_eq!(
        writer
            .find_by_external_identifier(&scheme, identifier.value(), None)
            .expect("resolve second-host identifier"),
        [ObjectRef::Asset(first.asset().id())]
    );

    {
        let mut transaction = writer.begin_transaction().expect("begin ambiguous import");
        transaction
            .import_original(&ambiguous)
            .expect("import ambiguous logical asset");
        transaction.commit().expect("commit ambiguous import");
    }
    let first_page = adopter
        .find_known_media_by_fingerprint(&fingerprint, &page(1))
        .expect("first candidate page");
    assert_eq!(first_page.items().len(), 1);
    let second_page = adopter
        .find_known_media_by_fingerprint(
            &fingerprint,
            &QueryPageRequest::new(1, first_page.next_cursor().cloned()).expect("next page"),
        )
        .expect("second candidate page");
    assert_eq!(second_page.items().len(), 1);
    assert_ne!(
        first_page.items()[0].asset(),
        second_page.items()[0].asset()
    );
}

#[test]
fn lookup_uses_only_current_locator_and_fingerprint_state() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("current.pproj");
    let uri = "file:///shared/clip.mov";
    let imported = single_file(3, uri, b"old");
    let resource_id = imported.resources()[0].id();
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    {
        let mut transaction = production.begin_transaction().expect("begin import");
        transaction
            .import_original(&imported)
            .expect("import media");
        transaction.commit().expect("commit import");
    }
    {
        let mut transaction = production.begin_transaction().expect("begin update");
        transaction
            .record_resource_fingerprint(
                resource_id,
                &ResourceFingerprint::new("foreign-host", 7, b"new".to_vec())
                    .expect("new fingerprint"),
            )
            .expect("replace effective fingerprint");
        transaction
            .retire_locator(imported.locators()[0].id())
            .expect("retire locator");
        transaction.commit().expect("commit update");
    }

    assert_eq!(
        production
            .find_known_media_by_locator(
                &LocatorIdentity::new(uri, None).expect("locator identity"),
                &page(10),
            )
            .expect("query retired locator")
            .items(),
        []
    );
    assert_eq!(
        production
            .find_known_media_by_fingerprint(
                &ResourceFingerprint::new("foreign-host", 7, b"old".to_vec())
                    .expect("old fingerprint"),
                &page(10),
            )
            .expect("query historical fingerprint")
            .items(),
        []
    );
    assert_eq!(
        production
            .find_known_media_by_fingerprint(
                &ResourceFingerprint::new("foreign-host", 7, b"new".to_vec())
                    .expect("new fingerprint"),
                &page(10),
            )
            .expect("query current fingerprint")
            .items()
            .len(),
        1
    );
}

#[test]
fn sequence_lookup_requires_the_exact_directory_and_naming() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("sequences.pproj");
    let uri = "file:///shared/plates/shot-a/";
    let naming = SequenceNaming::new("plate.", ".exr", 4).expect("naming");
    let imported = sequence(4, uri, naming.clone());
    let mut production = SqliteProduction::create(&path, None).expect("create production");
    {
        let mut transaction = production.begin_transaction().expect("begin import");
        transaction
            .import_original(&imported)
            .expect("import sequence");
        transaction.commit().expect("commit sequence");
    }

    assert_eq!(
        production
            .find_known_media_by_locator(
                &LocatorIdentity::new(uri, Some(naming)).expect("sequence identity"),
                &page(10),
            )
            .expect("find sequence")
            .items()
            .len(),
        1
    );
    for identity in [
        LocatorIdentity::new(uri, None).expect("directory-only identity"),
        LocatorIdentity::new(
            uri,
            Some(SequenceNaming::new("other.", ".exr", 4).expect("other naming")),
        )
        .expect("other sequence identity"),
    ] {
        assert_eq!(
            production
                .find_known_media_by_locator(&identity, &page(10))
                .expect("find nonmatching sequence")
                .items(),
            []
        );
    }
}

#[test]
fn simultaneous_first_encounters_may_create_distinct_logical_assets() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("first-import.pproj");
    let uri = "file:///shared/new.mov";
    let fingerprint =
        ResourceFingerprint::new("foreign-host", 7, b"new-media".to_vec()).expect("fingerprint");
    let locator = LocatorIdentity::new(uri, None).expect("locator identity");
    let mut first_host = SqliteProduction::create(&path, None).expect("create production");
    let mut second_host = SqliteProduction::open(&path).expect("open second handle");

    for host in [&first_host, &second_host] {
        assert_eq!(
            host.find_known_media_by_locator(&locator, &page(10))
                .expect("initial locator lookup")
                .items(),
            []
        );
        assert_eq!(
            host.find_known_media_by_fingerprint(&fingerprint, &page(10))
                .expect("initial fingerprint lookup")
                .items(),
            []
        );
    }

    for (host, import) in [
        (&mut first_host, single_file(5, uri, b"new-media")),
        (&mut second_host, single_file(6, uri, b"new-media")),
    ] {
        let mut transaction = host.begin_transaction().expect("begin first import");
        transaction.import_original(&import).expect("import media");
        transaction.commit().expect("commit first import");
    }

    assert_eq!(
        first_host
            .find_known_media_by_fingerprint(&fingerprint, &page(10))
            .expect("find both logical assets")
            .items()
            .len(),
        2
    );
}
