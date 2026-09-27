//! Opt-in content verification coverage through the CLI.

use std::fs;

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

#[test]
fn verification_detects_content_replaced_at_an_online_locator() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let production = temporary.path().join("verification.pproj");
    let media = temporary.path().join("clip.mov");
    fs::write(&media, b"original").expect("write original");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let imported = run_json(&[
        "media",
        "add",
        production,
        media.to_str().expect("UTF-8 media path"),
    ]);
    let asset_id = imported["asset_id"].as_str().expect("asset ID");
    let representation_id = imported["representation_id"]
        .as_str()
        .expect("representation ID");
    let resource_id = imported["resource_id"].as_str().expect("resource ID");
    fs::write(&media, b"replaced").expect("replace media in place");

    let presence = run_json(&["media", "resolve", production, asset_id]);
    assert_eq!(
        presence["resolutions"][0]["resources"][0]["state"],
        "online_at_known_locator"
    );
    let verified = run_json(&["media", "resolve", production, asset_id, "--verify"]);
    let resource = &verified["resolutions"][0]["resources"][0];
    assert_eq!(resource["state"], "error");
    assert_eq!(resource["evidence"][0]["kind"], "fingerprint_mismatch");

    let media_path = media.to_str().expect("UTF-8 media path");
    let before = run_json(&[
        "media",
        "verify-content",
        production,
        resource_id,
        media_path,
    ]);
    assert_eq!(before["verification"], "differs");

    let observed = run_json(&[
        "media",
        "fingerprint",
        production,
        resource_id,
        media.to_str().expect("UTF-8 media path"),
    ]);
    assert_eq!(observed["resource_id"], resource_id);
    assert_eq!(
        observed["representation_fingerprints"][0]["representation_id"],
        representation_id
    );
    let latest = run_json(&["revisions", "latest", production]);
    let revision_id = latest["id"].as_str().expect("revision ID");
    let events = run_json(&["revisions", "events", production, revision_id]);
    assert_eq!(events.as_array().expect("event array").len(), 2);
    assert_eq!(events[0]["kind"], "resource_fingerprint_observed");
    assert_eq!(events[1]["kind"], "representation_fingerprint_observed");

    let after = run_json(&[
        "media",
        "verify-content",
        production,
        resource_id,
        media_path,
    ]);
    assert_eq!(after["verification"], "matches");

    let reverified = run_json(&["media", "resolve", production, asset_id, "--verify"]);
    assert_eq!(
        reverified["resolutions"][0]["resources"][0]["state"],
        "online_at_known_locator"
    );
}
