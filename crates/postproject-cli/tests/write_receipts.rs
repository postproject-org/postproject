//! Receipts identify each command's commit, including no-change observations.

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
fn media_and_identifier_receipts_keep_their_own_revision() {
    let directory = tempfile::tempdir().unwrap();
    let production = directory.path().join("production.pproj");
    let media = directory.path().join("clip.dat");
    std::fs::write(&media, b"receipt fixture").unwrap();
    let path = production.to_str().unwrap();
    let created = json(&["init", path]);
    let imported = json(&["media", "add", path, media.to_str().unwrap()]);
    let receipt = &imported["commit_receipt"];
    assert_eq!(receipt["production_id"], created["id"]);
    assert_eq!(receipt["revision"]["sequence"], 1);
    let asset = imported["asset_id"].as_str().unwrap();
    let attached = json(&[
        "identifier",
        "add",
        path,
        "asset",
        asset,
        "https://example.com/clip",
        "clip-one",
    ]);
    assert_eq!(attached["commit_receipt"]["production_id"], created["id"]);
    assert_eq!(attached["commit_receipt"]["revision"]["sequence"], 2);
    assert_ne!(
        receipt["revision"]["id"],
        attached["commit_receipt"]["revision"]["id"]
    );
    let inspection = json(&["revisions", "latest", path]);
    assert_eq!(
        inspection["id"],
        attached["commit_receipt"]["revision"]["id"]
    );
    assert_eq!(receipt["revision"]["sequence"], 1);
    let duplicate = cargo_bin_cmd!("postproject")
        .args([
            "--json",
            "identifier",
            "add",
            path,
            "asset",
            asset,
            "https://example.com/clip",
            "clip-one",
        ])
        .assert()
        .failure();
    assert!(duplicate.get_output().stdout.is_empty());
    assert_eq!(json(&["revisions", "latest", path]), inspection);
}

#[test]
fn unchanged_fingerprints_do_not_attribute_an_existing_revision() {
    let directory = tempfile::tempdir().unwrap();
    let production = directory.path().join("production.pproj");
    let media = directory.path().join("clip.dat");
    std::fs::write(&media, b"fingerprint receipt fixture").unwrap();
    let path = production.to_str().unwrap();
    let created = json(&["init", path]);
    let imported = json(&["media", "add", path, media.to_str().unwrap()]);
    let resource = imported["resource_id"].as_str().unwrap();
    let unchanged = json(&[
        "media",
        "fingerprint",
        path,
        resource,
        media.to_str().unwrap(),
    ]);
    assert_eq!(unchanged["outcome"], "unchanged");
    assert_eq!(unchanged["commit_receipt"]["production_id"], created["id"]);
    assert!(unchanged["commit_receipt"]["revision"].is_null());
    std::fs::write(&media, b"changed fingerprint receipt fixture").unwrap();
    let changed = json(&[
        "media",
        "fingerprint",
        path,
        resource,
        media.to_str().unwrap(),
    ]);
    assert_eq!(changed["outcome"], "changed");
    assert_eq!(changed["commit_receipt"]["revision"]["sequence"], 2);
    assert_eq!(
        json(&["revisions", "latest", path])["id"],
        changed["commit_receipt"]["revision"]["id"]
    );
}
