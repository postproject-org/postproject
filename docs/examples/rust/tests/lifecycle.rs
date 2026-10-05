//! Runs the Rust listings of the production lifecycle guide.
//!
//! Each `// [name]` ... `// [/name]` region is included verbatim by the
//! documentation build, so keep regions self-contained and readable.

use std::fs;
use std::path::{Path, PathBuf};

use postproject_core::{
    AssetId, ErrorKind, MetadataProperty, MetadataValue, ObjectRef, OriginIdentity, ProductionId,
    PropertyId, Result, RevisionContext, RevisionId, TransactionState, VocabularyId,
};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;

// [asset-identity]
fn asset_identity(saved_id: &str) -> Result<()> {
    let asset: AssetId = saved_id.parse()?;
    let ordered = std::collections::BTreeSet::from([asset, asset]);
    let hashed = std::collections::HashSet::from([asset, asset]);
    assert_eq!(ordered.len(), 1);
    assert_eq!(hashed.len(), 1);
    println!("asset identity: {asset}");
    Ok(())
}
// [/asset-identity]

// [open-production]
fn open_production(path: &Path, asset_id: AssetId) -> Result<SqliteProduction> {
    let production = SqliteProduction::open(path)?;
    println!("production: {}", production.production().id());

    // `assets` enumerates the whole set; use `assets_page` for large productions.
    let assets = production.assets()?;
    let exists = assets.iter().any(|asset| asset.id() == asset_id);
    println!("asset {asset_id} exists: {exists}");
    for asset in &assets {
        println!("asset {}: {:?}", asset.id(), asset.display_name());
    }

    if let Some(revision) = production.latest_revision()? {
        println!(
            "latest revision {}: {:?}",
            revision.sequence(),
            revision.message()
        );
    }
    Ok(production)
}
// [/open-production]

// [transaction-lifecycle]
fn commit_then_roll_back(production: &mut SqliteProduction, asset_id: AssetId) -> Result<()> {
    let status = MetadataProperty::new(
        VocabularyId::new("https://example.com/ns/editorial/1")?,
        PropertyId::new("status")?,
    );
    let target = ObjectRef::Asset(asset_id);

    let mut transaction = production.begin_transaction()?;
    transaction.set_revision_context(RevisionContext::new(
        Some(OriginIdentity::new("com.example.editor", None, None)?),
        Some("Mark as selected".to_owned()),
    )?)?;
    transaction.add_metadata_value(target, &status, &MetadataValue::string("selected")?)?;
    transaction.commit()?;
    drop(transaction);
    let committed = production.latest_revision()?.map(|revision| revision.id());

    // Rolling back discards every staged change: no revision is recorded.
    let mut transaction = production.begin_transaction()?;
    transaction.add_metadata_value(target, &status, &MetadataValue::string("rejected")?)?;
    transaction.rollback()?;
    assert_eq!(transaction.state(), TransactionState::RolledBack);
    drop(transaction);

    assert_eq!(
        production.latest_revision()?.map(|revision| revision.id()),
        committed
    );
    assert_eq!(
        production.metadata_values(target, &status)?,
        [MetadataValue::string("selected")?]
    );
    Ok(())
}
// [/transaction-lifecycle]

// [error-handling]
fn open_missing(path: &Path) -> Result<()> {
    match SqliteProduction::open(path) {
        Ok(_) => println!("opened {}", path.display()),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            println!("not found: {}", error.message());
        }
        Err(error) => return Err(error),
    }
    Ok(())
}
// [/error-handling]

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/fixtures/sample-media.dat")
}

fn create_production(path: &Path, media: &Path) -> Result<(ProductionId, AssetId, RevisionId)> {
    let mut production = SqliteProduction::create(path, Some("Documentary".to_owned()))?;
    let import = prepare_original_media(media, Some("Camera A".to_owned()), None)?;
    let asset_id = import.asset().id();
    let mut transaction = production.begin_transaction()?;
    transaction.import_original(&import)?;
    transaction.commit()?;
    drop(transaction);
    let revision = production.latest_revision()?.expect("import revision");
    Ok((production.production().id(), asset_id, revision.id()))
}

#[test]
fn lifecycle_examples_run_in_order() -> Result<()> {
    asset_identity("00000000-0000-0000-0000-000000000001")?;
    let work = tempfile::tempdir().expect("temporary directory");
    let rushes = work.path().join("rushes");
    fs::create_dir_all(&rushes).expect("rushes directory");
    fs::copy(fixture(), rushes.join("A001.mov")).expect("media fixture");
    let path = work.path().join("lifecycle.pproj");
    let (production_id, asset_id, import_revision) =
        create_production(&path, &rushes.join("A001.mov"))?;

    let mut production = open_production(&path, asset_id)?;
    assert_eq!(production.production().id(), production_id);
    assert_eq!(production.assets()?.len(), 1);
    assert_eq!(production.assets()?[0].id(), asset_id);
    assert_eq!(
        production.latest_revision()?.map(|revision| revision.id()),
        Some(import_revision)
    );

    commit_then_roll_back(&mut production, asset_id)?;
    let latest = production.latest_revision()?.expect("committed revision");
    assert_eq!(latest.sequence(), 2);
    assert_eq!(latest.message(), Some("Mark as selected"));

    let missing = work.path().join("missing.pproj");
    open_missing(&missing)?;
    let error = SqliteProduction::open(&missing).expect_err("missing production");
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(!missing.exists());
    Ok(())
}
