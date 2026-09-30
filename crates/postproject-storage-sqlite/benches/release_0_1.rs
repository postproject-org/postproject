//! Informational performance baselines for the 0.1 release workloads.

#![allow(
    missing_docs,
    reason = "Criterion generates public harness entry points with no public API"
)]

use std::{fs, hint::black_box, path::PathBuf};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use postproject_core::{
    Asset, AssetId, ContentStructure, Locator, LocatorAvailability, LocatorId, OriginalMediaImport,
    Representation, RepresentationFingerprint, RepresentationId, RepresentationKind, Resource,
    ResourceFingerprint, ResourceId, Timestamp,
};
use postproject_media::{MediaResolver, prepare_media_root, prepare_original_media};
use postproject_storage_sqlite::SqliteProduction;
use tempfile::TempDir;

const BULK_IMPORT_COUNT: usize = 1_000;
const LARGE_PROJECT_ASSET_COUNT: usize = 10_000;
const RESOLVER_ENTRY_COUNT: usize = 3_000;

fn benchmarks(criterion: &mut Criterion) {
    benchmark_bulk_import(criterion);
    benchmark_large_production_load(criterion);
    benchmark_resolver_scan(criterion);
    benchmark_transaction_commit(criterion);
    benchmark_conflict_checks(criterion);
}

fn benchmark_bulk_import(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("media_import");
    group.sample_size(10);
    group.bench_function("1000_small_files", |bencher| {
        bencher.iter_batched(
            || media_fixture(BULK_IMPORT_COUNT, "bulk"),
            |(directory, media)| {
                let mut production =
                    SqliteProduction::create(directory.path().join("bulk.pproj"), None)
                        .expect("create benchmark production");
                let prepared: Vec<_> = media
                    .iter()
                    .map(|path| {
                        prepare_original_media(path, None, Some("benchmark".to_owned()))
                            .expect("prepare benchmark import")
                    })
                    .collect();
                let mut transaction = production
                    .begin_transaction()
                    .expect("begin bulk-import transaction");
                for import in &prepared {
                    transaction
                        .import_original(import)
                        .expect("stage benchmark import");
                }
                transaction.commit().expect("commit benchmark imports");
                drop(transaction);
                black_box(production);
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn benchmark_large_production_load(criterion: &mut Criterion) {
    let directory = tempfile::tempdir().expect("create large-production fixture");
    let path = directory.path().join("large.pproj");
    let mut production = SqliteProduction::create(&path, None).expect("create large production");
    let mut transaction = production
        .begin_transaction()
        .expect("begin large-production transaction");
    for index in 0..LARGE_PROJECT_ASSET_COUNT {
        transaction
            .import_original(&synthetic_import(index))
            .expect("stage synthetic import");
    }
    transaction
        .commit()
        .expect("commit large-production fixture");
    drop(transaction);
    drop(production);

    criterion.bench_function(
        "production_load/10000_assets_with_fingerprints",
        |bencher| {
            bencher.iter(|| {
                let production = SqliteProduction::open(&path).expect("open benchmark production");
                let assets = production.assets().expect("enumerate benchmark assets");
                for asset in &assets {
                    for representation in production
                        .representations(asset.id())
                        .expect("load benchmark representations")
                    {
                        black_box(representation.fingerprints());
                        for resource in production
                            .resources(representation.id())
                            .expect("load benchmark resources")
                        {
                            black_box(resource.fingerprints());
                        }
                    }
                }
                black_box(assets);
            });
        },
    );
}

fn benchmark_resolver_scan(criterion: &mut Criterion) {
    let directory = tempfile::tempdir().expect("create resolver fixture");
    let original = directory.path().join("original.mov");
    let matching_bytes = vec![42_u8; 4_096];
    fs::write(&original, &matching_bytes).expect("write original media");
    let import = prepare_original_media(&original, None, None).expect("prepare original media");
    fs::remove_file(&original).expect("remove known locator target");

    let root_path = directory.path().join("search-root");
    fs::create_dir(&root_path).expect("create resolver root");
    for index in 0..RESOLVER_ENTRY_COUNT {
        let mut decoy = vec![0_u8; matching_bytes.len()];
        decoy[..8].copy_from_slice(
            &u64::try_from(index)
                .expect("resolver fixture index fits u64")
                .to_le_bytes(),
        );
        fs::write(root_path.join(format!("decoy-{index:04}.mov")), decoy)
            .expect("write resolver decoy");
    }
    fs::write(root_path.join("relocated.mov"), matching_bytes).expect("write relocated media");
    let root = prepare_media_root(&root_path, None, 0).expect("prepare resolver root");
    let resolver = MediaResolver::default();

    criterion.bench_function("media_resolve/3000_candidates", |bencher| {
        bencher.iter(|| {
            black_box(
                resolver
                    .resolve_resource(
                        &import.resources()[0],
                        import.representation().content_structure(),
                        import.locators(),
                        std::slice::from_ref(&root),
                        &[],
                    )
                    .expect("resolve benchmark media"),
            )
        });
    });
}

fn benchmark_transaction_commit(criterion: &mut Criterion) {
    let directory = tempfile::tempdir().expect("create transaction fixture");
    let mut production =
        SqliteProduction::create(directory.path().join("transactions.pproj"), None)
            .expect("create transaction production");
    let mut index = 0_usize;

    criterion.bench_function("transaction_commit/single_import", |bencher| {
        bencher.iter(|| {
            let import = synthetic_import(index);
            index = index.wrapping_add(1);
            let mut transaction = production
                .begin_transaction()
                .expect("begin benchmark transaction");
            transaction
                .import_original(&import)
                .expect("stage benchmark mutation");
            transaction.commit().expect("commit benchmark transaction");
        });
    });
}

fn benchmark_conflict_checks(criterion: &mut Criterion) {
    let (directory, mut unbased, unbased_resources) = conflict_fixture("unbased");
    let mut unbased_index = 0_u64;
    let mut group = criterion.benchmark_group("transaction_commit/100_locator_keys");
    group.sample_size(20);
    group.bench_function("without_base", |bencher| {
        bencher.iter(|| {
            commit_locator_batch(&mut unbased, &unbased_resources, unbased_index, None);
            unbased_index = unbased_index.wrapping_add(1);
        });
    });

    let (_based_directory, mut based, based_resources) = conflict_fixture("based");
    let mut based_index = 0_u64;
    group.bench_function("with_current_base", |bencher| {
        bencher.iter(|| {
            let base = based
                .latest_revision()
                .expect("load benchmark base")
                .expect("benchmark base revision")
                .id();
            commit_locator_batch(&mut based, &based_resources, based_index, Some(base));
            based_index = based_index.wrapping_add(1);
        });
    });
    group.finish();
    black_box(directory);
}

fn conflict_fixture(name: &str) -> (TempDir, SqliteProduction, Vec<ResourceId>) {
    const KEY_COUNT: usize = 100;
    let directory = tempfile::tempdir().expect("create conflict fixture");
    let mut production =
        SqliteProduction::create(directory.path().join(format!("{name}.pproj")), None)
            .expect("create conflict production");
    let imports: Vec<_> = (0..KEY_COUNT).map(synthetic_import).collect();
    let resources = imports
        .iter()
        .map(|import| import.resources()[0].id())
        .collect();
    let mut transaction = production
        .begin_transaction()
        .expect("begin conflict fixture");
    for import in &imports {
        transaction
            .import_original(import)
            .expect("stage conflict fixture import");
    }
    transaction.commit().expect("commit conflict fixture");
    drop(transaction);
    (directory, production, resources)
}

fn commit_locator_batch(
    production: &mut SqliteProduction,
    resources: &[ResourceId],
    iteration: u64,
    base: Option<postproject_core::RevisionId>,
) {
    let mut transaction = match base {
        Some(base) => production.begin_transaction_at(base),
        None => production.begin_transaction(),
    }
    .expect("begin locator batch");
    for (position, resource_id) in resources.iter().enumerate() {
        let locator = Locator::new(
            LocatorId::new(),
            *resource_id,
            format!("file:///benchmark/conflicts/{iteration}/{position}.mov"),
            None,
            LocatorAvailability::Unknown,
        )
        .expect("construct conflict benchmark locator");
        transaction
            .add_locator(&locator)
            .expect("stage conflict benchmark locator");
    }
    transaction.commit().expect("commit locator batch");
}

fn media_fixture(count: usize, prefix: &str) -> (TempDir, Vec<PathBuf>) {
    let directory = tempfile::tempdir().expect("create media fixture");
    let mut paths = Vec::with_capacity(count);
    for index in 0..count {
        let path = directory.path().join(format!("{prefix}-{index:04}.mov"));
        fs::write(&path, format!("fixture media {index}")).expect("write media fixture");
        paths.push(path);
    }
    (directory, paths)
}

fn synthetic_import(index: usize) -> OriginalMediaImport {
    let asset = Asset::new(
        AssetId::new(),
        Timestamp::from_unix_micros(i64::try_from(index).expect("benchmark index fits i64")),
        None,
        Some("benchmark".to_owned()),
    );
    let resource_id = ResourceId::new();
    let fingerprint_value = u64::try_from(index)
        .expect("benchmark index fits u64")
        .to_le_bytes()
        .to_vec();
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        ContentStructure::single_resource(resource_id),
        vec![
            RepresentationFingerprint::new(
                "benchmark-representation",
                1,
                fingerprint_value.clone(),
            )
            .expect("construct benchmark representation fingerprint"),
        ],
    );
    let resource = Resource::new(
        resource_id,
        vec![
            ResourceFingerprint::new("benchmark-resource", 1, fingerprint_value)
                .expect("construct benchmark resource fingerprint"),
        ],
        None,
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource_id,
        format!("file:///benchmark/{index}.mov"),
        None,
        LocatorAvailability::Unknown,
    )
    .expect("construct benchmark locator");
    OriginalMediaImport::new(asset, representation, vec![resource], vec![locator])
        .expect("construct benchmark import")
}

criterion_group!(release_0_1, benchmarks);
criterion_main!(release_0_1);
