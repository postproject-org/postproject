//! Compound-media import coverage through the distributed CLI surface.

use std::{
    fs,
    path::Path,
    sync::{Mutex, MutexGuard, PoisonError},
};

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

/// Serializes creating fake executables with spawning the CLI.
///
/// A child forked while a fake script is still open for writing inherits that
/// descriptor, and executing the script then fails with `ETXTBSY`.
static PROCESS_LOCK: Mutex<()> = Mutex::new(());

fn process_lock() -> MutexGuard<'static, ()> {
    PROCESS_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(unix)]
fn fake_probe(directory: &Path, output: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let _process = process_lock();
    let path = directory.join("ffprobe-fake");
    fs::write(&path, format!("#!/bin/sh\nprintf '%s' '{output}'\n")).expect("write fake ffprobe");
    let mut permissions = fs::metadata(&path).expect("fake metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("make fake executable");
    path
}

#[cfg(windows)]
fn fake_probe(directory: &Path, output: &str) -> std::path::PathBuf {
    // `echo` would split multi-line output into commands and interpret
    // metacharacters, so the batch file replays the payload from a file.
    let payload = directory.join("ffprobe-fake.out");
    fs::write(&payload, output).expect("write fake ffprobe output");
    let path = directory.join("ffprobe-fake.cmd");
    let script = format!("@echo off\r\ntype \"{}\"\r\n", payload.display());
    fs::write(&path, script).expect("write fake ffprobe");
    path
}

fn run_json(arguments: &[&str]) -> Value {
    let _process = process_lock();
    let assertion = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(arguments)
        .assert()
        .success();
    serde_json::from_slice(&assertion.get_output().stdout).expect("command emits valid JSON")
}

#[test]
fn imports_checked_in_camera_card_as_one_package() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let production = temporary.path().join("camera-card.pproj");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/avchd-card");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);

    let imported = run_json(&[
        "media",
        "add",
        production,
        fixture.to_str().expect("UTF-8 fixture path"),
    ]);
    assert_eq!(imported["resource_count"], 4);
    let shown = run_json(&[
        "media",
        "show",
        production,
        imported["asset_id"].as_str().expect("asset ID"),
    ]);
    assert_eq!(shown["representations"][0]["structure"], "package");
    assert_eq!(
        shown["representations"][0]["resources"]
            .as_array()
            .expect("resources")
            .len(),
        4
    );
}

#[test]
fn imports_a_sparse_sequence_with_an_explicit_rate() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let production = temporary.path().join("sequence.pproj");
    let sequence = temporary.path().join("plate");
    fs::create_dir(&sequence).expect("create sequence directory");
    for name in ["plate.1001.exr", "plate.1003.exr"] {
        fs::write(sequence.join(name), name).expect("write frame");
    }
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);

    let imported = run_json(&[
        "media",
        "add",
        production,
        sequence.to_str().expect("UTF-8 sequence path"),
        "--sequence-rate",
        "24000/1001",
    ]);
    assert_eq!(imported["resource_count"], 1);
    let shown = run_json(&[
        "media",
        "show",
        production,
        imported["asset_id"].as_str().expect("asset ID"),
    ]);
    assert_eq!(shown["representations"][0]["structure"], "image_sequence");
}

#[test]
fn optional_inspection_records_metadata_or_reports_unavailable() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let production = temporary.path().join("inspected.pproj");
    let media = temporary.path().join("camera.mov");
    fs::write(&media, b"camera media").expect("write media");
    let output = r#"{"format":{"format_name":"mov","duration":"2.5"},"streams":[{"index":0,"codec_name":"prores","codec_type":"video","width":1920,"height":1080}]}"#;
    let ffprobe = fake_probe(temporary.path(), output);
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);

    let imported = run_json(&[
        "media",
        "add",
        production,
        media.to_str().expect("UTF-8 media path"),
        "--inspect",
        "--ffprobe",
        ffprobe.to_str().expect("UTF-8 ffprobe path"),
    ]);
    assert_eq!(imported["inspections"][0]["status"], "recorded");
    let metadata = run_json(&[
        "metadata",
        "list",
        production,
        "representation",
        imported["representation_id"]
            .as_str()
            .expect("representation ID"),
    ]);
    assert_eq!(
        metadata[0]["vocabulary"],
        "https://postproject.org/ns/technical-media/1"
    );

    let second_production = temporary.path().join("unavailable.pproj");
    let second_production = second_production.to_str().expect("UTF-8 production path");
    run_json(&["init", second_production]);
    let unavailable = run_json(&[
        "media",
        "add",
        second_production,
        media.to_str().expect("UTF-8 media path"),
        "--inspect",
        "--ffprobe",
        temporary
            .path()
            .join("missing-ffprobe")
            .to_str()
            .expect("UTF-8 missing path"),
    ]);
    assert_eq!(unavailable["inspections"][0]["status"], "unavailable");
}

#[test]
fn renamed_sequence_is_confirmed_under_its_new_naming() {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let plates = temporary.path().join("plates");
    let graded = temporary.path().join("graded");
    fs::create_dir(&plates).expect("create plates");
    fs::create_dir(&graded).expect("create graded");
    for frame in 1..=3 {
        fs::write(
            plates.join(format!("shot_{frame:04}.exr")),
            format!("frame {frame}"),
        )
        .expect("write frame");
    }
    let production = temporary.path().join("sequence.pproj");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let imported = run_json(&[
        "media",
        "add",
        production,
        plates.to_str().expect("UTF-8 plates path"),
        "--sequence-rate",
        "24/1",
    ]);
    let asset_id = imported["asset_id"].as_str().expect("asset ID").to_owned();
    let shown = run_json(&["media", "show", production, &asset_id]);
    let naming = &shown["representations"][0]["resources"][0]["locators"][0]["sequence_naming"];
    assert_eq!(naming["prefix"], "shot_");
    assert_eq!(naming["pattern"], "shot_%04d.exr");

    for frame in 1..=3 {
        fs::rename(
            plates.join(format!("shot_{frame:04}.exr")),
            graded.join(format!("shot-graded_{frame:04}.exr")),
        )
        .expect("rename frame");
    }
    let resolved = run_json(&[
        "media",
        "resolve",
        production,
        &asset_id,
        "--search-dir",
        graded.to_str().expect("UTF-8 graded path"),
    ]);
    let candidates = &resolved["resolutions"][0]["resources"][0]["candidates"];
    let uri = candidates[0]["uri"]
        .as_str()
        .expect("candidate URI")
        .to_owned();
    assert!(
        uri.ends_with("/graded"),
        "{uri} is not the graded directory"
    );
    assert_eq!(
        candidates[0]["sequence_naming"]["pattern"],
        "shot-graded_%04d.exr"
    );
    let confirmed = run_json(&[
        "media",
        "resolve",
        production,
        &asset_id,
        "--search-dir",
        graded.to_str().expect("UTF-8 graded path"),
        "--confirm",
        &uri,
        "--confirm-naming",
        "shot-graded_%04d.exr",
    ]);
    assert_eq!(
        confirmed["confirmed_sequence_naming"]["prefix"],
        "shot-graded_"
    );

    let shown = run_json(&["media", "show", production, &asset_id]);
    let locators = shown["representations"][0]["resources"][0]["locators"]
        .as_array()
        .expect("locators");
    let mut patterns = locators
        .iter()
        .map(|locator| {
            locator["sequence_naming"]["pattern"]
                .as_str()
                .expect("pattern")
        })
        .collect::<Vec<_>>();
    patterns.sort_unstable();
    assert_eq!(patterns, ["shot-graded_%04d.exr", "shot_%04d.exr"]);

    let resource_id = shown["representations"][0]["resources"][0]["id"]
        .as_str()
        .expect("resource ID");
    let verified = run_json(&[
        "media",
        "verify-content",
        production,
        resource_id,
        graded.to_str().expect("UTF-8 graded path"),
    ]);
    assert_eq!(verified["verification"], "matches");
}
