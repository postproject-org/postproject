//! Real CLI processes exchange native writes through checkpoints and suffixes.

use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
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

#[test]
fn native_media_write_reconstructs_in_distinct_processes_and_rejects_mirror_writes() {
    let directory = tempfile::tempdir().unwrap();
    let authority = directory.path().join("authority ü.pproj");
    let mirror = directory.path().join("mirror ü.pproj");
    let cursor = directory.path().join("cursor.json");
    let checkpoint = directory.path().join("knowledge.ppxc");
    let changes = directory.path().join("changes.ppxd");
    let media = directory.path().join("take.mov");
    fs::write(&media, b"deterministic media identity fixture").unwrap();
    let source = SqliteProduction::create(&authority, None).unwrap();
    let original = source.exchange_head().unwrap();
    drop(source);
    let position = run(&["exchange", "position"], &[&authority, &cursor], 0);
    assert_eq!(position["head"]["sequence"], "0");
    assert_eq!(position["head"]["digest"], original.digest().to_string());
    let exported = run(
        &["exchange", "checkpoint", "export"],
        &[&authority, &checkpoint],
        0,
    );
    let validated = run(&["exchange", "checkpoint", "validate"], &[&checkpoint], 0);
    let imported = run(
        &["exchange", "checkpoint", "import"],
        &[&checkpoint, &mirror],
        0,
    );
    assert_eq!(exported, validated);
    assert_eq!(exported, imported);
    let created = run(&["media", "add"], &[&authority, &media], 0);
    let exported = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "changes", "export"])
        .arg(&authority)
        .arg(&changes)
        .arg("--from")
        .arg(&cursor)
        .assert()
        .success();
    let exported: Value = serde_json::from_slice(&exported.get_output().stdout).unwrap();
    assert_eq!(exported["head"]["sequence"], "1");
    assert_eq!(
        run(&["exchange", "changes", "apply"], &[&mirror, &changes], 0),
        exported
    );
    assert_eq!(
        run(&["exchange", "changes", "apply"], &[&mirror, &changes], 0),
        exported
    );
    let source = SqliteProduction::open(&authority).unwrap();
    let passive = SqliteProduction::open(&mirror).unwrap();
    assert_eq!(passive.assets().unwrap(), source.assets().unwrap());
    assert_eq!(
        passive.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    assert_eq!(
        passive.assets().unwrap()[0].id().to_string(),
        created["asset_id"]
    );
    let inspected = run(&["exchange", "inspect"], &[&mirror], 0);
    assert_eq!(inspected["role"], "passive_mirror");
    assert_eq!(inspected["head"], exported["head"]);
    assert_eq!(inspected["replay_status"], "available");
    assert!(
        inspected["supported_features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|feature| feature == "jobs.v1")
    );
    let rejected = run(&["media", "add"], &[&mirror, &media], 8);
    assert_eq!(rejected["error"]["code"], "unsupported");
    assert_eq!(
        SqliteProduction::open(&mirror)
            .unwrap()
            .exchange_head()
            .unwrap(),
        source.exchange_head().unwrap()
    );
}

#[test]
fn failed_output_and_import_preserve_unrelated_files_and_report_exact_categories() {
    let directory = tempfile::tempdir().unwrap();
    let authority = directory.path().join("authority.pproj");
    let output = directory.path().join("checkpoint.ppxc");
    let mirror = directory.path().join("mirror.pproj");
    SqliteProduction::create(&authority, None).unwrap();
    run(
        &["exchange", "checkpoint", "export"],
        &[&authority, &output],
        0,
    );
    let bytes = fs::read(&output).unwrap();
    let exists = run(
        &["exchange", "checkpoint", "export"],
        &[&authority, &output],
        4,
    );
    assert_eq!(exists["error"]["code"], "already_exists");
    assert_eq!(fs::read(&output).unwrap(), bytes);
    let bounded = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "checkpoint", "import"])
        .arg(&output)
        .arg(&mirror)
        .args(["--max-encoded-bytes", "1"])
        .assert()
        .code(2);
    let bounded: Value = serde_json::from_slice(&bounded.get_output().stdout).unwrap();
    assert_eq!(bounded["error"]["code"], "limit_exceeded");
    assert!(!mirror.exists());
    fs::write(&output, &bytes[..bytes.len() - 1]).unwrap();
    let truncated = run(
        &["exchange", "checkpoint", "import"],
        &[&output, &mirror],
        2,
    );
    assert_eq!(truncated["error"]["code"], "integrity");
    assert!(!mirror.exists());
    fs::write(&output, bytes).unwrap();
    fs::write(&mirror, b"unrelated destination").unwrap();
    let exists = run(
        &["exchange", "checkpoint", "import"],
        &[&output, &mirror],
        4,
    );
    assert_eq!(exists["error"]["code"], "already_exists");
    assert_eq!(fs::read(&mirror).unwrap(), b"unrelated destination");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
}

#[test]
fn foreign_positions_never_publish_a_changes_file() {
    let directory = tempfile::tempdir().unwrap();
    let authority = directory.path().join("authority.pproj");
    let other = directory.path().join("other.pproj");
    let cursor = directory.path().join("cursor.json");
    let output = directory.path().join("changes.ppxd");
    SqliteProduction::create(&authority, None).unwrap();
    SqliteProduction::create(&other, None).unwrap();
    run(&["exchange", "position"], &[&other, &cursor], 0);
    let result = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "changes", "export"])
        .arg(&authority)
        .arg(&output)
        .arg("--from")
        .arg(&cursor)
        .assert()
        .code(2);
    let result: Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(result["error"]["code"], "scope_mismatch");
    assert!(!output.exists());
}
