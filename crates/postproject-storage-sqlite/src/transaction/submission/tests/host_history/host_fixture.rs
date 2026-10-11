use super::{Scenario, lifecycle::run};
use crate::SqliteProduction;

#[test]
#[ignore = "requires POSTPROJECT_EXCHANGE_HOST_FIXTURE from the real Blender handoff"]
fn extend_native_host_history_with_deterministic_jobs() {
    let root = std::path::PathBuf::from(
        std::env::var_os("POSTPROJECT_EXCHANGE_HOST_FIXTURE").expect("host fixture path"),
    );
    let identities: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("knowledge-identities.json")).unwrap())
            .unwrap();
    let mut source = SqliteProduction::open(root.join("authority.pproj")).unwrap();
    assert_eq!(
        source
            .jobs(
                &postproject_core::JobQuery::default(),
                &postproject_core::QueryPageRequest::new(1, None).unwrap()
            )
            .unwrap()
            .items(),
        [],
        "fresh host fixture required"
    );
    let high_water: Option<i64> = source
        .connection
        .query_row("SELECT high_water_micros FROM job_clock", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        high_water, None,
        "fixture must have no previous authority-time decisions"
    );
    let clock = std::sync::Arc::new(super::Clock::new(100));
    source.job_clock = clock.clone();
    let original = source.exchange_head().unwrap();
    run(&mut Scenario {
        asset: identities["asset"].as_str().unwrap().parse().unwrap(),
        input: identities["input"].as_str().unwrap().parse().unwrap(),
        source: &mut source,
        clock,
    });
    assert!(source.exchange_head().unwrap().sequence() > original.sequence());
    println!(
        "same host history retained: native prefix {}, complete job tail {}",
        original.sequence(),
        source.exchange_head().unwrap().sequence()
    );
}
