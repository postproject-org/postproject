//! CLI metadata decisions and commit receipt attribution.

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

fn json(args: &[&str]) -> Value {
    let output = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(args)
        .assert()
        .success();
    serde_json::from_slice(&output.get_output().stdout).unwrap()
}

#[test]
fn removal_requires_a_base_and_rejects_an_intervening_append() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("production with spaces.pproj");
    let media = directory.path().join("Média.dat");
    std::fs::write(&media, b"fixture").unwrap();
    let path = path.to_str().unwrap();
    json(&["init", path]);
    let imported = json(&["media", "add", path, media.to_str().unwrap()]);
    let asset = imported["asset_id"].as_str().unwrap();
    let namespace = "com.example.editor";
    let first = json(&[
        "metadata", "add-text", path, "asset", asset, namespace, "keywords", "first",
    ]);
    let inspection = json(&["inspect", path]);
    let base = inspection["decision_base"].as_str().unwrap();
    let second = json(&[
        "metadata", "add-text", path, "asset", asset, namespace, "keywords", "second",
    ]);
    assert_ne!(
        first["commit_receipt"]["revision"]["id"],
        second["commit_receipt"]["revision"]["id"]
    );
    assert_eq!(
        second["commit_receipt"]["production_id"],
        inspection["production_id"]
    );

    let missing = cargo_bin_cmd!("postproject")
        .args([
            "--json", "metadata", "remove", path, "asset", asset, namespace, "keywords",
        ])
        .assert()
        .failure();
    let value: Value = serde_json::from_slice(&missing.get_output().stdout).unwrap();
    assert_eq!(value["error"]["code"], "invalid_argument");
    assert!(
        std::str::from_utf8(&missing.get_output().stderr)
            .unwrap()
            .contains("requires --decision-base")
    );
    let rejected = cargo_bin_cmd!("postproject")
        .args([
            "--json",
            "--decision-base",
            base,
            "metadata",
            "remove",
            path,
            "asset",
            asset,
            namespace,
            "keywords",
        ])
        .assert()
        .failure();
    let error: Value = serde_json::from_slice(&rejected.get_output().stdout).unwrap();
    let conflict = &error["error"]["transaction_conflict"];
    assert_eq!(conflict["key"]["kind"], "metadata_property");
    assert_eq!(conflict["key"]["target_id"], asset);
    assert_eq!(
        conflict["superseding_revision_id"],
        second["commit_receipt"]["revision"]["id"]
    );
    let retained = json(&["metadata", "list", path, "asset", asset]);
    assert_eq!(retained.as_array().unwrap().len(), 2);

    let fresh = json(&["inspect", path]);
    assert_eq!(
        fresh["revision_sequence"],
        second["commit_receipt"]["revision"]["sequence"]
    );
    let removed = json(&[
        "--decision-base",
        fresh["decision_base"].as_str().unwrap(),
        "metadata",
        "remove",
        path,
        "asset",
        asset,
        namespace,
        "keywords",
    ]);
    assert!(removed["commit_receipt"]["revision"]["id"].is_string());
    assert_ne!(
        removed["commit_receipt"]["revision"]["id"],
        second["commit_receipt"]["revision"]["id"]
    );
    assert!(
        json(&["metadata", "list", path, "asset", asset])
            .as_array()
            .unwrap()
            .is_empty()
    );
}
