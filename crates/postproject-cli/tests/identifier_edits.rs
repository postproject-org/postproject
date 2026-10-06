//! CLI identifier removals require the observation that authorized them.

use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::{AssetId, ExternalIdentifier, IdentifierScheme, ObjectRef};
use postproject_storage_sqlite::SqliteProduction;
use serde_json::Value;
use std::str::FromStr;

fn json(args: &[&str]) -> Value {
    let output = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(args)
        .assert()
        .success();
    serde_json::from_slice(&output.get_output().stdout).unwrap()
}

fn reattach(production: &std::path::Path, asset: &str) {
    let mut writer = SqliteProduction::open(production).unwrap();
    let base = writer.read_session().unwrap().decision_base();
    let mut replacement = writer.begin_edit(base).unwrap();
    let target = ObjectRef::Asset(AssetId::from_str(asset).unwrap());
    let identifier = ExternalIdentifier::new(
        IdentifierScheme::new("com.example.clip").unwrap(),
        "observed",
        None,
    )
    .unwrap();
    replacement
        .remove_external_identifier(target, &identifier)
        .unwrap();
    replacement
        .add_external_identifier(target, &identifier)
        .unwrap();
    replacement.commit().unwrap();
}

#[test]
fn removal_rejects_old_observations_and_returns_its_own_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let production = directory.path().join("production.pproj");
    let media = directory.path().join("clip.dat");
    std::fs::write(&media, b"identifier decision fixture").unwrap();
    let path = production.to_str().unwrap();
    json(&["init", path]);
    let imported = json(&["media", "add", path, media.to_str().unwrap()]);
    let asset = imported["asset_id"].as_str().unwrap();
    json(&[
        "identifier",
        "add",
        path,
        "asset",
        asset,
        "com.example.clip",
        "observed",
    ]);
    let missing = cargo_bin_cmd!("postproject")
        .args([
            "--json",
            "identifier",
            "remove",
            path,
            "asset",
            asset,
            "com.example.clip",
            "observed",
        ])
        .assert()
        .failure();
    assert!(missing.get_output().stdout.is_empty());
    let inspected = json(&["inspect", path]);
    let old = inspected["decision_base"].as_str().unwrap();
    reattach(&production, asset);
    let head = json(&["revisions", "latest", path]);
    let stale = cargo_bin_cmd!("postproject")
        .args([
            "--json",
            "--decision-base",
            old,
            "identifier",
            "remove",
            path,
            "asset",
            asset,
            "com.example.clip",
            "observed",
        ])
        .assert()
        .failure();
    let error: Value = serde_json::from_slice(&stale.get_output().stdout).unwrap();
    assert_eq!(
        error["error"]["transaction_conflict"]["key"]["kind"],
        "external_identifier"
    );
    assert!(error.get("commit_receipt").is_none());
    assert_eq!(json(&["revisions", "latest", path]), head);
    assert_eq!(
        json(&["identifier", "list", path, "asset", asset])
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let refreshed = json(&["inspect", path]);
    let fresh = refreshed["decision_base"].as_str().unwrap();
    let removed = json(&[
        "--decision-base",
        fresh,
        "identifier",
        "remove",
        path,
        "asset",
        asset,
        "com.example.clip",
        "observed",
    ]);
    assert_eq!(removed["commit_receipt"]["revision"]["sequence"], 4);
    assert!(
        json(&["identifier", "list", path, "asset", asset])
            .as_array()
            .unwrap()
            .is_empty()
    );
}
