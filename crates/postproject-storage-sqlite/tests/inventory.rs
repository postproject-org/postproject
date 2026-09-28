//! Inventory and sidecar-cache integration coverage.

use std::{fs, path::PathBuf};

use postproject_core::{
    FrameRange, ImageSequencePattern, MediaRoot, MediaRootId, RationalRate, RepresentationKind,
};
use postproject_media::{
    ImageSequenceSource, InventoryCategory, InventoryScanner, MediaRootMapping,
    prepare_original_media, prepare_representation,
};
use postproject_storage_sqlite::SqliteProduction;

struct InventoryFixture {
    _temporary: tempfile::TempDir,
    production: SqliteProduction,
    mapping: MediaRootMapping,
    cache_path: PathBuf,
}

fn inventory_fixture() -> InventoryFixture {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let root_path = temporary.path().join("media");
    fs::create_dir(&root_path).expect("create media root");
    let production_path = temporary.path().join("inventory.pproj");
    let cache_path = temporary.path().join("cache/inventory.json");

    let online_path = root_path.join("online.mov");
    let changed_path = root_path.join("changed.mov");
    let missing_path = root_path.join("missing.mov");
    let duplicate_path = root_path.join("duplicate.mov");
    fs::write(&online_path, b"online-media").expect("write online media");
    fs::write(&changed_path, b"before-change").expect("write changed media");
    fs::write(&missing_path, b"missing-media").expect("write missing media");
    fs::write(&duplicate_path, b"duplicate-media").expect("write duplicate media");
    let online = prepare_original_media(&online_path, None, None).expect("prepare online");
    let changed = prepare_original_media(&changed_path, None, None).expect("prepare changed");
    let missing = prepare_original_media(&missing_path, None, None).expect("prepare missing");
    let duplicate = prepare_original_media(&duplicate_path, None, None).expect("prepare duplicate");

    let sequence_path = root_path.join("sequence");
    fs::create_dir(&sequence_path).expect("create sequence");
    fs::write(sequence_path.join("plate.0001.exr"), b"frame-one").expect("write frame one");
    fs::write(sequence_path.join("plate.0002.exr"), b"frame-two").expect("write frame two");
    let sequence = ImageSequenceSource::new(
        &sequence_path,
        ImageSequencePattern::new("plate.", ".exr", 4).expect("sequence pattern"),
        FrameRange::new(1, 2, 1).expect("frame range"),
        RationalRate::new(24, 1).expect("rate"),
        Vec::new(),
    );
    let sequence =
        prepare_representation(online.asset().id(), RepresentationKind::Original, sequence)
            .expect("prepare sequence");

    let mut production =
        SqliteProduction::create(&production_path, None).expect("create production");
    let mut transaction = production.begin_transaction().expect("begin import");
    transaction.import_original(&online).expect("stage online");
    transaction
        .import_original(&changed)
        .expect("stage changed");
    transaction
        .import_original(&missing)
        .expect("stage missing");
    transaction
        .import_original(&duplicate)
        .expect("stage duplicate");
    transaction
        .add_representation(&sequence)
        .expect("stage sequence");
    transaction
        .add_media_root(
            MediaRoot::new(
                MediaRootId::new(),
                "rushes",
                Some("Rushes".to_owned()),
                None,
                0,
                true,
            )
            .expect("portable root"),
        )
        .expect("stage root");
    transaction.commit().expect("commit fixture");
    drop(transaction);

    fs::write(&changed_path, b"after-change!").expect("replace changed media");
    fs::remove_file(&missing_path).expect("remove missing media");
    fs::remove_file(&duplicate_path).expect("remove duplicate original");
    fs::write(root_path.join("duplicate-a.mov"), b"duplicate-media").expect("write duplicate a");
    fs::write(root_path.join("duplicate-b.mov"), b"duplicate-media").expect("write duplicate b");
    fs::write(root_path.join("new.mov"), b"brand-new-media").expect("write new media");
    fs::remove_file(sequence_path.join("plate.0002.exr")).expect("remove frame two");

    let mapping = MediaRootMapping::new("rushes", &root_path).expect("map root");
    InventoryFixture {
        _temporary: temporary,
        production,
        mapping,
        cache_path,
    }
}

#[test]
fn inventory_reports_all_categories_and_reuses_a_disposable_cache() {
    let InventoryFixture {
        _temporary,
        production,
        mapping,
        cache_path,
    } = inventory_fixture();
    let scanner = InventoryScanner::default();
    let first = scanner
        .scan(
            &production,
            std::slice::from_ref(&mapping),
            Some(&cache_path),
        )
        .expect("first inventory");
    let categories = first
        .items()
        .iter()
        .map(postproject_media::InventoryItem::category)
        .collect::<std::collections::BTreeSet<_>>();
    for expected in [
        InventoryCategory::KnownOnline,
        InventoryCategory::Partial,
        InventoryCategory::Missing,
        InventoryCategory::NewCandidate,
        InventoryCategory::Changed,
        InventoryCategory::DuplicateCandidate,
        InventoryCategory::AmbiguousRelinkCandidate,
    ] {
        assert!(
            categories.contains(&expected),
            "missing category {expected:?}"
        );
    }
    assert!(first.stats().fingerprints_computed > 0);

    let second = scanner
        .scan(
            &production,
            std::slice::from_ref(&mapping),
            Some(&cache_path),
        )
        .expect("warm inventory");
    assert_eq!(second.items(), first.items());
    assert_eq!(second.stats().fingerprints_computed, 0);
    assert!(second.stats().fingerprint_cache_hits > 0);
    assert!(!second.stats().cache_rebuilt);

    fs::remove_file(&cache_path).expect("delete disposable cache");
    let rebuilt = scanner
        .scan(&production, &[mapping], Some(&cache_path))
        .expect("rebuilt inventory");
    assert_eq!(rebuilt.items(), first.items());
    assert!(rebuilt.stats().cache_rebuilt);
    assert!(rebuilt.stats().fingerprints_computed > 0);

    fs::write(&cache_path, b"not valid JSON").expect("corrupt cache");
    let corrupt = scanner
        .scan(&production, &[], Some(&cache_path))
        .expect("corrupt cache degrades to scan");
    assert!(corrupt.stats().cache_rebuilt);
    assert!(
        corrupt
            .items()
            .iter()
            .any(|item| item.category() == InventoryCategory::RootUnmapped)
    );
}
