//! JSON error contracts and explicit format negotiation for shell consumers.

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

#[test]
fn json_errors_preserve_categories_and_human_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let production = directory.path().join("production ü with spaces.pproj");
    let path = production.to_str().unwrap();
    let created = cargo_bin_cmd!("postproject")
        .args(["--json", "--output-version", "1", "init", path])
        .assert()
        .success();
    let value: Value = serde_json::from_slice(&created.get_output().stdout).unwrap();
    assert_eq!(value["format_version"], 1);
    let missing = cargo_bin_cmd!("postproject")
        .args([
            "--json",
            "job",
            "show",
            path,
            "11111111-1111-4111-8111-111111111111",
        ])
        .assert()
        .code(3);
    let error: Value = serde_json::from_slice(&missing.get_output().stdout).unwrap();
    assert_eq!(error["format_version"], 1);
    assert_eq!(error["error"]["code"], "not_found");
    assert!(error["error"]["transaction_conflict"].is_null());
    assert!(String::from_utf8_lossy(&missing.get_output().stderr).contains("error:"));
    for version in ["0", "2", "256"] {
        cargo_bin_cmd!("postproject")
            .args(["--json", "--output-version", version, "init", path])
            .assert()
            .code(2);
    }
    let human = cargo_bin_cmd!("postproject")
        .args(["job", "show", path, "11111111-1111-4111-8111-111111111111"])
        .assert()
        .code(3);
    assert!(human.get_output().stdout.is_empty());
}
