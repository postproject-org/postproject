//! Local process boundaries preserve identified requests and exact wire values.

use std::{
    fs,
    process::{Command as Process, Stdio},
};

use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{ClientId, Command, Extensions, Proposal, RequestId};
use postproject_storage_sqlite::SqliteProduction;
use serde_json::Value;

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:cli:opaque").unwrap(),
        PropertyId::new("exact").unwrap(),
    )
}

fn proposal(production: &SqliteProduction, commands: Vec<Command>) -> Proposal {
    Proposal::new(
        production.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        commands,
        Extensions::default(),
    )
    .unwrap()
}

fn write_proposal(path: &std::path::Path, proposal: &Proposal) {
    fs::write(
        path,
        proposal.document().unwrap().canonical_bytes().unwrap(),
    )
    .unwrap();
}

#[test]
fn two_processes_submit_once_and_lookup_returns_the_original_exact_outcome() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority ü with spaces.pproj");
    let file = directory.path().join("proposal.json");
    let production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let request = proposal(
        &production,
        vec![Command::AppendMetadata {
            target,
            property: property(),
            value: MetadataValue::u64(u64::MAX),
        }],
    );
    write_proposal(&file, &request);
    drop(production);
    let children: Vec<_> = (0..2)
        .map(|_| {
            Process::new(env!("CARGO_BIN_EXE_postproject"))
                .args(["--json", "exchange", "submit"])
                .arg(&path)
                .arg(&file)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let results: Vec<Value> = children
        .into_iter()
        .map(|child| {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            serde_json::from_slice(&output.stdout).unwrap()
        })
        .collect();
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0]["format_version"], 1);
    assert_eq!(results[0]["outcome"]["version"], "1");
    assert_eq!(
        results[0]["outcome"]["receipt"]["revision"]["sequence"],
        "1"
    );
    let stored = SqliteProduction::open(&path).unwrap();
    assert_eq!(
        stored.metadata_values(target, &property()).unwrap(),
        [MetadataValue::u64(u64::MAX)]
    );
    assert_eq!(stored.changes_since(0, 10).unwrap().len(), 1);
    drop(stored);
    let lookup = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "outcome"])
        .arg(&path)
        .args([
            "--history",
            &request.scope().history().to_string(),
            "--client",
            &request.client().to_string(),
            "--request",
            &request.request().to_string(),
        ])
        .assert()
        .success();
    let recovered: Value = serde_json::from_slice(&lookup.get_output().stdout).unwrap();
    assert_eq!(recovered["outcome"], results[0]["outcome"]);
    let changed = Proposal::new(
        request.scope(),
        request.client(),
        request.request(),
        None,
        RevisionContext::default(),
        vec![],
        Extensions::default(),
    )
    .unwrap();
    write_proposal(&file, &changed);
    let mismatch = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .assert()
        .code(4);
    let failure: Value = serde_json::from_slice(&mismatch.get_output().stdout).unwrap();
    assert_eq!(failure["error"]["code"], "request_identity_mismatch");
    assert!(failure.get("outcome").is_none());
}

#[test]
fn retained_rejection_and_no_op_are_distinct_from_unknown_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let file = directory.path().join("proposal.json");
    let production = SqliteProduction::create(&path, None).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let request = proposal(
        &production,
        vec![
            Command::AppendMetadata {
                target,
                property: property(),
                value: MetadataValue::i64(1),
            },
            Command::RemoveMetadata {
                target,
                property: property(),
            },
        ],
    );
    write_proposal(&file, &request);
    drop(production);
    let rejection = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .assert()
        .code(2);
    let rejected: Value = serde_json::from_slice(&rejection.get_output().stdout).unwrap();
    assert_eq!(rejected["error"]["code"], "invalid_argument");
    assert_eq!(rejected["outcome"]["status"], "rejected");
    assert!(rejected["outcome"]["receipt"].is_null());
    let production = SqliteProduction::open(&path).unwrap();
    assert_eq!(
        production.metadata_values(target, &property()).unwrap(),
        Vec::<MetadataValue>::new()
    );
    assert_eq!(
        production.changes_since(0, 10).unwrap(),
        Vec::<postproject_core::Revision>::new()
    );
    let no_op = proposal(&production, vec![]);
    write_proposal(&file, &no_op);
    drop(production);
    let accepted = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .assert()
        .success();
    let accepted: Value = serde_json::from_slice(&accepted.get_output().stdout).unwrap();
    assert_eq!(accepted["outcome"]["status"], "accepted");
    assert!(accepted["outcome"]["receipt"]["revision"].is_null());
    let unknown = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "outcome"])
        .arg(&path)
        .args([
            "--history",
            &no_op.scope().history().to_string(),
            "--client",
            &no_op.client().to_string(),
            "--request",
            &RequestId::new().to_string(),
        ])
        .assert()
        .success();
    let unknown: Value = serde_json::from_slice(&unknown.get_output().stdout).unwrap();
    assert!(unknown["outcome"].is_null());
}

#[test]
fn strict_framing_and_limit_failures_precede_production_open() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("never-created.pproj");
    let file = directory.path().join("proposal.json");
    fs::write(&file, br#"{"kind":"proposal","kind":"proposal"}"#).unwrap();
    let malformed = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .assert()
        .code(2);
    let malformed: Value = serde_json::from_slice(&malformed.get_output().stdout).unwrap();
    assert_eq!(malformed["error"]["code"], "malformed");
    fs::File::create(&file)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    let bounded = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .assert()
        .code(2);
    let bounded: Value = serde_json::from_slice(&bounded.get_output().stdout).unwrap();
    assert_eq!(bounded["error"]["code"], "limit_exceeded");
    assert!(!path.exists());
}
