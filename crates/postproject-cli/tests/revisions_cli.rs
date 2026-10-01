//! CLI coverage for filtered revision pages and cross-process revision waits.

use std::process::{Command, Stdio};

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

fn run_json(arguments: &[&str]) -> Value {
    let assertion = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(arguments)
        .assert()
        .success();
    serde_json::from_slice(&assertion.get_output().stdout).expect("command emits valid JSON")
}

fn sequences(revisions: &Value) -> Vec<u64> {
    revisions
        .as_array()
        .expect("revision array")
        .iter()
        .map(|revision| revision["sequence"].as_u64().expect("sequence"))
        .collect()
}

#[test]
fn filtered_pages_return_matching_revisions_and_a_cursor() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production = directory.path().join("filtered.pproj");
    let media = directory.path().join("A001.mov");
    std::fs::write(&media, b"filtered CLI fixture").expect("write media");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    run_json(&[
        "media",
        "add",
        production,
        media.to_str().expect("UTF-8 path"),
    ]);
    run_json(&["root", "add", production, "media"]);

    let page = run_json(&[
        "revisions",
        "filtered",
        production,
        "--kind",
        "media_root_added",
        "--kind",
        "job_failed",
    ]);
    assert_eq!(sequences(&page["revisions"]), [2]);
    assert_eq!(page["through_sequence"], 2);
    let first = run_json(&[
        "revisions",
        "filtered",
        production,
        "--kind",
        "asset_imported",
        "--kind",
        "media_root_added",
        "--limit",
        "1",
    ]);
    assert_eq!(sequences(&first["revisions"]), [1]);
    assert_eq!(first["through_sequence"], 1);

    cargo_bin_cmd!("postproject")
        .args(["revisions", "filtered", production, "--kind", "row_updated"])
        .assert()
        .failure();
}

#[test]
fn wait_times_out_or_returns_revisions_from_another_process() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production = directory.path().join("wait.pproj");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let timed_out = run_json(&["revisions", "wait", production, "--timeout-ms", "0"]);
    assert_eq!(timed_out["result"], "timed_out");
    assert_eq!(
        timed_out["revisions"].as_array().expect("array").as_slice(),
        Vec::<Value>::new()
    );

    let waiting = Command::new(env!("CARGO_BIN_EXE_postproject"))
        .args(["--json", "revisions", "wait", production, "--after", "0"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn waiting CLI");
    run_json(&["root", "add", production, "media"]);
    let output = waiting.wait_with_output().expect("wait for waiting CLI");
    assert!(output.status.success());
    let waited: Value = serde_json::from_slice(&output.stdout).expect("wait emits JSON");
    assert_eq!(waited["result"], "revisions");
    assert_eq!(sequences(&waited["revisions"]), [1]);

    let latest = run_json(&["revisions", "wait", production, "--timeout-ms", "0"]);
    assert_eq!(latest["result"], "timed_out");
    cargo_bin_cmd!("postproject")
        .args(["revisions", "wait", production, "--timeout-ms", "60001"])
        .assert()
        .failure();
}
