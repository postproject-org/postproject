//! Relinking selects a representation and writes only on explicit confirmation.

use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::RepresentationId;
use postproject_storage_sqlite::SqliteProduction;
use serde_json::Value;

#[test]
fn relink_is_explicit_verified_bounded_and_idempotent() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("production.pproj");
    let old = root.path().join("original.mov");
    let moved = root.path().join("moved");
    std::fs::write(&old, b"camera original").unwrap();
    std::fs::create_dir(&moved).unwrap();
    cargo_bin_cmd!("postproject")
        .arg("init")
        .arg(&project)
        .assert()
        .success();
    let imported = cargo_bin_cmd!("postproject")
        .args(["--json", "media", "add"])
        .arg(&project)
        .arg(&old)
        .assert()
        .success();
    let imported: Value = serde_json::from_slice(&imported.get_output().stdout).unwrap();
    let representation = imported["representation_id"].as_str().unwrap();
    let id: RepresentationId = representation.parse().unwrap();
    std::fs::rename(&old, moved.join("original.mov")).unwrap();
    let before = SqliteProduction::open(&project)
        .unwrap()
        .latest_revision()
        .unwrap();
    let preview = cargo_bin_cmd!("postproject")
        .args(["--json", "media", "relink"])
        .arg(&project)
        .arg(representation)
        .arg("--search-dir")
        .arg(&moved)
        .assert()
        .success();
    let preview: Value = serde_json::from_slice(&preview.get_output().stdout).unwrap();
    assert_eq!(preview["representation_id"], representation);
    assert!(
        preview["decision_base"]
            .as_str()
            .unwrap()
            .starts_with("ppdb1:")
    );
    assert_eq!(preview["locations"].as_array().unwrap().len(), 1);
    assert_eq!(
        SqliteProduction::open(&project)
            .unwrap()
            .latest_revision()
            .unwrap(),
        before
    );
    let confirmed = cargo_bin_cmd!("postproject")
        .args(["--json", "media", "relink"])
        .arg(&project)
        .arg(representation)
        .arg("--search-dir")
        .arg(&moved)
        .arg("--confirm")
        .assert()
        .success();
    let confirmed: Value = serde_json::from_slice(&confirmed.get_output().stdout).unwrap();
    assert_eq!(confirmed["commit_receipt"]["revision"]["sequence"], 2);
    let no_op = cargo_bin_cmd!("postproject")
        .args(["--json", "media", "relink"])
        .arg(&project)
        .arg(representation)
        .arg("--confirm")
        .assert()
        .success();
    let no_op: Value = serde_json::from_slice(&no_op.get_output().stdout).unwrap();
    assert!(no_op["commit_receipt"]["revision"].is_null());
    let store = SqliteProduction::open(&project).unwrap();
    let resource = store.resources(id).unwrap().remove(0);
    assert_eq!(store.locators(resource.id()).unwrap().len(), 2);
    drop(store);
    std::fs::remove_file(moved.join("original.mov")).unwrap();
    std::fs::write(moved.join("a.mov"), b"camera original").unwrap();
    std::fs::write(moved.join("b.mov"), b"camera original").unwrap();
    let head = SqliteProduction::open(&project)
        .unwrap()
        .latest_revision()
        .unwrap();
    for budget in ["1", "100000"] {
        cargo_bin_cmd!("postproject")
            .args(["media", "relink"])
            .arg(&project)
            .arg(representation)
            .arg("--search-dir")
            .arg(&moved)
            .args(["--max-entries-per-directory", budget, "--confirm"])
            .assert()
            .failure();
        assert_eq!(
            SqliteProduction::open(&project)
                .unwrap()
                .latest_revision()
                .unwrap(),
            head
        );
    }
}
