//! Independent processes serialize identified requests under the authority lock.

use std::{
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdout, Command as ProcessCommand, Stdio},
};

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{
    ClientId, Command, Document, Extensions, Limits, Outcome, Proposal, RequestId,
};
use postproject_storage_sqlite::SqliteProduction;

const PRODUCTION: &str = "POSTPROJECT_TEST_SUBMISSION_PRODUCTION";
const REQUEST: &str = "POSTPROJECT_TEST_SUBMISSION_REQUEST";
const RESULT: &str = "POSTPROJECT_TEST_SUBMISSION_RESULT";

#[test]
fn submission_worker() {
    let Some(path) = env::var_os(PRODUCTION) else {
        return;
    };
    let mut source = SqliteProduction::open(path).unwrap();
    let bytes = fs::read(env::var_os(REQUEST).unwrap()).unwrap();
    let request =
        Proposal::from_document(&Document::parse(&bytes, Limits::default()).unwrap()).unwrap();
    println!("postproject-ready");
    std::io::stdout().flush().unwrap();
    std::io::stdin().read_exact(&mut [0]).unwrap();
    let outcome = source.submit_proposal(&request).unwrap();
    fs::write(
        env::var_os(RESULT).unwrap(),
        outcome.document().unwrap().canonical_bytes().unwrap(),
    )
    .unwrap();
}

fn ready_worker(
    production: &Path,
    request: &Path,
    result: &Path,
) -> (Child, BufReader<ChildStdout>) {
    let mut child = ProcessCommand::new(env::current_exe().unwrap())
        .args(["--exact", "submission_worker", "--nocapture"])
        .env(PRODUCTION, production)
        .env(REQUEST, request)
        .env(RESULT, result)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert!(
            output.read_line(&mut line).unwrap() > 0,
            "worker exited before ready"
        );
        if line.trim() == "postproject-ready" {
            break;
        }
    }
    (child, output)
}

fn race(production: &Path, requests: [&Path; 2], results: [&Path; 2]) -> [Outcome; 2] {
    // Both independent authority facades are open before either writer starts.
    let mut children = [
        ready_worker(production, requests[0], results[0]),
        ready_worker(production, requests[1], results[1]),
    ];
    for (child, _) in &mut children {
        child.stdin.take().unwrap().write_all(b"x").unwrap();
    }
    for (child, _) in &mut children {
        assert!(child.wait().unwrap().success());
    }
    results.map(|path| {
        Outcome::from_document(
            &Document::parse(&fs::read(path).unwrap(), Outcome::limits()).unwrap(),
        )
        .unwrap()
    })
}

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:race").unwrap(),
        PropertyId::new("values").unwrap(),
    )
}

#[test]
fn same_request_race_returns_one_original_result_and_creates_one_record() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
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
            value: MetadataValue::i64(7),
        }],
        Extensions::default(),
    )
    .unwrap();
    let request_path = directory.path().join("request.json");
    fs::write(
        &request_path,
        request.document().unwrap().canonical_bytes().unwrap(),
    )
    .unwrap();
    let result_paths = [
        directory.path().join("one.json"),
        directory.path().join("two.json"),
    ];
    let outcomes = race(
        &path,
        [&request_path, &request_path],
        [&result_paths[0], &result_paths[1]],
    );
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(source.submit_proposal(&request).unwrap(), outcomes[0]);
    assert_eq!(
        source
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap(),
        Some(outcomes[0].clone())
    );
    assert_eq!(
        source.metadata_values(target, &property()).unwrap(),
        [MetadataValue::i64(7)]
    );
    assert_eq!(source.changes_since(0, 10).unwrap().len(), 1);
    assert_eq!(
        source.record_reader(1).unwrap().manifest().effect_count(),
        1
    );
    assert_eq!(source.exchange_head().unwrap().sequence(), 1);
    let connection = rusqlite::Connection::open(&path).unwrap();
    for table in ["exchange_records", "exchange_outcomes", "revisions"] {
        assert_eq!(
            connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
