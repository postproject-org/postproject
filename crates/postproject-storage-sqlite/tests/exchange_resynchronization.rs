//! Explicit recovery preserves evidence; complete histories retain their floor.

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{
    ClientId, Command, Extensions, FailureKind, Proposal, ProtocolBase, RequestId,
};
use postproject_storage_sqlite::{ExchangeError, ResynchronizationLimits, SqliteProduction};

fn base(source: &SqliteProduction) -> ProtocolBase {
    ProtocolBase::new(
        source.exchange_scope().unwrap(),
        source.read_session().unwrap().decision_base(),
    )
    .unwrap()
}

fn append(source: &mut SqliteProduction, value: i64) -> Proposal {
    let request = Proposal::new(
        source.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![Command::AppendMetadata {
            target: ObjectRef::Production(source.production().id()),
            property: MetadataProperty::new(
                VocabularyId::new("urn:exact").unwrap(),
                PropertyId::new("value").unwrap(),
            ),
            value: MetadataValue::i64(value),
        }],
        Extensions::default(),
    )
    .unwrap();
    source.submit_proposal(&request).unwrap();
    request
}

fn evidence(connection: &rusqlite::Connection) -> Vec<Vec<u8>> {
    let mut facts = Vec::new();
    for sql in [
        "SELECT payload FROM exchange_effect_fragments ORDER BY revision_id, effect_position, fragment_position",
        "SELECT manifest FROM exchange_records ORDER BY sequence",
        "SELECT document FROM exchange_record_chunks ORDER BY revision_id, position, fragment_position",
        "SELECT outcome FROM exchange_outcomes ORDER BY client_id, request_id",
    ] {
        facts.extend(
            connection
                .prepare(sql)
                .unwrap()
                .query_map([], |row| row.get::<_, Vec<u8>>(0))
                .unwrap()
                .map(Result::unwrap),
        );
    }
    facts
}

#[test]
fn missing_development_predecessor_recovers_only_explicitly_without_losing_evidence_or_outcomes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let request = append(&mut source, 1);
    let outcome = source
        .submission_outcome(request.scope(), request.client(), request.request())
        .unwrap();
    let stale = base(&source);
    append(&mut source, 2);
    let original = source.exchange_floor().unwrap();
    let scope = source.exchange_scope().unwrap();
    let revisions = source.changes_since(0, 10).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute("DELETE FROM exchange_records WHERE sequence = 1", [])
        .unwrap();
    let bytes = evidence(&connection);
    let head = base(&source);
    assert!(
        matches!(source.resynchronize_exchange_history(stale, ResynchronizationLimits::default()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::InvalidBase)
    );
    connection.execute_batch("CREATE TRIGGER fail_floor BEFORE UPDATE ON exchange_history BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    assert!(
        source
            .resynchronize_exchange_history(head, ResynchronizationLimits::default())
            .is_err()
    );
    assert_eq!(source.exchange_floor().unwrap(), original);
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM exchange_prior_anchors", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(evidence(&connection), bytes);
    connection
        .execute_batch("DROP TRIGGER fail_floor;")
        .unwrap();
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    assert_eq!(source.exchange_floor().unwrap(), original);
    let recovered = source
        .resynchronize_exchange_history(head, ResynchronizationLimits::default())
        .unwrap();
    assert_eq!(
        recovered,
        postproject_protocol::Position::anchor(head).unwrap()
    );
    assert_eq!(source.exchange_scope().unwrap(), scope);
    assert_eq!(source.exchange_head().unwrap(), recovered);
    assert_eq!(source.changes_since(0, 10).unwrap(), revisions);
    assert_eq!(evidence(&connection), bytes);
    assert_eq!(
        source
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap(),
        outcome
    );
    assert_eq!(Some(source.submit_proposal(&request).unwrap()), outcome);
    assert!(
        matches!(source.record_reader(2), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::HistoryGap)
    );
    append(&mut source, 3);
    assert_eq!(
        source.record_reader(3).unwrap().manifest().predecessor(),
        recovered
    );
    drop(source);
    let mut source = SqliteProduction::open(&path).unwrap();
    assert_eq!(
        source
            .resynchronize_exchange_history(base(&source), ResynchronizationLimits::default())
            .unwrap(),
        recovered
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT anchor_digest FROM exchange_prior_anchors WHERE floor_sequence = 0",
                [],
                |row| row.get::<_, Vec<u8>>(0)
            )
            .unwrap(),
        original.digest().as_bytes()
    );
}

#[test]
fn complete_histories_keep_their_floor_and_budgets_or_non_authorities_cannot_change_it() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    append(&mut source, 1);
    let floor = source.exchange_floor().unwrap();
    let head = source.exchange_head().unwrap();
    assert!(
        matches!(source.resynchronize_exchange_history(base(&source), ResynchronizationLimits::new(1, 1).unwrap()), Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::LimitExceeded)
    );
    assert_eq!(
        source
            .resynchronize_exchange_history(base(&source), ResynchronizationLimits::default())
            .unwrap(),
        floor
    );
    assert_eq!(source.exchange_head().unwrap(), head);
    let mut read = source.read_session().unwrap().into_read_only();
    assert!(
        read.resynchronize_exchange_history(base(&source), ResynchronizationLimits::default())
            .is_err()
    );
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        floor,
    )
    .unwrap();
    assert!(
        mirror
            .resynchronize_exchange_history(
                floor.decision_base(),
                ResynchronizationLimits::default()
            )
            .is_err()
    );
    assert_eq!(mirror.exchange_floor().unwrap(), floor);
}
