//! A complete output command publishes one receipt and no partial prefix.
use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::{MetadataProperty, ObjectRef, PropertyId, VocabularyId};
use postproject_storage_sqlite::SqliteProduction;
use serde_json::Value;

#[test]
fn multiple_outputs_keep_actual_parameters_and_late_failure_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("production.pproj");
    let first = root.path().join("render.dat");
    let second = root.path().join("thumbnail.dat");
    let parameters = root.path().join("parameters.json");
    std::fs::write(&first, b"render").unwrap();
    std::fs::write(&second, b"thumbnail").unwrap();
    std::fs::write(
        &parameters,
        r#"[
      {"vocabulary":"example:recipe","property":"setting","value":{"type":"i64","value":1}},
      {"vocabulary":"example:recipe","property":"setting","value":{"type":"i64","value":2}}
    ]"#,
    )
    .unwrap();
    cargo_bin_cmd!("postproject")
        .arg("init")
        .arg(&project)
        .assert()
        .success();
    let output = cargo_bin_cmd!("postproject")
        .args(["--json", "artifact", "register"])
        .arg(&project)
        .arg(&first)
        .arg(&second)
        .args([
            "--activity-kind",
            "example:generate",
            "--tool-name",
            "Fixture renderer",
            "--parameters",
        ])
        .arg(&parameters)
        .assert()
        .success();
    let result: Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    assert_eq!(result["outputs"].as_array().unwrap().len(), 2);
    assert_eq!(result["commit_receipt"]["revision"]["sequence"], 1);
    let store = SqliteProduction::open(&project).unwrap();
    let activities = store.activities().unwrap();
    assert_eq!(activities.len(), 1);
    let activity = &activities[0];
    assert!(activity.inputs().is_empty());
    assert_eq!(activity.outputs().len(), 2);
    let property = MetadataProperty::new(
        VocabularyId::new("example:recipe").unwrap(),
        PropertyId::new("setting").unwrap(),
    );
    assert_eq!(
        store
            .metadata_values(ObjectRef::Activity(activity.id()), &property)
            .unwrap()
            .len(),
        2
    );
    drop(store);
    cargo_bin_cmd!("postproject")
        .args(["artifact", "register"])
        .arg(&project)
        .arg(&first)
        .arg(root.path().join("missing.dat"))
        .args(["--activity-kind", "example:generate"])
        .assert()
        .failure();
    let store = SqliteProduction::open(&project).unwrap();
    assert_eq!(store.activities().unwrap(), activities);
    assert_eq!(store.assets().unwrap().len(), 2);
}
