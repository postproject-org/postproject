//! Root decision requirements, conflicts and atomic receipt attribution.

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

fn json(args: &[&str]) -> Value {
    let result = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(args)
        .assert()
        .success();
    serde_json::from_slice(&result.get_output().stdout).unwrap()
}

#[test]
fn root_decisions_require_bases_and_preserve_stale_failures() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("roots with spaces.pproj");
    let path = path.to_str().unwrap();
    json(&["init", path]);
    let created = json(&["root", "add", path, "rushes"]);
    let root = created["id"].as_str().unwrap();
    let old = json(&["inspect", path]);
    let base = old["decision_base"].as_str().unwrap();
    for action in ["enable", "disable", "remove"] {
        let error = cargo_bin_cmd!("postproject")
            .args(["--json", "root", action, path, root])
            .assert()
            .failure();
        let value: Value = serde_json::from_slice(&error.get_output().stdout).unwrap();
        assert_eq!(value["error"]["code"], "invalid_argument");
        assert!(String::from_utf8_lossy(&error.get_output().stderr).contains("--decision-base"));
    }
    let disabled = json(&["--decision-base", base, "root", "disable", path, root]);
    assert_eq!(disabled["enabled"], false);
    assert_ne!(
        created["commit_receipt"]["revision"]["id"],
        disabled["commit_receipt"]["revision"]["id"]
    );
    let stale = cargo_bin_cmd!("postproject")
        .args([
            "--json",
            "--decision-base",
            base,
            "root",
            "remove",
            path,
            root,
        ])
        .assert()
        .failure();
    let error: Value = serde_json::from_slice(&stale.get_output().stdout).unwrap();
    let conflict = &error["error"]["transaction_conflict"];
    assert_eq!(conflict["key"]["kind"], "media_root");
    assert_eq!(conflict["key"]["target_id"], root);
    assert_eq!(
        conflict["superseding_revision_id"],
        disabled["commit_receipt"]["revision"]["id"]
    );
    assert_eq!(json(&["root", "list", path])[0]["enabled"], false);
    let fresh = json(&["inspect", path]);
    assert_eq!(
        fresh["revision_sequence"],
        disabled["commit_receipt"]["revision"]["sequence"]
    );
    let base = fresh["decision_base"].as_str().unwrap();
    let noop = json(&["--decision-base", base, "root", "disable", path, root]);
    assert!(noop["commit_receipt"]["revision"].is_null());
    let removed = json(&["--decision-base", base, "root", "remove", path, root]);
    assert!(removed["commit_receipt"]["revision"]["id"].is_string());
    assert!(json(&["root", "list", path]).as_array().unwrap().is_empty());
}
