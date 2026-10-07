//! Cross-platform acceptance flow carried forward from the 0.4 release plan.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::{Value, json};

fn run_json(arguments: &[&str]) -> Value {
    let assertion = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(arguments)
        .assert()
        .success();
    serde_json::from_slice(&assertion.get_output().stdout).expect("command emits valid JSON")
}

#[cfg(unix)]
fn fake_ffmpeg(directory: &Path, fail: bool) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.join(if fail { "ffmpeg-fail" } else { "ffmpeg-ok" });
    let behavior = if fail {
        "printf partial > \"$last\"\nprintf 'acceptance encoder failed' >&2\nexit 9"
    } else {
        "printf acceptance-proxy > \"$last\"\nexit 0"
    };
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo 'ffmpeg version acceptance-1'; exit 0; fi\nfor last do :; done\n{behavior}\n"
        ),
    )
    .expect("write fake ffmpeg");
    let mut permissions = fs::metadata(&path).expect("fake metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("make fake executable");
    path
}

#[cfg(windows)]
fn fake_ffmpeg(directory: &Path, fail: bool) -> PathBuf {
    let path = directory.join(if fail {
        "ffmpeg-fail.cmd"
    } else {
        "ffmpeg-ok.cmd"
    });
    let behavior = if fail {
        ">\"%last%\" echo partial\r\n>&2 echo acceptance encoder failed\r\nexit /b 9"
    } else {
        ">\"%last%\" echo acceptance-proxy\r\nexit /b 0"
    };
    fs::write(
        &path,
        format!(
            "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo ffmpeg version acceptance-1\r\n  exit /b 0\r\n)\r\nset \"last=\"\r\n:args\r\nif \"%~1\"==\"\" goto run\r\nset \"last=%~1\"\r\nshift\r\ngoto args\r\n:run\r\n{behavior}\r\n"
        ),
    )
    .expect("write fake ffmpeg");
    path
}

fn import_file(production: &str, path: &Path) -> Value {
    run_json(&[
        "media",
        "add",
        production,
        path.to_str().expect("UTF-8 media path"),
    ])
}

fn request_proxy(production: &str, source: &Value) -> Value {
    run_json(&[
        "job",
        "request",
        production,
        "org.postproject:generate-proxy",
        source["asset_id"].as_str().expect("asset ID"),
        "proxy",
        "--input",
        source["representation_id"]
            .as_str()
            .expect("representation ID"),
        "--target-root",
        "proxies",
        "--profile",
        "proxy-720p",
    ])
}

fn run_one(production: &str, output_root: &Path, executable: &Path) -> Value {
    let mapping = format!("proxies={}", output_root.display());
    run_json(&[
        "job",
        "run",
        production,
        "--once",
        "--root-map",
        &mapping,
        "--ffmpeg",
        executable.to_str().expect("UTF-8 executable path"),
    ])
}

fn replace_and_observe(production: &str, imported: &Value, path: &Path, content: &[u8]) {
    fs::write(path, content).expect("replace input content");
    run_json(&[
        "media",
        "fingerprint",
        production,
        "--decision-base",
        run_json(&["inspect", production])["decision_base"]
            .as_str()
            .expect("decision base"),
        imported["resource_id"].as_str().expect("resource ID"),
        path.to_str().expect("UTF-8 input path"),
    ]);
}

fn page_items(value: &Value) -> &Vec<Value> {
    value["items"].as_array().expect("page items")
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered test keeps the published acceptance story auditable"
)]
fn published_cli_story_runs_with_a_fake_executor() {
    let temporary = tempfile::tempdir().expect("create acceptance directory");
    let production_path = temporary.path().join("acceptance.pproj");
    let production = production_path.to_str().expect("UTF-8 production path");
    let output_root = temporary.path().join("proxies");
    fs::create_dir(&output_root).expect("create proxy root");
    let camera_path = temporary.path().join("camera.mov");
    fs::write(&camera_path, b"camera original").expect("write camera original");
    let sequence_path = temporary.path().join("plates");
    fs::create_dir(&sequence_path).expect("create sequence");
    for frame in [1001, 1003] {
        fs::write(
            sequence_path.join(format!("plate.{frame:04}.exr")),
            format!("frame {frame}"),
        )
        .expect("write sequence frame");
    }
    let ffmpeg = fake_ffmpeg(temporary.path(), false);
    let failing_ffmpeg = fake_ffmpeg(temporary.path(), true);

    run_json(&["init", production]);
    run_json(&["root", "add", production, "proxies"]);
    let camera = import_file(production, &camera_path);
    let sequence = run_json(&[
        "media",
        "add",
        production,
        sequence_path.to_str().expect("UTF-8 sequence path"),
        "--sequence-rate",
        "24000/1001",
    ]);
    assert_eq!(
        run_json(&[
            "representation",
            "show",
            production,
            sequence["representation_id"]
                .as_str()
                .expect("sequence representation ID"),
        ])["structure"],
        "image_sequence"
    );

    request_proxy(production, &camera);
    let before_worker = run_json(&["revisions", "latest", production])["sequence"]
        .as_u64()
        .expect("latest sequence")
        .to_string();
    let waiting = Command::new(env!("CARGO_BIN_EXE_postproject"))
        .args([
            "--json",
            "revisions",
            "wait",
            production,
            "--after",
            &before_worker,
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn second-process waiter");
    let succeeded = run_one(production, &output_root, &ffmpeg);
    assert_eq!(succeeded[0]["job"]["state"], "succeeded");
    let proxy_id = succeeded[0]["job"]["completion_representation_id"]
        .as_str()
        .expect("proxy representation ID")
        .to_owned();
    let waited = waiting
        .wait_with_output()
        .expect("wait for revision process");
    assert!(waited.status.success());
    let waited: Value = serde_json::from_slice(&waited.stdout).expect("wait JSON");
    assert_eq!(waited["result"], "revisions");
    assert_ne!(
        waited["revisions"]
            .as_array()
            .expect("revisions")
            .as_slice(),
        Vec::<Value>::new()
    );
    let producing = run_json(&["activity", "producing", production, &proxy_id]);
    assert_eq!(page_items(&producing).len(), 1);
    assert!(page_items(&producing)[0]["inputs"][0]["snapshot"].is_object());

    replace_and_observe(
        production,
        &camera,
        &camera_path,
        b"replacement camera original",
    );
    let stale = run_json(&["artifact", "evaluate", production, &proxy_id]);
    assert_eq!(stale["state"], "stale");
    assert_eq!(stale["reasons"][0]["kind"], "fingerprint_changed");
    let plan = run_json(&["job", "plan", production, "--artifact", &proxy_id]);
    assert_eq!(plan[0]["job"]["state"], "requested");
    request_proxy(production, &camera);
    let regenerated = run_one(production, &output_root, &ffmpeg);
    let regenerated_id = regenerated[0]["job"]["completion_representation_id"]
        .as_str()
        .expect("regenerated representation ID");
    assert_eq!(
        run_json(&["artifact", "evaluate", production, regenerated_id])["state"],
        "current"
    );

    let representations_before = run_json(&[
        "representation",
        "list",
        production,
        camera["asset_id"].as_str().expect("camera asset ID"),
    ]);
    request_proxy(production, &camera);
    let failed = run_one(production, &output_root, &failing_ffmpeg);
    assert_eq!(failed[0]["job"]["state"], "failed");
    assert!(
        failed[0]["job"]["failure_diagnostic"]
            .as_str()
            .expect("failure diagnostic")
            .contains("acceptance encoder failed")
    );
    let representations_after = run_json(&[
        "representation",
        "list",
        production,
        camera["asset_id"].as_str().expect("camera asset ID"),
    ]);
    assert_eq!(
        page_items(&representations_after).len(),
        page_items(&representations_before).len()
    );

    let character_path = temporary.path().join("character.blend");
    let shot_path = temporary.path().join("shot.blend");
    let render_path = temporary.path().join("shot-render.exr");
    fs::write(&character_path, b"character v1").expect("write character");
    fs::write(&shot_path, b"shot layer").expect("write shot");
    fs::write(&render_path, b"shot render").expect("write render");
    let character = import_file(production, &character_path);
    let shot = import_file(production, &shot_path);
    let render = import_file(production, &render_path);
    let dependency_spec = temporary.path().join("shot-dependencies.json");
    fs::write(
        &dependency_spec,
        serde_json::to_vec(&json!([{
            "source_resource_id": shot["resource_id"],
            "kind": "org.postproject:reference.character",
            "target": {"kind": "asset", "id": character["asset_id"]},
            "resolved_representation_id": character["representation_id"],
            "required": true,
            "authored_reference": "//characters/lead.blend"
        }]))
        .expect("serialize dependencies"),
    )
    .expect("write dependencies");
    let inspection = run_json(&["inspect", production]);
    run_json(&[
        "--decision-base",
        inspection["decision_base"].as_str().unwrap(),
        "dependency",
        "record",
        production,
        shot["representation_id"]
            .as_str()
            .expect("shot representation ID"),
        dependency_spec.to_str().expect("UTF-8 dependency path"),
    ]);
    run_json(&[
        "activity",
        "add",
        production,
        "org.postproject:render",
        "--input",
        shot["representation_id"]
            .as_str()
            .expect("shot representation ID"),
        "--output",
        render["representation_id"]
            .as_str()
            .expect("render representation ID"),
        "--tool-name",
        "Acceptance renderer",
    ]);
    replace_and_observe(production, &character, &character_path, b"character v2");
    let render_id = render["representation_id"]
        .as_str()
        .expect("render representation ID");
    let render_state = run_json(&["artifact", "evaluate", production, render_id]);
    assert_eq!(render_state["state"], "stale");
    assert!(
        render_state["reasons"]
            .as_array()
            .expect("render reasons")
            .iter()
            .any(|reason| reason["kind"] == "dependency_fingerprint_changed")
    );
    let stale_outputs = run_json(&["artifact", "stale", production]);
    assert!(
        page_items(&stale_outputs)
            .iter()
            .any(|item| item["representation_id"] == render_id)
    );
    assert!(!page_items(&stale_outputs).iter().any(|item| {
        item["representation_id"]
            == shot["representation_id"]
                .as_str()
                .expect("shot representation ID")
    }));
    let dependents = run_json(&[
        "dependency",
        "dependents",
        production,
        "asset",
        character["asset_id"].as_str().expect("character asset ID"),
    ]);
    assert_eq!(
        page_items(&dependents)[0]["target"]["id"],
        shot["representation_id"]
    );
}
