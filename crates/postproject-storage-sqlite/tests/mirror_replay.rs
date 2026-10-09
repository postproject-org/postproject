//! Real distinct files preserve authority history and recover incomplete applies.

use postproject_core::{MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId};
use postproject_protocol::{FailureKind, Limits, RecordChunk};
use postproject_storage_sqlite::{
    ExchangeError, ExchangeResult, RecordReader, ReplayLimits, SqliteProduction,
};

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:replay:test").unwrap(),
        PropertyId::new("ordered").unwrap(),
    )
}

fn stream(reader: &mut RecordReader) -> impl Iterator<Item = ExchangeResult<RecordChunk>> + '_ {
    std::iter::from_fn(|| reader.next_chunk().transpose())
}

fn apply(source: &SqliteProduction, mirror: &mut SqliteProduction, sequence: u64) -> bool {
    let mut reader = source.record_reader(sequence).unwrap();
    let manifest = reader.manifest().clone();
    mirror
        .apply_record(&manifest, stream(&mut reader), ReplayLimits::default())
        .unwrap()
}

fn first_record(source: &mut SqliteProduction) {
    let target = ObjectRef::Production(source.production().id());
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(1))
        .unwrap();
    edit.replace_metadata_values(
        target,
        &property(),
        &[MetadataValue::i64(2), MetadataValue::i64(2)],
    )
    .unwrap();
    edit.remove_metadata_property(target, &property()).unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(3))
        .unwrap();
    edit.commit().unwrap();
}

#[test]
fn same_key_effects_original_history_duplicates_and_reopen_converge() {
    let directory = tempfile::tempdir().unwrap();
    let mut source =
        SqliteProduction::create(directory.path().join("source.pproj"), Some("Source".into()))
            .unwrap();
    let path = directory.path().join("mirror.pproj");
    let mut mirror = SqliteProduction::create_genesis_mirror(
        &path,
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    first_record(&mut source);
    assert!(apply(&source, &mut mirror, 1));
    let target = ObjectRef::Production(source.production().id());
    assert_eq!(
        mirror.metadata_values(target, &property()).unwrap(),
        [MetadataValue::i64(3)]
    );
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    let revision = source.changes_since(0, 1).unwrap()[0].id();
    assert_eq!(
        mirror.events_for_revision(revision).unwrap(),
        source.events_for_revision(revision).unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert!(!apply(&source, &mut mirror, 1));
    drop(mirror);
    let mut mirror = SqliteProduction::open(&path).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(
        target,
        &property(),
        &MetadataValue::bytes(vec![7; 2 * 1024 * 1024]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    assert!(apply(&source, &mut mirror, 2));
    assert!(!apply(&source, &mut mirror, 1));
    assert_eq!(
        mirror.metadata_values(target, &property()).unwrap(),
        source.metadata_values(target, &property()).unwrap()
    );
    let connection = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = connection
        .query_row(
            "SELECT last_changed_revision_sequence FROM conflict_versions",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 2);
    assert!(mirror.begin_transaction().is_err());
}

#[test]
fn missing_chunks_budget_cancellation_and_storage_failure_leave_genesis_visible() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let path = directory.path().join("mirror.pproj");
    let anchor = source.exchange_floor().unwrap();
    let mut mirror =
        SqliteProduction::create_genesis_mirror(&path, source.production(), anchor).unwrap();
    first_record(&mut source);
    let mut reader = source.record_reader(1).unwrap();
    let manifest = reader.manifest().clone();
    assert!(
        mirror
            .apply_record(&manifest, [], ReplayLimits::default())
            .is_err()
    );
    assert_eq!(mirror.exchange_head().unwrap(), anchor);
    let budget = ReplayLimits::new(1, Limits::default()).unwrap();
    assert!(
        matches!(mirror.apply_record(&manifest, stream(&mut reader), budget), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::LimitExceeded)
    );
    let cancelled = std::iter::once(Err(postproject_protocol::ProtocolError::new(
        FailureKind::Unsupported,
        "caller cancelled",
    )
    .into()));
    assert!(
        mirror
            .apply_record(&manifest, cancelled, ReplayLimits::default())
            .is_err()
    );
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_manifest BEFORE INSERT ON exchange_records BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    let mut reader = source.record_reader(1).unwrap();
    assert!(
        mirror
            .apply_record(&manifest, stream(&mut reader), ReplayLimits::default())
            .is_err()
    );
    assert_eq!(mirror.exchange_head().unwrap(), anchor);
    assert!(mirror.changes_since(0, 10).unwrap().is_empty());
    let target = ObjectRef::Production(source.production().id());
    assert!(
        mirror
            .metadata_values(target, &property())
            .unwrap()
            .is_empty()
    );
    let staged: i64 = connection
        .query_row("SELECT count(*) FROM exchange_record_chunks", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(staged, 0);
    connection
        .execute_batch("DROP TRIGGER reject_manifest")
        .unwrap();
    assert!(apply(&source, &mut mirror, 1));
}

#[test]
fn replay_crash_worker() {
    let Some(source_path) = std::env::var_os("POSTPROJECT_REPLAY_CRASH_SOURCE") else {
        return;
    };
    let source = SqliteProduction::open(source_path).unwrap();
    let mut mirror =
        SqliteProduction::open(std::env::var_os("POSTPROJECT_REPLAY_CRASH_MIRROR").unwrap())
            .unwrap();
    let mut reader = source.record_reader(1).unwrap();
    let manifest = reader.manifest().clone();
    let after_commit = std::env::var_os("POSTPROJECT_REPLAY_CRASH_AFTER_COMMIT").is_some();
    let mut count = 0;
    let chunks = std::iter::from_fn(|| {
        if count == 1 && !after_commit {
            std::process::exit(77);
        }
        count += 1;
        reader.next_chunk().transpose()
    });
    mirror
        .apply_record(&manifest, chunks, ReplayLimits::default())
        .unwrap();
    if after_commit {
        std::process::exit(78);
    }
    panic!("worker did not reach the crash point");
}

#[test]
fn process_exit_after_chunk_staging_recovers_and_retries_the_same_record() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let mirror_path = directory.path().join("mirror.pproj");
    let mut source = SqliteProduction::create(&source_path, None).unwrap();
    let anchor = source.exchange_floor().unwrap();
    drop(
        SqliteProduction::create_genesis_mirror(&mirror_path, source.production(), anchor).unwrap(),
    );
    first_record(&mut source);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "replay_crash_worker", "--nocapture"])
        .env("POSTPROJECT_REPLAY_CRASH_SOURCE", &source_path)
        .env("POSTPROJECT_REPLAY_CRASH_MIRROR", &mirror_path)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(77));
    let mut mirror = SqliteProduction::open(&mirror_path).unwrap();
    assert_eq!(mirror.exchange_head().unwrap(), anchor);
    assert!(mirror.changes_since(0, 10).unwrap().is_empty());
    assert!(apply(&source, &mut mirror, 1));
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
}

#[test]
fn a_durable_submission_captures_and_replays_its_own_original_receipt() {
    use postproject_core::RevisionContext;
    use postproject_protocol::{ClientId, Command, Extensions, OutcomeStatus, Proposal, RequestId};
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    let target = ObjectRef::Production(source.production().id());
    let request = Proposal::new(
        source.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![Command::AppendMetadata {
            target,
            property: property(),
            value: MetadataValue::u64(u64::MAX),
        }],
        Extensions::default(),
    )
    .unwrap();
    let outcome = source.submit_proposal(&request).unwrap();
    let OutcomeStatus::Accepted(receipt) = outcome.status() else {
        panic!("submission rejected");
    };
    assert!(apply(
        &source,
        &mut mirror,
        receipt.revision().unwrap().sequence()
    ));
    assert_eq!(
        mirror.changes_since(0, 1).unwrap().first(),
        receipt.revision()
    );
    assert_eq!(
        mirror.metadata_values(target, &property()).unwrap(),
        [MetadataValue::u64(u64::MAX)]
    );
    assert_eq!(source.submit_proposal(&request).unwrap(), outcome);
    assert!(!apply(&source, &mut mirror, 1));
}

#[test]
fn process_exit_after_commit_recovers_an_identical_duplicate_without_another_revision() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("source.pproj");
    let mirror_path = directory.path().join("mirror.pproj");
    let mut source = SqliteProduction::create(&source_path, None).unwrap();
    drop(
        SqliteProduction::create_genesis_mirror(
            &mirror_path,
            source.production(),
            source.exchange_floor().unwrap(),
        )
        .unwrap(),
    );
    first_record(&mut source);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "replay_crash_worker", "--nocapture"])
        .env("POSTPROJECT_REPLAY_CRASH_SOURCE", &source_path)
        .env("POSTPROJECT_REPLAY_CRASH_MIRROR", &mirror_path)
        .env("POSTPROJECT_REPLAY_CRASH_AFTER_COMMIT", "1")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(78));
    let mut mirror = SqliteProduction::open(&mirror_path).unwrap();
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert!(!apply(&source, &mut mirror, 1));
    assert_eq!(mirror.changes_since(0, 10).unwrap().len(), 1);
}
