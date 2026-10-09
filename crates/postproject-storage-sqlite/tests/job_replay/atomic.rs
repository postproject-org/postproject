use std::time::Duration;

use postproject_core::{
    JobState, MetadataProperty, MetadataValue, ObjectRef, PropertyId, ToolIdentity, VocabularyId,
};
use postproject_storage_sqlite::SqliteProduction;

use super::{request, support};

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:exact").unwrap(),
        PropertyId::new("Exact").unwrap(),
    )
}

#[test]
fn handled_late_request_failure_preserves_other_effects_and_allows_the_same_id_retry() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let media = support::media(1, false);
    let job = request(media.asset().id(), media.representation().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&media).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_input BEFORE INSERT ON job_inputs BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(7))
        .unwrap();
    assert!(edit.request_job(&job).is_err());
    edit.commit().unwrap();
    drop(edit);
    assert_eq!(source.exchange_head().unwrap().sequence(), 2);
    assert_eq!(
        source.metadata_values(target, &property()).unwrap(),
        [MetadataValue::i64(7)]
    );
    assert!(source.job(job.id()).is_err());
    for table in ["jobs", "job_inputs"] {
        assert_eq!(
            connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    connection
        .execute_batch("DROP TRIGGER reject_input;")
        .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.request_job(&job).unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert_eq!(source.job(job.id()).unwrap(), job);
    assert_eq!(
        source
            .record_reader(3)
            .unwrap()
            .manifest()
            .predecessor()
            .sequence(),
        2
    );
}

#[test]
fn handled_capture_failure_rolls_back_the_claim_and_its_original_observation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let media = support::media(1, false);
    let job = request(media.asset().id(), media.representation().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&media).unwrap();
    edit.request_job(&job).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER corrupt_claim AFTER UPDATE OF state ON jobs WHEN NEW.state = 2 BEGIN UPDATE jobs SET kind = 'not-namespaced' WHERE id = NEW.id; END;").unwrap();
    let target = ObjectRef::Production(source.production().id());
    let tool = ToolIdentity::new("Exact worker", None, None).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(7))
        .unwrap();
    assert!(
        edit.claim_job_lease(job.id(), &tool, None, Duration::from_secs(3600))
            .is_err()
    );
    edit.commit().unwrap();
    drop(edit);
    assert_eq!(source.job(job.id()).unwrap(), job);
    assert!(matches!(
        source.job(job.id()).unwrap().state(),
        JobState::Requested
    ));
    assert_eq!(source.exchange_head().unwrap().sequence(), 2);
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM revision_events WHERE kind = 21",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM jobs WHERE claim_id IS NOT NULL",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    connection
        .execute_batch("DROP TRIGGER corrupt_claim;")
        .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    let lease = edit
        .claim_job_lease(job.id(), &tool, None, Duration::from_secs(3600))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert!(lease.export_token().is_ok());
    assert_eq!(
        source
            .record_reader(3)
            .unwrap()
            .manifest()
            .predecessor()
            .sequence(),
        2
    );
}
