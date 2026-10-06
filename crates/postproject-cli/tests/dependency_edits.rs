//! Dependency JSON rejects stale replacement and attributes unchanged work.

use assert_cmd::{assert::Assert, cargo::cargo_bin_cmd};
use serde_json::{Value, json};

fn command(args: &[&str]) -> Value {
    let output = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(args)
        .assert()
        .success();
    serde_json::from_slice(&output.get_output().stdout).unwrap()
}

fn record(path: &str, source: &str, spec: &str, base: Option<&str>) -> Assert {
    let mut command = cargo_bin_cmd!("postproject");
    command.arg("--json");
    if let Some(base) = base {
        command.args(["--decision-base", base]);
    }
    command
        .args(["dependency", "record", path, source, spec])
        .assert()
}

#[test]
fn stale_replacement_preserves_the_set_and_unchanged_work_has_no_revision() {
    let directory = tempfile::tempdir().unwrap();
    let production = directory.path().join("production.pproj");
    let media = directory.path().join("clip.dat");
    let spec = directory.path().join("dependencies.json");
    std::fs::write(&media, b"dependency decision fixture").unwrap();
    std::fs::write(&spec, b"[]").unwrap();
    let path = production.to_str().unwrap();
    let spec_path = spec.to_str().unwrap();
    command(&["init", path]);
    let imported = command(&["media", "add", path, media.to_str().unwrap()]);
    let source = imported["representation_id"].as_str().unwrap();
    assert!(
        record(path, source, spec_path, None)
            .failure()
            .get_output()
            .stdout
            .is_empty()
    );
    assert!(command(&["dependency", "show", path, source]).is_null());
    let inspected = command(&["inspect", path]);
    let base = inspected["decision_base"].as_str().unwrap();
    record(path, source, spec_path, Some(base)).success();
    let retained = command(&["inspect", path]);
    let old = retained["decision_base"].as_str().unwrap();
    std::fs::write(&spec, serde_json::to_vec(&json!([{
        "kind": "com.example:reference", "target": {"kind": "asset", "id": imported["asset_id"]},
        "resolved_representation_id": source, "required": true, "authored_reference": "self-reference"
    }])).unwrap()).unwrap();
    record(path, source, spec_path, Some(old)).success();
    let head = command(&["revisions", "latest", path]);
    std::fs::write(&spec, b"[]").unwrap();
    let stale = record(path, source, spec_path, Some(old)).failure();
    let error: Value = serde_json::from_slice(&stale.get_output().stdout).unwrap();
    assert_eq!(
        error["error"]["transaction_conflict"]["key"]["kind"],
        "dependency_set"
    );
    assert!(error.get("commit_receipt").is_none());
    assert_eq!(command(&["revisions", "latest", path]), head);
    assert_eq!(
        command(&["dependency", "show", path, source])["dependencies"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let refreshed = command(&["inspect", path]);
    let fresh = refreshed["decision_base"].as_str().unwrap();
    let removed = record(path, source, spec_path, Some(fresh)).success();
    let removed: Value = serde_json::from_slice(&removed.get_output().stdout).unwrap();
    assert_eq!(removed["commit_receipt"]["revision"]["sequence"], 4);
    let refreshed = command(&["inspect", path]);
    let fresh = refreshed["decision_base"].as_str().unwrap();
    let unchanged = record(path, source, spec_path, Some(fresh)).success();
    let unchanged: Value = serde_json::from_slice(&unchanged.get_output().stdout).unwrap();
    assert!(unchanged["commit_receipt"]["revision"].is_null());
}
