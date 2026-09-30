//! Bounded domain-query timings on the 0.4 release fixture.
//!
//! Every row measures one page of a named query. The page size is fixed, so a
//! query backed by an index that makes it proportional to its result reports a
//! time that does not grow with the 10,000-asset production.

use std::{env, hint::black_box, path::PathBuf, time::Instant};

use postproject_core::{
    ActivityKind, ActivityOutputQuery, ArtifactEvaluationLimits, AssetId, LocatorIdentity,
    MetadataProperty, MetadataQuery, MetadataValue, PropertyId, ProvenanceQueryLimits, QueryPage,
    QueryPageRequest, RepresentationId, ResourceFingerprint, ResourceId, StaleArtifactQuery,
    ToolIdentity, VocabularyId,
};
use postproject_storage_sqlite::SqliteProduction;

const ASSET_COUNT: u64 = 10_000;
const REPRESENTATIONS_PER_ASSET: u64 = 10;
const REVISION_COUNT: u64 = 100_000;
const PAGE_SIZE: u32 = 100;

#[allow(
    clippy::too_many_lines,
    reason = "one sequential list keeps every named query's measured page visible together"
)]
fn main() {
    let path = env::var_os("POSTPROJECT_BENCH_FIXTURE").map_or_else(
        || {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../target/bench-fixtures/release-0.4.pproj"
            ))
        },
        PathBuf::from,
    );
    let seed = env::var("POSTPROJECT_BENCH_SEED").unwrap_or_else(|_| "postproject-0.4".into());
    let runs = env::var("POSTPROJECT_BENCH_RUNS").map_or(5, |value| {
        value.parse().expect("POSTPROJECT_BENCH_RUNS is an integer")
    });
    assert!(runs > 0, "POSTPROJECT_BENCH_RUNS must be positive");
    assert!(path.is_file(), "generate {} first", path.display());

    let representation = |index: u64| RepresentationId::from_bytes(stable_id(&seed, b'p', index));
    let last_asset = AssetId::from_bytes(stable_id(&seed, b'a', ASSET_COUNT - 1));
    let last_representation = representation(ASSET_COUNT * REPRESENTATIONS_PER_ASSET - 2);
    let last_resource = ResourceId::from_bytes(stable_id(
        &seed,
        b'r',
        (ASSET_COUNT * REPRESENTATIONS_PER_ASSET - 2) * 2,
    ));
    let property = MetadataProperty::new(
        VocabularyId::new("https://postproject.org/ns/benchmark").expect("fixture vocabulary"),
        PropertyId::new("property-000").expect("fixture property"),
    );
    let provenance_limits = ProvenanceQueryLimits::new(64, 1_000).expect("provenance bounds");
    let evaluation_limits = ArtifactEvaluationLimits::new(64, 1_000).expect("evaluation bounds");
    let page = QueryPageRequest::new(PAGE_SIZE, None).expect("page request");

    println!("fixture\t{}", path.display());
    println!("seed\t{seed}");
    println!("runs\t{runs}");
    println!("page_size\t{PAGE_SIZE}");
    measure("assets", runs, &path, |production| {
        production.assets_page(&page)
    });
    measure("representations_of_asset", runs, &path, |production| {
        production.representations_page(last_asset, &page)
    });
    measure("resources_of_representation", runs, &path, |production| {
        production.resources_page(last_representation, &page)
    });
    measure("locators_of_resource", runs, &path, |production| {
        production.locators_page(last_resource, &page)
    });
    let known_locator =
        LocatorIdentity::new("root://media/099998/0.mov", None).expect("known locator identity");
    measure("known_media_by_locator", runs, &path, |production| {
        production.find_known_media_by_locator(&known_locator, &page)
    });
    let known_resource_index = (ASSET_COUNT * REPRESENTATIONS_PER_ASSET - 2) * 2;
    let known_fingerprint = ResourceFingerprint::new(
        "pp-blake3-full-file",
        1,
        blake3::hash(format!("{seed}:resource:{known_resource_index}").as_bytes())
            .as_bytes()
            .to_vec(),
    )
    .expect("known resource fingerprint");
    measure("known_media_by_fingerprint", runs, &path, |production| {
        production.find_known_media_by_fingerprint(&known_fingerprint, &page)
    });
    measure(
        "representations_under_media_root",
        runs,
        &path,
        |production| production.representations_under_media_root("media", &page),
    );
    measure("unresolved_media", runs, &path, |production| {
        production.unresolved_media(&page)
    });
    let property_query = MetadataQuery::new(property.clone(), None).expect("metadata query");
    measure("metadata_property", runs, &path, |production| {
        production.metadata_query(&property_query, &page)
    });
    let value_query =
        MetadataQuery::new(property, Some(MetadataValue::u64((ASSET_COUNT - 1) * 100)))
            .expect("metadata value query");
    measure("metadata_property_value", runs, &path, |production| {
        production.metadata_query(&value_query, &page)
    });
    let by_kind = ActivityOutputQuery::Kind(
        ActivityKind::new("org.postproject:transcode").expect("activity kind"),
    );
    measure("outputs_by_activity_kind", runs, &path, |production| {
        production.activity_outputs(&by_kind, &page)
    });
    let by_tool = ActivityOutputQuery::Tool(
        ToolIdentity::new("ffmpeg", Some("7.1".into()), None).expect("tool identity"),
    );
    measure("outputs_by_tool", runs, &path, |production| {
        production.activity_outputs(&by_tool, &page)
    });
    measure("activities_producing", runs, &path, |production| {
        production.activities_producing_page(representation(2_100), &page)
    });
    measure("activities_consuming", runs, &path, |production| {
        production.activities_consuming_page(representation(1_000), &page)
    });
    measure("ancestors_depth_50", runs, &path, |production| {
        production.ancestors_page(representation(50), provenance_limits, &page)
    });
    measure("descendants_fan_out", runs, &path, |production| {
        production.descendants_page(representation(1_000), provenance_limits, &page)
    });
    let stale = StaleArtifactQuery::new(None, evaluation_limits);
    measure("stale_artifacts", runs, &path, |production| {
        production.stale_artifacts(stale, &page)
    });
    let stale_from_source = StaleArtifactQuery::new(
        Some(representation(
            (ASSET_COUNT - 100) * REPRESENTATIONS_PER_ASSET,
        )),
        evaluation_limits,
    );
    measure("stale_artifacts_from_source", runs, &path, |production| {
        production.stale_artifacts(stale_from_source, &page)
    });
    measure("objects_changed_last_1000", runs, &path, |production| {
        production.objects_changed_since(REVISION_COUNT - 1_000, &page)
    });
}

fn measure<T>(
    name: &str,
    runs: usize,
    path: &PathBuf,
    mut operation: impl FnMut(&SqliteProduction) -> postproject_core::Result<QueryPage<T>>,
) {
    let mut samples = Vec::with_capacity(runs);
    let mut items = 0;
    let mut has_next = false;
    for _ in 0..runs {
        let production = SqliteProduction::open(path).expect("open fixture");
        let started = Instant::now();
        let page = operation(&production).expect("query page");
        samples.push(started.elapsed());
        items = page.items().len();
        has_next = page.next_cursor().is_some();
        black_box(page);
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2].as_secs_f64() * 1_000.0;
    let minimum = samples[0].as_secs_f64() * 1_000.0;
    let maximum = samples[samples.len() - 1].as_secs_f64() * 1_000.0;
    println!(
        "{name}\titems={items}\tnext={has_next}\tmedian={median:.3} ms\tmin={minimum:.3} ms\tmax={maximum:.3} ms"
    );
}

fn stable_id(seed: &str, domain: u8, index: u64) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(seed.as_bytes());
    hasher.update(&[domain]);
    hasher.update(&index.to_be_bytes());
    let mut id = [0; 16];
    id.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    id[6] = (id[6] & 0x0f) | 0x40;
    id[8] = (id[8] & 0x3f) | 0x80;
    id
}
