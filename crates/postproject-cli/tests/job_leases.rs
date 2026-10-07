//! Credentials are scoped transport, never ordinary command arguments or output.

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

fn json(args: &[&str]) -> Value {
    let result = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(args)
        .assert()
        .success();
    serde_json::from_slice(&result.get_output().stdout).unwrap()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one cross-process lifecycle verifies scope, transport and unchanged journal"
)]
fn malformed_scope_and_existing_output_reject_before_job_writes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("production.pproj");
    let media = directory.path().join("input.dat");
    let token_path = directory.path().join("lease.token");
    fs::write(&media, b"job lease fixture").unwrap();
    let production = path.to_str().unwrap();
    json(&["init", production]);
    let imported = json(&["media", "add", production, media.to_str().unwrap()]);
    let requested = json(&[
        "job",
        "request",
        production,
        "org.example:work",
        imported["asset_id"].as_str().unwrap(),
        "derived",
        "--input",
        imported["representation_id"].as_str().unwrap(),
    ]);
    let job = requested["id"].as_str().unwrap();
    let file = token_path.to_str().unwrap();
    let head = json(&["revisions", "latest", production]);
    fs::write(&token_path, "private pre-existing file").unwrap();
    cargo_bin_cmd!("postproject")
        .args([
            "job",
            "claim",
            production,
            job,
            "--tool-name",
            "worker",
            "--lease-token-file",
            file,
        ])
        .assert()
        .failure();
    assert_eq!(
        fs::read_to_string(&token_path).unwrap(),
        "private pre-existing file"
    );
    assert_eq!(json(&["revisions", "latest", production]), head);
    for duration in ["0s", "25h", "1.5s", "18446744073709551615h"] {
        cargo_bin_cmd!("postproject")
            .args([
                "job",
                "claim",
                production,
                job,
                "--tool-name",
                "worker",
                "--lease-token-file",
                file,
                "--lease",
                duration,
            ])
            .assert()
            .failure();
        assert_eq!(json(&["revisions", "latest", production]), head);
    }
    // A malformed token is rejected before opening or creating a production.
    let absent = directory.path().join("absent.pproj");
    cargo_bin_cmd!("postproject")
        .args([
            "job",
            "release",
            absent.to_str().unwrap(),
            job,
            "--lease-token-file",
            file,
        ])
        .assert()
        .failure();
    assert!(!absent.exists());
    // Reserve a different path: the existing one must remain untouched.
    let active_file = directory.path().join("active.token");
    let active = active_file.to_str().unwrap();
    let claimed = json(&[
        "job",
        "claim",
        production,
        job,
        "--tool-name",
        "worker",
        "--lease-token-file",
        active,
    ]);
    let token = fs::read_to_string(&active_file).unwrap();
    assert!(!claimed.to_string().contains(&token));
    assert!(claimed.get("claim_id").is_none());
    assert_wrong_production(directory.path(), job, active, &token);
    let released = cargo_bin_cmd!("postproject")
        .args([
            "--json",
            "job",
            "release",
            production,
            job,
            "--lease-token-file",
            "-",
        ])
        .write_stdin(format!("{token}\n"))
        .assert()
        .success();
    let released: Value = serde_json::from_slice(&released.get_output().stdout).unwrap();
    assert_eq!(released["state"], "requested");
    let next_file = directory.path().join("next.token");
    json(&[
        "job",
        "claim",
        production,
        job,
        "--tool-name",
        "worker",
        "--lease-token-file",
        next_file.to_str().unwrap(),
    ]);
    let head = json(&["revisions", "latest", production]);
    cargo_bin_cmd!("postproject")
        .args([
            "job",
            "fail",
            production,
            job,
            "obsolete worker",
            "--lease-token-file",
            active,
        ])
        .assert()
        .failure();
    assert_eq!(json(&["revisions", "latest", production]), head);
    let shown = json(&["job", "show", production, job]);
    assert_eq!(shown["state"], "claimed");
    assert!(!shown.to_string().contains(&token));
}

fn assert_wrong_production(directory: &std::path::Path, job: &str, active: &str, token: &str) {
    let other = directory.join("other.pproj");
    json(&["init", other.to_str().unwrap()]);
    let rejected = cargo_bin_cmd!("postproject")
        .args([
            "job",
            "release",
            other.to_str().unwrap(),
            job,
            "--lease-token-file",
            active,
        ])
        .assert()
        .failure();
    assert!(!String::from_utf8_lossy(&rejected.get_output().stderr).contains(token));
    assert!(json(&["revisions", "latest", other.to_str().unwrap()]).is_null());
}
