//! Explicit authority recovery preserves earlier bytes and private outcomes.

use std::path::Path;

use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{ClientId, Command, Extensions, Proposal, RequestId};
use postproject_storage_sqlite::SqliteProduction;
use serde_json::Value;

fn run(args: &[&str], paths: &[&Path], code: i32) -> Value {
    let result = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(args)
        .args(paths)
        .assert()
        .code(code);
    serde_json::from_slice(&result.get_output().stdout).unwrap()
}

fn append(source: &mut SqliteProduction, value: u64) -> Proposal {
    let request = Proposal::new(
        source.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![Command::AppendMetadata {
            target: ObjectRef::Production(source.production().id()),
            property: MetadataProperty::new(
                VocabularyId::new("urn:old:exact").unwrap(),
                PropertyId::new("value").unwrap(),
            ),
            value: MetadataValue::u64(value),
        }],
        Extensions::default(),
    )
    .unwrap();
    source.submit_proposal(&request).unwrap();
    request
}

fn evidence(path: &Path) -> Vec<Vec<u8>> {
    let connection = rusqlite::Connection::open(path).unwrap();
    let mut bytes = Vec::new();
    for sql in [
        "SELECT payload FROM exchange_effect_fragments ORDER BY revision_id, effect_position, fragment_position",
        "SELECT manifest FROM exchange_records ORDER BY sequence",
        "SELECT document FROM exchange_record_chunks ORDER BY revision_id, position, fragment_position",
        "SELECT outcome FROM exchange_outcomes ORDER BY client_id, request_id",
    ] {
        bytes.extend(
            connection
                .prepare(sql)
                .unwrap()
                .query_map([], |row| row.get::<_, Vec<u8>>(0))
                .unwrap()
                .map(Result::unwrap),
        );
    }
    bytes
}

#[test]
fn incomplete_heads_require_fresh_explicit_recovery_and_a_new_mirror_checkpoint() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let stale = directory.path().join("stale.json");
    let base = directory.path().join("fresh.json");
    let checkpoint = directory.path().join("checkpoint.ppxc");
    let mirror = directory.path().join("mirror.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let request = append(&mut source, 1);
    run(&["exchange", "history", "decision"], &[&path, &stale], 0);
    append(&mut source, 2);
    let revisions = source.changes_since(0, 10).unwrap();
    let outcome = source
        .submission_outcome(request.scope(), request.client(), request.request())
        .unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute("DELETE FROM exchange_records WHERE sequence = 2", [])
        .unwrap();
    let original_bytes = evidence(&path);
    let inspect = run(&["exchange", "inspect"], &[&path], 0);
    assert!(inspect["head"].is_null());
    assert_eq!(inspect["replay_status"], "history_gap");
    let rejected = run(
        &["exchange", "checkpoint", "export"],
        &[&path, &checkpoint],
        4,
    );
    assert_eq!(rejected["error"]["code"], "history_gap");
    assert!(!checkpoint.exists());
    let rejected = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "history", "resynchronize"])
        .arg(&path)
        .arg("--base")
        .arg(&stale)
        .assert()
        .code(2);
    let rejected: Value = serde_json::from_slice(&rejected.get_output().stdout).unwrap();
    assert_eq!(rejected["error"]["code"], "invalid_base");
    let decision = run(&["exchange", "history", "decision"], &[&path, &base], 0);
    assert_eq!(decision["base"]["sequence"], "2");
    let recovered = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "history", "resynchronize"])
        .arg(&path)
        .arg("--base")
        .arg(&base)
        .assert()
        .success();
    let recovered: Value = serde_json::from_slice(&recovered.get_output().stdout).unwrap();
    assert_eq!(recovered["floor"]["sequence"], "2");
    assert_eq!(
        recovered["floor"]["history"],
        source.exchange_scope().unwrap().history().to_string()
    );
    assert_eq!(evidence(&path), original_bytes);
    assert_eq!(source.changes_since(0, 10).unwrap(), revisions);
    assert_eq!(
        source
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap(),
        outcome
    );
    run(
        &["exchange", "checkpoint", "export"],
        &[&path, &checkpoint],
        0,
    );
    run(
        &["exchange", "checkpoint", "import"],
        &[&checkpoint, &mirror],
        0,
    );
    let passive = SqliteProduction::open(&mirror).unwrap();
    assert_eq!(
        passive.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert_eq!(passive.changes_since(0, 10).unwrap(), revisions);
    let mirror_base = directory.path().join("mirror-base.json");
    run(
        &["exchange", "history", "decision"],
        &[&mirror, &mirror_base],
        0,
    );
    let rejected = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "history", "resynchronize"])
        .arg(&mirror)
        .arg("--base")
        .arg(mirror_base)
        .assert()
        .code(8);
    let rejected: Value = serde_json::from_slice(&rejected.get_output().stdout).unwrap();
    assert_eq!(rejected["error"]["code"], "unsupported");
    assert_eq!(
        passive.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
}
