//! Private delivery survives retries without granting duplicate ownership.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use assert_cmd::cargo::cargo_bin_cmd;
use postproject_core::{
    Job, JobId, JobKind, JobState, RepresentationKind, RequestedJobOutput, RevisionContext,
    ToolIdentity,
};
use postproject_protocol::{ClientId, Command, Extensions, Proposal, RequestId};
use postproject_storage_sqlite::SqliteProduction;
use serde_json::Value;

fn fixture(directory: &Path) -> (PathBuf, JobId) {
    let path = directory.join("authority.pproj");
    let media = directory.join("take.mov");
    fs::write(&media, b"deterministic recorded media").unwrap();
    let mut source = SqliteProduction::create(&path, None).unwrap();
    cargo_bin_cmd!("postproject")
        .args(["media", "add"])
        .arg(&path)
        .arg(media)
        .assert()
        .success();
    let asset = source.assets().unwrap()[0].id();
    let input = source.representations(asset).unwrap()[0].id();
    let job = Job::new(
        JobId::new(),
        JobKind::new("unknown:Process").unwrap(),
        vec![input],
        RequestedJobOutput::new(asset, RepresentationKind::Proxy, None).unwrap(),
    )
    .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.request_job(&job).unwrap();
    edit.commit().unwrap();
    (path, job.id())
}

fn proposal(path: &Path, command: Command) -> Proposal {
    let source = SqliteProduction::open(path).unwrap();
    Proposal::new(
        source.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![command],
        Extensions::default(),
    )
    .unwrap()
}

fn write_proposal(path: &Path, proposal: &Proposal) {
    fs::write(
        path,
        proposal.document().unwrap().canonical_bytes().unwrap(),
    )
    .unwrap();
}

fn claim(job: JobId) -> Command {
    Command::ClaimJob {
        job_id: job,
        tool: ToolIdentity::new("CLI worker", None, None).unwrap(),
        agent: None,
        duration: Duration::from_secs(3600),
    }
}

#[cfg(unix)]
fn assert_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn private_claim_delivery_and_bound_renewal_recover_after_release() {
    let directory = tempfile::tempdir().unwrap();
    let (path, job) = fixture(directory.path());
    let file = directory.path().join("proposal.json");
    let token_file = directory.path().join("private-token");
    write_proposal(&file, &proposal(&path, claim(job)));
    let output = format!("{job}={}", token_file.display());
    let accepted = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .arg("--claim-output")
        .arg(&output)
        .assert()
        .success();
    let accepted: Value = serde_json::from_slice(&accepted.get_output().stdout).unwrap();
    let token = fs::read_to_string(&token_file).unwrap();
    let secret = token.rsplit(':').next().unwrap();
    assert!(!accepted.to_string().contains(secret));
    #[cfg(unix)]
    assert_private(&token_file);
    let duplicate = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .arg("--claim-output")
        .arg(&output)
        .assert()
        .success();
    let duplicate: Value = serde_json::from_slice(&duplicate.get_output().stdout).unwrap();
    assert_eq!(duplicate, accepted);
    assert_eq!(fs::read_to_string(&token_file).unwrap(), token);
    let renewal = proposal(
        &path,
        Command::RenewJob {
            job_id: job,
            duration: Duration::from_secs(3600),
        },
    );
    write_proposal(&file, &renewal);
    let renewed = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .arg("--lease-token-file")
        .arg(&token_file)
        .assert()
        .success();
    let renewed: Value = serde_json::from_slice(&renewed.get_output().stdout).unwrap();
    assert!(!renewed.to_string().contains(secret));
    let release = proposal(&path, Command::ReleaseJob(job));
    write_proposal(&file, &release);
    cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .arg("--lease-token-file")
        .arg(&token_file)
        .assert()
        .success();
    assert!(matches!(
        SqliteProduction::open(&path)
            .unwrap()
            .job(job)
            .unwrap()
            .state(),
        JobState::Requested
    ));
    write_proposal(&file, &renewal);
    let recovered = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .arg("--lease-token-file")
        .arg("-")
        .write_stdin(token.clone())
        .assert()
        .success();
    let recovered: Value = serde_json::from_slice(&recovered.get_output().stdout).unwrap();
    assert_eq!(recovered, renewed);
    let missing = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .assert()
        .code(4);
    let missing: Value = serde_json::from_slice(&missing.get_output().stdout).unwrap();
    assert_eq!(missing["error"]["code"], "request_identity_mismatch");
    assert_eq!(
        SqliteProduction::open(&path)
            .unwrap()
            .exchange_head()
            .unwrap()
            .sequence(),
        5
    );
}

#[test]
fn claim_requires_explicit_private_destination_before_any_write() {
    let directory = tempfile::tempdir().unwrap();
    let (path, job) = fixture(directory.path());
    let file = directory.path().join("proposal.json");
    let request = proposal(&path, claim(job));
    write_proposal(&file, &request);
    let result = cargo_bin_cmd!("postproject")
        .args(["--json", "exchange", "submit"])
        .arg(&path)
        .arg(&file)
        .assert()
        .code(2);
    let result: Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(result["error"]["code"], "invalid_argument");
    let source = SqliteProduction::open(&path).unwrap();
    assert_eq!(source.exchange_head().unwrap().sequence(), 2);
    assert!(
        source
            .submission_outcome(request.scope(), request.client(), request.request())
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        source.job(job).unwrap().state(),
        JobState::Requested
    ));
}
