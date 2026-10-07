//! End-to-end CLI coverage for the reference local executor.

use std::{fs, path::Path, str::FromStr};

use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::{JobId, JobState, MetadataProperty, ObjectRef, PropertyId, VocabularyId};
use postproject_storage_sqlite::SqliteProduction;
use serde_json::Value;

const PARAMETER_VOCABULARY: &str = "https://postproject.org/ns/executor-parameters/1";

fn run_json(arguments: &[&str]) -> Value {
    let assertion = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(arguments)
        .assert()
        .success();
    serde_json::from_slice(&assertion.get_output().stdout).expect("command emits valid JSON")
}

#[cfg(unix)]
fn fake_ffmpeg(directory: &Path, fail: bool) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.join(if fail { "ffmpeg-fail" } else { "ffmpeg-ok" });
    let behavior = if fail {
        "printf partial > \"$last\"\nprintf 'fake encoder crashed' >&2\nexit 9"
    } else {
        "printf proxy-media > \"$last\"\nexit 0"
    };
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo 'ffmpeg version cli-fake-1'; exit 0; fi\nfor last do :; done\n{behavior}\n"
        ),
    )
    .expect("write fake ffmpeg");
    let mut permissions = fs::metadata(&path).expect("fake metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("make fake executable");
    path
}

#[cfg(windows)]
fn fake_ffmpeg(directory: &Path, fail: bool) -> std::path::PathBuf {
    let path = directory.join(if fail {
        "ffmpeg-fail.cmd"
    } else {
        "ffmpeg-ok.cmd"
    });
    let behavior = if fail {
        ">\"%last%\" echo partial\r\n>&2 echo fake encoder crashed\r\nexit /b 9"
    } else {
        ">\"%last%\" echo proxy-media\r\nexit /b 0"
    };
    fs::write(
        &path,
        format!(
            "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo ffmpeg version cli-fake-1\r\n  exit /b 0\r\n)\r\nset \"last=\"\r\n:args\r\nif \"%~1\"==\"\" goto run\r\nset \"last=%~1\"\r\nshift\r\ngoto args\r\n:run\r\n{behavior}\r\n"
        ),
    )
    .expect("write fake ffmpeg");
    path
}

struct Fixture {
    directory: tempfile::TempDir,
    production: std::path::PathBuf,
    output_root: std::path::PathBuf,
    asset_id: String,
    representation_id: String,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let production = directory.path().join("executor.pproj");
    let source = directory.path().join("source.mov");
    let output_root = directory.path().join("proxies");
    fs::write(&source, b"source media").expect("write source media");
    fs::create_dir(&output_root).expect("create output root");
    let production_text = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production_text]);
    run_json(&["root", "add", production_text, "proxies"]);
    let imported = run_json(&[
        "media",
        "add",
        production_text,
        source.to_str().expect("UTF-8 source path"),
    ]);
    Fixture {
        directory,
        production,
        output_root,
        asset_id: imported["asset_id"].as_str().expect("asset ID").to_owned(),
        representation_id: imported["representation_id"]
            .as_str()
            .expect("representation ID")
            .to_owned(),
    }
}

fn request_proxy(fixture: &Fixture) -> Value {
    run_json(&[
        "job",
        "request",
        fixture.production.to_str().expect("UTF-8 production path"),
        "org.postproject:generate-proxy",
        &fixture.asset_id,
        "proxy",
        "--input",
        &fixture.representation_id,
        "--target-root",
        "proxies",
        "--profile",
        "proxy-720p",
    ])
}

fn run_once(fixture: &Fixture, executable: &Path) -> assert_cmd::assert::Assert {
    cargo_bin_cmd!("postproject")
        .arg("--json")
        .args([
            "job",
            "run",
            fixture.production.to_str().expect("UTF-8 production path"),
            "--once",
            "--root-map",
            &format!("proxies={}", fixture.output_root.display()),
            "--ffmpeg",
            executable.to_str().expect("UTF-8 executable path"),
        ])
        .assert()
}

#[test]
fn run_once_completes_proxy_with_snapshots_and_profile() {
    let fixture = fixture();
    let requested = request_proxy(&fixture);
    let executable = fake_ffmpeg(fixture.directory.path(), false);
    let assertion = run_once(&fixture, &executable).success();
    let result: Value =
        serde_json::from_slice(&assertion.get_output().stdout).expect("executor JSON");
    assert_eq!(result[0]["job"]["state"], "succeeded");
    assert_worker_receipts(&result[0]);
    let output_path = result[0]["output"].as_str().expect("output path");
    assert!(Path::new(output_path).is_file());

    let production = SqliteProduction::open(&fixture.production).expect("open production");
    let job_id = JobId::from_str(requested["id"].as_str().expect("job ID")).expect("parse ID");
    let job = production.job(job_id).expect("load completed job");
    let JobState::Succeeded(completion) = job.state() else {
        panic!("job did not succeed");
    };
    let activities = production
        .activities_producing(completion.representation_id())
        .expect("load producing activity");
    let [activity] = activities.as_slice() else {
        panic!("expected one producing activity");
    };
    assert_eq!(activity.tool().expect("ffmpeg tool").name(), "ffmpeg");
    assert_eq!(
        activity.tool().expect("ffmpeg tool").version(),
        Some("cli-fake-1")
    );
    assert!(activity.inputs()[0].snapshot().is_some());
    assert!(activity.outputs()[0].snapshot().is_some());
    let profile = MetadataProperty::new(
        VocabularyId::new(PARAMETER_VOCABULARY).expect("vocabulary"),
        PropertyId::new("profile").expect("property"),
    );
    assert_eq!(
        production
            .metadata_values(ObjectRef::Activity(activity.id()), &profile)
            .expect("load activity profile")[0]
            .as_string(),
        Some("proxy-720p")
    );
}

#[test]
fn absent_ffmpeg_leaves_job_requested() {
    let fixture = fixture();
    let requested = request_proxy(&fixture);
    let assertion = run_once(&fixture, &fixture.directory.path().join("missing-ffmpeg")).failure();
    assert!(
        String::from_utf8_lossy(&assertion.get_output().stderr)
            .contains("reference executor unavailable")
    );

    let production = SqliteProduction::open(&fixture.production).expect("open production");
    let job_id = JobId::from_str(requested["id"].as_str().expect("job ID")).expect("parse ID");
    assert!(matches!(
        production.job(job_id).expect("load requested job").state(),
        JobState::Requested
    ));
}

#[test]
fn crashing_ffmpeg_fails_job_and_removes_output() {
    let fixture = fixture();
    let requested = request_proxy(&fixture);
    let executable = fake_ffmpeg(fixture.directory.path(), true);
    let assertion = run_once(&fixture, &executable).success();
    let result: Value =
        serde_json::from_slice(&assertion.get_output().stdout).expect("executor JSON");
    assert_eq!(result[0]["job"]["state"], "failed");
    assert_worker_receipts(&result[0]);
    assert!(
        result[0]["job"]["failure_diagnostic"]
            .as_str()
            .expect("failure diagnostic")
            .contains("fake encoder crashed")
    );
    assert!(
        fs::read_dir(&fixture.output_root)
            .expect("read output root")
            .next()
            .is_none()
    );

    let production = SqliteProduction::open(&fixture.production).expect("open production");
    let job_id = JobId::from_str(requested["id"].as_str().expect("job ID")).expect("parse ID");
    assert!(matches!(
        production.job(job_id).expect("load failed job").state(),
        JobState::Failed(_)
    ));
}

fn assert_worker_receipts(result: &Value) {
    let receipts = result["commit_receipts"].as_array().unwrap();
    assert!(receipts.len() >= 2);
    let production = receipts[0]["production_id"].as_str().unwrap();
    let mut previous = 0;
    for receipt in receipts {
        assert_eq!(receipt["production_id"], production);
        assert!(receipt["revision"]["id"].is_string());
        let sequence = receipt["revision"]["sequence"].as_u64().unwrap();
        assert!(sequence > previous);
        previous = sequence;
    }
}
