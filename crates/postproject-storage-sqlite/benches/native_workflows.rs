//! Identical ordinary domain workloads for released and development stores.

#[path = "native_workflows/fixture.rs"]
mod fixture;

use std::{env, fs, hint::black_box, path::Path, time::Instant};

use postproject_core::QueryPageRequest;
use postproject_storage_sqlite::SqliteProduction;

const ASSETS: usize = 10_000;
const APPENDS: usize = 1_000;

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "native_workflows create|read|append production.pproj"
    );
    let path = Path::new(&args[2]);
    let started = Instant::now();
    match args[1].as_str() {
        "create" => create(path),
        "read" => read(path),
        "append" => append(path),
        _ => panic!("unknown workload"),
    }
    println!("elapsed_ns\t{}", started.elapsed().as_nanos());
    println!("closed_store_bytes\t{}", fs::metadata(path).unwrap().len());
}

fn create(path: &Path) {
    assert!(!path.exists());
    let mut production = SqliteProduction::create(path, Some("Native benchmark".into())).unwrap();
    let mut transaction = production.begin_transaction().unwrap();
    for index in 0..ASSETS {
        transaction
            .import_original(&fixture::synthetic_import(index))
            .unwrap();
    }
    transaction.commit().unwrap();
    println!("assets_created\t{ASSETS}");
}

fn read(path: &Path) {
    let production = SqliteProduction::open(path).unwrap();
    let session = production.read_session().unwrap();
    let mut cursor = None;
    let mut assets = 0;
    let mut representations = 0;
    let mut resources = 0;
    loop {
        let page = session
            .read()
            .assets_page(&QueryPageRequest::new(1000, cursor).unwrap())
            .unwrap();
        assets += page.items().len();
        for asset in page.items() {
            for representation in session.read().representations(asset.id()).unwrap() {
                representations += 1;
                black_box(representation.fingerprints());
                for resource in session.read().resources(representation.id()).unwrap() {
                    resources += 1;
                    black_box(resource.fingerprints());
                }
            }
        }
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        (assets, representations, resources),
        (ASSETS, ASSETS, ASSETS)
    );
    println!("assets_read\t{assets}");
}

fn append(path: &Path) {
    let mut production = SqliteProduction::open(path).unwrap();
    let mut samples = Vec::with_capacity(APPENDS);
    for index in ASSETS..ASSETS + APPENDS {
        let import = fixture::synthetic_import(index);
        let started = Instant::now();
        let mut transaction = production.begin_transaction().unwrap();
        transaction.import_original(&import).unwrap();
        transaction.commit().unwrap();
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    println!("commits\t{APPENDS}");
    println!("commit_min_ns\t{}", samples[0]);
    println!("commit_median_ns\t{}", samples[APPENDS / 2]);
    println!("commit_p95_ns\t{}", samples[APPENDS * 95 / 100]);
    println!("commit_max_ns\t{}", samples[APPENDS - 1]);
}
