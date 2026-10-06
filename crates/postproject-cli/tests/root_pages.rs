//! Root page JSON preserves order, continuation and cursor production scope.

use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::{MediaRoot, MediaRootId};
use postproject_storage_sqlite::SqliteProduction;
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
fn inspection_and_root_edits_work_beyond_the_convenience_cap() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("many.pproj");
    let mut production = SqliteProduction::create(&path, None).unwrap();
    let mut edit = production.begin_transaction().unwrap();
    for index in 1_u128..=1001 {
        edit.add_media_root(
            MediaRoot::new(
                MediaRootId::from_bytes(index.to_be_bytes()),
                format!("root-{index}"),
                None,
                None,
                0,
                true,
            )
            .unwrap(),
        )
        .unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    drop(production);
    let production = path.to_str().unwrap();
    let inspection = json(&["inspect", production, "--limit", "2"]);
    assert_eq!(inspection["media_roots"].as_array().unwrap().len(), 2);
    assert_eq!(inspection["truncated"], true);
    let id = MediaRootId::from_bytes(1001_u128.to_be_bytes()).to_string();
    let disabled = json(&[
        "root",
        "disable",
        production,
        &id,
        "--decision-base",
        inspection["decision_base"].as_str().unwrap(),
    ]);
    assert_eq!(disabled["enabled"], false);
    let fresh = json(&["inspect", production]);
    let removed = json(&[
        "root",
        "remove",
        production,
        &id,
        "--decision-base",
        fresh["decision_base"].as_str().unwrap(),
    ]);
    assert_eq!(removed["id"], id);
    assert_eq!(
        json(&["root", "list", production])
            .as_array()
            .unwrap()
            .len(),
        1000
    );
}

#[test]
fn root_pages_are_bounded_ordered_and_reject_foreign_continuations() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("roots.pproj");
    let production = path.to_str().unwrap();
    json(&["init", production]);
    json(&["root", "add", production, "second"]);
    let first = json(&["root", "add", production, "first", "--priority=-1"]);
    let page = json(&["root", "page", production, "--limit", "1"]);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["id"], first["id"]);
    let cursor = page["next_cursor"].as_str().unwrap();
    let last = json(&[
        "root", "page", production, "--limit", "1", "--cursor", cursor,
    ]);
    assert_eq!(last["items"][0]["name"], "second");
    assert!(last["next_cursor"].is_null());
    let foreign = directory.path().join("foreign.pproj");
    let foreign = foreign.to_str().unwrap();
    json(&["init", foreign]);
    cargo_bin_cmd!("postproject")
        .args(["root", "page", foreign, "--cursor", cursor])
        .assert()
        .failure();
    for limit in ["0", "1001"] {
        cargo_bin_cmd!("postproject")
            .args(["root", "page", production, "--limit", limit])
            .assert()
            .failure();
    }
    let before = json(&["revisions", "latest", production]);
    json(&["root", "page", production]);
    assert_eq!(json(&["revisions", "latest", production]), before);
}
