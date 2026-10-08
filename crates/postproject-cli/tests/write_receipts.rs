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
    let error: Value = serde_json::from_slice(&duplicate.get_output().stdout).unwrap();
    assert_eq!(error["error"]["code"], "already_exists");
    assert!(error["error"].get("commit_receipt").is_none());
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
        "--decision-base",
        json(&["inspect", path])["decision_base"]
            .as_str()
            .expect("decision base"),
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
        "--decision-base",
        json(&["inspect", path])["decision_base"]
            .as_str()
            .expect("decision base"),
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

#[test]
fn observation_requires_current_context_and_receipts_include_file_facts() {
    let directory = tempfile::tempdir().unwrap();
    let production = directory.path().join("production.pproj");
    let media = directory.path().join("clip.dat");
    std::fs::write(&media, b"unchanged content").unwrap();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&media)
        .unwrap();
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1))
        .unwrap();
    let path = production.to_str().unwrap();
    json(&["init", path]);
    let imported = json(&["media", "add", path, media.to_str().unwrap()]);
    let resource = imported["resource_id"].as_str().unwrap();
    let head = json(&["revisions", "latest", path]);
    let missing = cargo_bin_cmd!("postproject")
        .args(["media", "fingerprint", path, resource, "absent-file"])
        .assert()
        .failure();
    assert!(
        String::from_utf8_lossy(&missing.get_output().stderr).contains("requires --decision-base")
    );
    assert_eq!(json(&["revisions", "latest", path]), head);
    let old = json(&["inspect", path]);
    json(&["root", "add", path, "archive"]);
    let newer = json(&["revisions", "latest", path]);
    let stale = cargo_bin_cmd!("postproject")
        .args([
            "--decision-base",
            old["decision_base"].as_str().unwrap(),
            "media",
            "fingerprint",
            path,
            resource,
            "absent-file",
        ])
        .assert()
        .failure();
    assert!(String::from_utf8_lossy(&stale.get_output().stderr).contains("current decision base"));
    assert_eq!(json(&["revisions", "latest", path]), newer);
    file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(2))
        .unwrap();
    let observed = json(&[
        "--decision-base",
        json(&["inspect", path])["decision_base"].as_str().unwrap(),
        "media",
        "fingerprint",
        path,
        resource,
        media.to_str().unwrap(),
    ]);
    assert_eq!(observed["outcome"], "unchanged");
    let revision = &observed["commit_receipt"]["revision"];
    assert_eq!(revision["sequence"], 3);
    assert_eq!(json(&["revisions", "latest", path])["id"], revision["id"]);
    let events = json(&[
        "revisions",
        "events",
        path,
        revision["id"].as_str().unwrap(),
    ]);
    assert_eq!(events.as_array().unwrap().len(), 1);
    assert_eq!(events[0]["kind"], "resource_file_facts_observed");
    assert_eq!(events[0]["resource_id"], resource);
}
