//! Runs the Rust listings of the identifier and metadata guide.
//!
//! Each `// [name]` ... `// [/name]` region is included verbatim by the
//! documentation build, so keep regions self-contained and readable.

use std::fs;
use std::path::{Path, PathBuf};

use postproject_core::{
    AssetId, DecimalValue, ExternalIdentifier, IdentifierScheme, LocatorIdentity, MetadataField,
    MetadataMatch, MetadataProperty, MetadataQuery, MetadataValue, MetadataValueKind, ObjectRef,
    PropertyId, QueryPageRequest, RationalValue, RepresentationId, ResourceFingerprint, Result,
    Timestamp, VocabularyId,
};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;

// [known-media-adoption]
fn find_known_media(
    production_path: &Path,
    locator_uri: &str,
    fingerprint: &ResourceFingerprint,
) -> Result<AssetId> {
    // Another host or process opens the same production explicitly.
    let production = SqliteProduction::open(production_path)?;
    let page = QueryPageRequest::new(100, None)?;
    let locator = LocatorIdentity::new(locator_uri, None)?;
    let by_locator = production.find_known_media_by_locator(&locator, &page)?;
    let by_content = production.find_known_media_by_fingerprint(fingerprint, &page)?;

    // Never silently choose when either query returns several candidates.
    assert_eq!(by_locator.items(), by_content.items());
    assert_eq!(by_locator.items().len(), 1);
    Ok(by_locator.items()[0].asset().id())
}
// [/known-media-adoption]

// [remove-identifier]
fn replace_reel_identifier(production: &mut SqliteProduction, asset_id: AssetId) -> Result<()> {
    let target = ObjectRef::Asset(asset_id);
    let reel = ExternalIdentifier::new(IdentifierScheme::new("com.example.reel")?, "R-12", None)?;
    let serial = ExternalIdentifier::new(
        IdentifierScheme::new("com.example.camera.serial")?,
        "A-0007",
        Some("body".to_owned()),
    )?;
    {
        let mut transaction = production.begin_transaction()?;
        transaction.add_external_identifier(target, &reel)?;
        transaction.add_external_identifier(target, &serial)?;
        transaction.commit()?;
    }

    for identifier in production.external_identifiers(target)? {
        println!(
            "{} = {} ({:?})",
            identifier.scheme().as_str(),
            identifier.value(),
            identifier.qualifier()
        );
    }
    for object in
        production.find_by_external_identifier(reel.scheme(), reel.value(), reel.qualifier())?
    {
        println!("tagged with reel R-12: {object:?}");
    }

    let mut transaction = production.begin_transaction()?;
    transaction.remove_external_identifier(target, &reel)?;
    transaction.commit()
}
// [/remove-identifier]

// [typed-metadata]
fn editorial(property: &str) -> Result<MetadataProperty> {
    Ok(MetadataProperty::new(
        VocabularyId::new("https://example.com/ns/editorial/1")?,
        PropertyId::new(property)?,
    ))
}

fn add_typed_metadata(
    production: &mut SqliteProduction,
    asset_id: AssetId,
    original_id: RepresentationId,
) -> Result<()> {
    let values = [
        ("slate", MetadataValue::string("12A/3")?),
        (
            "title",
            MetadataValue::language_string("Interview", "en-US")?,
        ),
        ("offset", MetadataValue::i64(-48)),
        ("take", MetadataValue::u64(3)),
        (
            "exposure",
            MetadataValue::decimal(DecimalValue::new(125, 2)?),
        ),
        ("circled", MetadataValue::boolean(true)),
        (
            "shot-at",
            MetadataValue::timestamp(Timestamp::from_unix_micros(1_700_000_000_000_000)),
        ),
        (
            "script",
            MetadataValue::uri("https://example.com/scripts/12")?,
        ),
        (
            "thumbnail-hash",
            MetadataValue::bytes(vec![0xde, 0xad, 0xbe, 0xef])?,
        ),
        (
            "rate",
            MetadataValue::rational(RationalValue::new(24000, 1001)?),
        ),
        (
            "keywords",
            MetadataValue::list(vec![
                MetadataValue::string("interview")?,
                MetadataValue::string("exterior")?,
            ])?,
        ),
        (
            "lens",
            MetadataValue::structure(vec![
                MetadataField::new(PropertyId::new("model")?, MetadataValue::string("35mm")?),
                MetadataField::new(PropertyId::new("t-stop")?, MetadataValue::u64(2)),
            ])?,
        ),
        (
            "selected-take",
            MetadataValue::reference(ObjectRef::Representation(original_id)),
        ),
    ];
    let target = ObjectRef::Asset(asset_id);
    let mut transaction = production.begin_transaction()?;
    for (property, value) in &values {
        transaction.add_metadata_value(target, &editorial(property)?, value)?;
    }
    transaction.commit()?;
    drop(transaction);

    for assertion in production.metadata(target)? {
        let property = assertion.property().property().as_str();
        println!("{property}: {}", describe(assertion.value()));
    }
    Ok(())
}

fn describe(value: &MetadataValue) -> String {
    match value.kind() {
        MetadataValueKind::String => format!("{:?}", value.as_string()),
        MetadataValueKind::LangString => format!("{:?}", value.as_language_string()),
        MetadataValueKind::I64 => format!("{:?}", value.as_i64()),
        MetadataValueKind::U64 => format!("{:?}", value.as_u64()),
        MetadataValueKind::Decimal => format!("{:?}", value.as_decimal()),
        MetadataValueKind::Bool => format!("{:?}", value.as_bool()),
        MetadataValueKind::Timestamp => format!("{:?}", value.as_timestamp()),
        MetadataValueKind::Uri => format!("{:?}", value.as_uri()),
        MetadataValueKind::Bytes => format!("{:?}", value.as_bytes()),
        MetadataValueKind::Rational => format!("{:?}", value.as_rational()),
        MetadataValueKind::List => {
            let items = value.as_list().unwrap_or_default();
            let items: Vec<String> = items.iter().map(describe).collect();
            format!("[{}]", items.join(", "))
        }
        MetadataValueKind::Struct => {
            let fields = value.as_structure().unwrap_or_default();
            let fields: Vec<String> = fields
                .iter()
                .map(|field| format!("{}: {}", field.name().as_str(), describe(field.value())))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
        MetadataValueKind::Reference => format!("{:?}", value.as_reference()),
        // New value types may be added; preserve what cannot be interpreted.
        _ => "unsupported value type".to_owned(),
    }
}

fn objects_with_slate(production: &SqliteProduction) -> Result<Vec<ObjectRef>> {
    let query = MetadataQuery::new(editorial("slate")?, None)?;
    let mut objects = Vec::new();
    let mut cursor = None;
    loop {
        let page = production.metadata_query(&query, &QueryPageRequest::new(1, cursor)?)?;
        objects.extend(page.items().iter().map(MetadataMatch::target));
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            return Ok(objects);
        }
    }
}
// [/typed-metadata]

// [remove-metadata]
fn remove_keywords(production: &mut SqliteProduction, asset_id: AssetId) -> Result<()> {
    let keywords = MetadataProperty::new(
        VocabularyId::new("https://example.com/ns/editorial/1")?,
        PropertyId::new("keywords")?,
    );
    let target = ObjectRef::Asset(asset_id);
    {
        let mut transaction = production.begin_transaction()?;
        // Removes every value of the property at once.
        transaction.remove_metadata_property(target, &keywords)?;
        transaction.commit()?;
    }
    assert!(production.metadata_values(target, &keywords)?.is_empty());
    Ok(())
}
// [/remove-metadata]

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/fixtures/sample-media.dat")
}

#[test]
fn knowledge_examples_run_in_order() -> Result<()> {
    let work = tempfile::tempdir().expect("temporary directory");
    let rushes = work.path().join("rushes");
    fs::create_dir_all(&rushes).expect("rushes directory");
    fs::copy(fixture(), rushes.join("A001.mov")).expect("media fixture");

    let production_path = work.path().join("knowledge.pproj");
    let mut production = SqliteProduction::create(&production_path, None)?;
    let import = prepare_original_media(rushes.join("A001.mov"), None, None)?;
    let asset_id = import.asset().id();
    let original_id = import.representation().id();
    {
        let mut transaction = production.begin_transaction()?;
        transaction.import_original(&import)?;
        transaction.commit()?;
    }

    let found = find_known_media(
        &production_path,
        import.locators()[0].uri(),
        &import.resources()[0].fingerprints()[0],
    )?;
    assert_eq!(found, asset_id);

    replace_reel_identifier(&mut production, asset_id)?;
    let remaining = production.external_identifiers(ObjectRef::Asset(asset_id))?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].scheme().as_str(), "com.example.camera.serial");
    assert_eq!(remaining[0].qualifier(), Some("body"));
    let reel = IdentifierScheme::new("com.example.reel")?;
    assert!(
        production
            .find_by_external_identifier(&reel, "R-12", None)?
            .is_empty()
    );

    add_typed_metadata(&mut production, asset_id, original_id)?;
    let target = ObjectRef::Asset(asset_id);
    let stored = production.metadata(target)?;
    assert_eq!(stored.len(), 13);
    let kinds: Vec<MetadataValueKind> = stored
        .iter()
        .map(|assertion| assertion.value().kind())
        .collect();
    for kind in [
        MetadataValueKind::String,
        MetadataValueKind::LangString,
        MetadataValueKind::I64,
        MetadataValueKind::U64,
        MetadataValueKind::Decimal,
        MetadataValueKind::Bool,
        MetadataValueKind::Timestamp,
        MetadataValueKind::Uri,
        MetadataValueKind::Bytes,
        MetadataValueKind::Rational,
        MetadataValueKind::List,
        MetadataValueKind::Struct,
        MetadataValueKind::Reference,
    ] {
        assert!(kinds.contains(&kind), "missing {kind:?}");
    }
    assert_eq!(
        production.metadata_values(target, &editorial("selected-take")?)?,
        [MetadataValue::reference(ObjectRef::Representation(
            original_id
        ))]
    );
    assert_eq!(
        production.metadata_values(target, &editorial("exposure")?)?[0]
            .as_decimal()
            .map(|value| (value.coefficient(), value.scale())),
        Some((125, 2))
    );

    // A second object carrying the slate property gives the query two pages.
    {
        let mut transaction = production.begin_transaction()?;
        transaction.add_metadata_value(
            ObjectRef::Representation(original_id),
            &editorial("slate")?,
            &MetadataValue::string("12A/3")?,
        )?;
        transaction.commit()?;
    }
    let with_slate = objects_with_slate(&production)?;
    assert_eq!(with_slate.len(), 2);
    assert!(with_slate.contains(&target));
    assert!(with_slate.contains(&ObjectRef::Representation(original_id)));

    remove_keywords(&mut production, asset_id)?;
    assert_eq!(production.metadata(target)?.len(), 12);
    Ok(())
}
