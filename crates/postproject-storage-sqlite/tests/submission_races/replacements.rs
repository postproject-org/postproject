use super::*;
use postproject_protocol::{OutcomeStatus, ProtocolBase, RejectionKind};

#[test]
fn distinct_replacements_at_one_base_commit_one_winner_and_retain_one_conflict() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(source.production().id());
    let mut edit = source.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property(), &MetadataValue::i64(0))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let scope = source.exchange_scope().unwrap();
    let base = ProtocolBase::new(scope, source.read_session().unwrap().decision_base()).unwrap();
    let requests = [1, 2].map(|value| {
        Proposal::new(
            scope,
            ClientId::new(),
            RequestId::new(),
            Some(base),
            RevisionContext::default(),
            vec![Command::ReplaceMetadata {
                target,
                property: property(),
                values: vec![MetadataValue::i64(value)],
            }],
            Extensions::default(),
        )
        .unwrap()
    });
    let paths = [
        directory.path().join("one-request.json"),
        directory.path().join("two-request.json"),
    ];
    for (request, path) in requests.iter().zip(&paths) {
        fs::write(path, request.document().unwrap().canonical_bytes().unwrap()).unwrap();
    }
    let results = [
        directory.path().join("one-result.json"),
        directory.path().join("two-result.json"),
    ];
    let outcomes = race(&path, [&paths[0], &paths[1]], [&results[0], &results[1]]);
    let mut winners = 0;
    let mut conflicts = 0;
    for (index, outcome) in outcomes.iter().enumerate() {
        assert_eq!(source.submit_proposal(&requests[index]).unwrap(), *outcome);
        match outcome.status() {
            OutcomeStatus::Accepted(receipt) => {
                winners += 1;
                assert_eq!(receipt.revision().unwrap().sequence(), 2);
                assert_eq!(
                    source.metadata_values(target, &property()).unwrap(),
                    [MetadataValue::i64(if index == 0 { 1 } else { 2 })]
                );
            }
            OutcomeStatus::Rejected(rejection) => {
                conflicts += 1;
                assert_eq!(
                    rejection.kind(),
                    RejectionKind::Domain(postproject_core::ErrorKind::Conflict)
                );
                assert_eq!(rejection.conflict().unwrap().base_sequence(), 1);
            }
            _ => panic!("unsupported terminal result"),
        }
    }
    assert_eq!((winners, conflicts), (1, 1));
    assert_eq!(source.exchange_head().unwrap().sequence(), 2);
    let connection = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM exchange_outcomes", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM exchange_records", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn additive_append_race_merges_in_commit_order_and_advances_destructive_guards() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let scope = source.exchange_scope().unwrap();
    let target = ObjectRef::Production(scope.production());
    let base = ProtocolBase::new(scope, source.read_session().unwrap().decision_base()).unwrap();
    let requests = [1, 2].map(|value| {
        Proposal::new(
            scope,
            ClientId::new(),
            RequestId::new(),
            Some(base),
            RevisionContext::default(),
            vec![Command::AppendMetadata {
                target,
                property: property(),
                value: MetadataValue::i64(value),
            }],
            Extensions::default(),
        )
        .unwrap()
    });
    let paths = [
        directory.path().join("one-request.json"),
        directory.path().join("two-request.json"),
    ];
    for (request, path) in requests.iter().zip(&paths) {
        fs::write(path, request.document().unwrap().canonical_bytes().unwrap()).unwrap();
    }
    let results = [
        directory.path().join("one-result.json"),
        directory.path().join("two-result.json"),
    ];
    let outcomes = race(&path, [&paths[0], &paths[1]], [&results[0], &results[1]]);
    let mut committed = std::collections::BTreeMap::new();
    for (index, outcome) in outcomes.iter().enumerate() {
        let OutcomeStatus::Accepted(receipt) = outcome.status() else {
            panic!("additive append rejected");
        };
        committed.insert(
            receipt.revision().unwrap().sequence(),
            MetadataValue::i64(if index == 0 { 1 } else { 2 }),
        );
        assert_eq!(source.submit_proposal(&requests[index]).unwrap(), *outcome);
    }
    assert_eq!(
        source.metadata_values(target, &property()).unwrap(),
        committed.into_values().collect::<Vec<_>>()
    );
    assert_eq!(source.exchange_head().unwrap().sequence(), 2);
    let replacement = Proposal::new(
        scope,
        ClientId::new(),
        RequestId::new(),
        Some(base),
        RevisionContext::default(),
        vec![Command::ReplaceMetadata {
            target,
            property: property(),
            values: vec![],
        }],
        Extensions::default(),
    )
    .unwrap();
    let rejected = source.submit_proposal(&replacement).unwrap();
    assert!(
        matches!(rejected.status(), OutcomeStatus::Rejected(rejection)
        if rejection.kind() == RejectionKind::Domain(postproject_core::ErrorKind::Conflict))
    );
    assert_eq!(source.exchange_head().unwrap().sequence(), 2);
    assert_eq!(
        source.metadata_values(target, &property()).unwrap().len(),
        2
    );
}
