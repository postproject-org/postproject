//! Locator retirement rejects missing and stale decision tokens without writes.

use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::ResourceId;
use postproject_media::prepare_confirmed_locator;
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

#[test]
fn retirement_requires_a_current_locator_decision_and_returns_its_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let production = directory.path().join("production.pproj");
    let media = directory.path().join("clip.dat");
    std::fs::write(&media, b"locator decision fixture").unwrap();
    let path = production.to_str().unwrap();
    json(&["init", path]);
    let imported = json(&["media", "add", path, media.to_str().unwrap()]);
    let locator = imported["locator_id"].as_str().unwrap();
    let resource = imported["resource_id"].as_str().unwrap();
    let before = json(&["revisions", "latest", path]);
    let missing = cargo_bin_cmd!("postproject")
        .args(["--json", "locator", "retire", path, locator])
        .assert()
        .failure();
    assert!(missing.get_output().stdout.is_empty());
    assert_eq!(json(&["revisions", "latest", path]), before);
    let inspection = json(&["inspect", path]);
    let base = inspection["decision_base"].as_str().unwrap();
    {
        let mut writer = SqliteProduction::open(&production).unwrap();
        let mut transaction = writer.begin_transaction().unwrap();
        let replacement = prepare_confirmed_locator(
            ResourceId::from_str(resource).unwrap(),
            "file:///replacement.dat",
            None,
            None,
        )
        .unwrap();
        transaction.add_locator(&replacement).unwrap();
        transaction.commit().unwrap();
    }
    let winner = json(&["revisions", "latest", path]);
    let stale = cargo_bin_cmd!("postproject")
        .args([
            "--json",
            "--decision-base",
            base,
            "locator",
            "retire",
            path,
            locator,
        ])
        .assert()
        .failure();
    let error: Value = serde_json::from_slice(&stale.get_output().stdout).unwrap();
    assert_eq!(
        error["error"]["transaction_conflict"]["key"]["kind"],
        "locator_set"
    );
    assert!(error.get("commit_receipt").is_none());
    assert_eq!(json(&["revisions", "latest", path]), winner);
    assert_eq!(
        json(&["locator", "list", path, resource])["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let refreshed = json(&["inspect", path]);
    let fresh = refreshed["decision_base"].as_str().unwrap();
    let retired = json(&["--decision-base", fresh, "locator", "retire", path, locator]);
    assert_eq!(retired["commit_receipt"]["revision"]["sequence"], 3);
    assert_eq!(
        json(&["locator", "list", path, resource])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
