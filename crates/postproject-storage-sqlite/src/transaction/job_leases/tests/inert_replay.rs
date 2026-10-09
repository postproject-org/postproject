use super::*;
use crate::ReplayLimits;

#[derive(Debug)]
struct UnavailableClock;

impl JobClock for UnavailableClock {
    fn now(&self) -> Result<Timestamp> {
        Err(Error::new(
            ErrorKind::Internal,
            "mirror must never consult this clock",
        ))
    }
}

#[test]
fn expired_claims_and_exact_reacquisition_replay_without_authority_time() {
    let mut fixture = Fixture::new();
    let old = fixture.claim();
    let first_expiry = match fixture.store.job(fixture.job).unwrap().state() {
        JobState::Claimed(claim) => claim.expires_at(),
        _ => panic!("lost original claim"),
    };
    fixture.clock.set(first_expiry.as_unix_micros());
    let tool = ToolIdentity::new("Different worker", None, None).unwrap();
    let mut edit = fixture.store.begin_transaction().unwrap();
    let new = edit
        .claim_job_lease(fixture.job, &tool, None, Duration::from_micros(10))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert!(
        fixture
            .store
            .import_job_lease(&old.export_token().unwrap())
            .is_err()
    );
    let mut mirror = SqliteProduction::create_genesis_mirror(
        fixture.directory.path().join("mirror.pproj"),
        fixture.store.production(),
        fixture.store.exchange_floor().unwrap(),
    )
    .unwrap();
    mirror.job_clock = Arc::new(UnavailableClock);
    // Both observed expiries are long before the real wall clock. Replaying the
    // source-selected transition must neither reject nor expire those facts.
    for sequence in 1..=3 {
        let mut reader = fixture.store.record_reader(sequence).unwrap();
        let manifest = reader.manifest().clone();
        mirror
            .apply_record(
                &manifest,
                std::iter::from_fn(|| reader.next_chunk().transpose()),
                ReplayLimits::default(),
            )
            .unwrap();
    }
    assert_eq!(
        mirror.job(fixture.job).unwrap(),
        fixture.store.job(fixture.job).unwrap()
    );
    assert!(
        mirror
            .import_job_lease(&new.export_token().unwrap())
            .is_err()
    );
    assert_eq!(
        mirror
            .connection
            .query_row("SELECT high_water_micros FROM job_clock", [], |row| row
                .get::<_, Option<
                i64,
            >>(
                0
            ))
            .unwrap(),
        None
    );
}
