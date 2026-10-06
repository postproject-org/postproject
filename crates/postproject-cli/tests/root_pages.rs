//! Root page JSON preserves order, continuation and cursor production scope.

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
