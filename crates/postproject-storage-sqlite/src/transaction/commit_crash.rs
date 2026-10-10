//! Parent termination at the real SQLite commit boundary, with no unwinding.

use std::{
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command as ProcessCommand, Stdio},
};

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{
    ClientId, Command, Document, Extensions, Limits, OutcomeStatus, Proposal, RequestId,
};

use crate::SqliteProduction;

const PHASE: &str = "POSTPROJECT_TEST_COMMIT_CRASH_PHASE";
const DIRECTORY: &str = "POSTPROJECT_TEST_COMMIT_CRASH_DIRECTORY";
const MODE: &str = "POSTPROJECT_TEST_COMMIT_CRASH_MODE";

pub(super) fn boundary(phase: &str) {
    if env::var(PHASE).as_deref() == Ok(phase) {
        // Only compiled into this crate's unit-test binary. The parent kills
        // this process while SQLite still owns all uncommitted writes, or
        // immediately after durability but before native acknowledgement.
        println!("\npostproject-commit-boundary");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_exact(&mut [0]).unwrap();
        panic!("parent unexpectedly resumed the crash worker");
    }
}

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:unknown:process-crash").unwrap(),
        PropertyId::new("ordered").unwrap(),
    )
}

#[test]
fn worker() {
    let Some(directory) = env::var_os(DIRECTORY) else {
        return;
    };
    let directory = Path::new(&directory);
    let mut source = SqliteProduction::open(directory.join("source.pproj")).unwrap();
    if env::var(MODE).unwrap() == "proposal" {
        let document = Document::parse(
            &fs::read(directory.join("request.json")).unwrap(),
            Limits::default(),
        )
        .unwrap();
        source
            .submit_proposal(&Proposal::from_document(&document).unwrap())
            .unwrap();
    } else {
        let target = ObjectRef::Production(source.production().id());
        let mut edit = source.begin_transaction().unwrap();
        edit.add_metadata_value(target, &property(), &MetadataValue::u64(u64::MAX))
            .unwrap();
        edit.commit().unwrap();
    }
    panic!("worker missed the requested boundary");
}

struct Worker(Child);

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn terminate(directory: &Path, mode: &str, phase: &str) {
    let mut worker = Worker(
        ProcessCommand::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "transaction::commit_crash::worker",
                "--nocapture",
            ])
            .env(DIRECTORY, directory)
            .env(MODE, mode)
            .env(PHASE, phase)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut output = BufReader::new(worker.0.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert!(
            output.read_line(&mut line).unwrap() > 0,
            "worker exited early"
        );
        if line.trim() == "postproject-commit-boundary" {
            break;
        }
    }
    worker.0.kill().unwrap();
    assert!(!worker.0.wait().unwrap().success());
}

#[test]
fn terminated_native_and_proposal_commits_recover_only_complete_original_results() {
    for mode in ["native", "proposal"] {
        for phase in ["before", "after"] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("source.pproj");
            let source = SqliteProduction::create(&path, None).unwrap();
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
            fs::write(
                directory.path().join("request.json"),
                request.document().unwrap().canonical_bytes().unwrap(),
            )
            .unwrap();
            drop(source);
            terminate(directory.path(), mode, phase);
            let mut reopened = SqliteProduction::open(&path).unwrap();
            let committed = phase == "after";
            assert_eq!(
                reopened.metadata_values(target, &property()).unwrap(),
                if committed {
                    vec![MetadataValue::u64(u64::MAX)]
                } else {
                    vec![]
                }
            );
            assert_eq!(
                reopened.exchange_head().unwrap().sequence(),
                u64::from(committed)
            );
            assert_eq!(
                reopened.changes_since(0, 10).unwrap().len(),
                usize::from(committed)
            );
            let retained = reopened
                .submission_outcome(request.scope(), request.client(), request.request())
                .unwrap();
            assert_eq!(retained.is_some(), committed && mode == "proposal");
            if let Some(original) = retained {
                let OutcomeStatus::Accepted(receipt) = original.status() else {
                    panic!("changing proposal rejected");
                };
                assert_eq!(receipt.revision().unwrap().sequence(), 1);
                assert_eq!(reopened.submit_proposal(&request).unwrap(), original);
                assert_eq!(reopened.changes_since(0, 10).unwrap().len(), 1);
                assert_eq!(
                    reopened.metadata_values(target, &property()).unwrap(),
                    [MetadataValue::u64(u64::MAX)]
                );
            } else if mode == "proposal" {
                reopened.submit_proposal(&request).unwrap();
                assert_eq!(reopened.exchange_head().unwrap().sequence(), 1);
            }
            let connection = rusqlite::Connection::open(&path).unwrap();
            let expected = i64::from(committed || mode == "proposal");
            for table in ["revisions", "exchange_records"] {
                assert_eq!(
                    connection
                        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                            .get::<_, i64>(0))
                        .unwrap(),
                    expected
                );
            }
        }
    }
}
