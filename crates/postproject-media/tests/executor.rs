//! Subprocess-boundary tests for the reference local executor.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use postproject_core::{JobId, JobKind};
use postproject_media::{
    ExecutionOutcome, ExecutionRequest, Executor, ExecutorCapability, FfmpegExecutor,
    GENERATE_PROXY_JOB_KIND, GENERATE_THUMBNAIL_JOB_KIND, PROXY_720P_PROFILE,
    THUMBNAIL_640_PROFILE,
};

/// Serializes tests that create and invoke scripts on platforms which lock executables.
static PROCESS_LOCK: Mutex<()> = Mutex::new(());

fn process_lock() -> MutexGuard<'static, ()> {
    PROCESS_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

fn input_fixture(directory: &Path) -> PathBuf {
    let path = directory.join("source.mov");
    fs::write(&path, b"source media").expect("write input fixture");
    path
}

#[cfg(unix)]
fn fake_ffmpeg(directory: &Path, fail: bool) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.join(if fail {
        "ffmpeg-fail"
    } else {
        "ffmpeg-success"
    });
    let behavior = if fail {
        "printf 'partial output' > \"$last\"\nprintf 'synthetic crash' >&2\nexit 7"
    } else {
        "printf 'encoded media' > \"$last\"\nexit 0"
    };
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then\n  printf 'ffmpeg version fake-1.2.3\\n'\n  exit 0\nfi\nfor last do :; done\n{behavior}\n"
    );
    fs::write(&path, script).expect("write fake ffmpeg");
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
        "ffmpeg-success.cmd"
    });
    let behavior = if fail {
        ">\"%last%\" echo partial output\r\n>&2 echo synthetic crash\r\nexit /b 7"
    } else {
        "<nul >\"%last%\" set /p \"=encoded media\"\r\nexit /b 0"
    };
    let script = format!(
        "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo ffmpeg version fake-1.2.3\r\n  exit /b 0\r\n)\r\nset \"last=\"\r\n:args\r\nif \"%~1\"==\"\" goto run\r\nset \"last=%~1\"\r\nshift\r\ngoto args\r\n:run\r\n{behavior}\r\n"
    );
    fs::write(&path, script).expect("write fake ffmpeg");
    path
}

fn request(directory: &Path, kind: &str, profile: &str) -> ExecutionRequest {
    ExecutionRequest::new(
        JobId::from_bytes([1; 16]),
        JobKind::new(kind).expect("valid job kind"),
        profile,
        input_fixture(directory),
        directory,
    )
    .expect("valid execution request")
}

#[test]
fn reports_version_and_publishes_successful_output() {
    let _process = process_lock();
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let executable = fake_ffmpeg(temporary.path(), false);
    let executor = FfmpegExecutor::with_executable(executable);
    assert_eq!(
        executor.capability().expect("probe capability"),
        ExecutorCapability::Available {
            version: "fake-1.2.3".to_owned()
        }
    );

    let request = request(
        temporary.path(),
        GENERATE_PROXY_JOB_KIND,
        PROXY_720P_PROFILE,
    );
    let outcome = executor
        .execute(request, &mut || Ok(()))
        .expect("execute fake ffmpeg");
    let ExecutionOutcome::Completed {
        output,
        ffmpeg_version,
    } = outcome
    else {
        panic!("expected completed output");
    };
    assert_eq!(ffmpeg_version, "fake-1.2.3");
    assert_eq!(
        output.extension().and_then(|value| value.to_str()),
        Some("mp4")
    );
    assert_eq!(
        fs::read(output).expect("read published output"),
        b"encoded media"
    );
    assert!(
        fs::read_dir(temporary.path())
            .expect("read target root")
            .all(|entry| !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp."))
    );
}

#[test]
fn distinct_attempts_for_one_job_never_share_published_output() {
    let _process = process_lock();
    let temporary = tempfile::tempdir().unwrap();
    let executor = FfmpegExecutor::with_executable(fake_ffmpeg(temporary.path(), false));
    let execute = || {
        let request = request(
            temporary.path(),
            GENERATE_PROXY_JOB_KIND,
            PROXY_720P_PROFILE,
        );
        let ExecutionOutcome::Completed { output, .. } =
            executor.execute(request, &mut || Ok(())).unwrap()
        else {
            panic!("independent attempt must complete");
        };
        output
    };
    let old = execute();
    fs::write(&old, b"old worker output").unwrap();
    let current = execute();
    assert_ne!(old, current);
    assert_eq!(fs::read(&old).unwrap(), b"old worker output");
    assert_eq!(fs::read(&current).unwrap(), b"encoded media");
    // A rejected old worker can clean up only its private path.
    fs::remove_file(old).unwrap();
    assert_eq!(fs::read(current).unwrap(), b"encoded media");
}

#[test]
fn crashing_tool_removes_temporary_output() {
    let _process = process_lock();
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let executable = fake_ffmpeg(temporary.path(), true);
    let executor = FfmpegExecutor::with_executable(executable);
    let request = request(
        temporary.path(),
        GENERATE_THUMBNAIL_JOB_KIND,
        THUMBNAIL_640_PROFILE,
    );

    let outcome = executor
        .execute(request, &mut || Ok(()))
        .expect("run failing fake");
    assert!(matches!(
        outcome,
        ExecutionOutcome::Failed { diagnostic } if diagnostic.contains("synthetic crash")
    ));
    let files = fs::read_dir(temporary.path())
        .expect("read target root")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect::<Vec<_>>();
    assert!(
        !files
            .iter()
            .any(|name| name.to_string_lossy().ends_with(".jpg"))
    );
    assert!(
        !files
            .iter()
            .any(|name| name.to_string_lossy().contains(".tmp."))
    );
}

#[test]
fn missing_executable_is_an_optional_capability_gap() {
    let _process = process_lock();
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let executor = FfmpegExecutor::with_executable(temporary.path().join("missing-ffmpeg"));
    assert!(matches!(
        executor.capability().expect("probe missing executable"),
        ExecutorCapability::Unavailable { .. }
    ));
}

#[test]
fn validates_the_closed_reference_profile_set() {
    assert!(FfmpegExecutor::supports(
        GENERATE_PROXY_JOB_KIND,
        PROXY_720P_PROFILE
    ));
    assert!(FfmpegExecutor::supports(
        GENERATE_THUMBNAIL_JOB_KIND,
        THUMBNAIL_640_PROFILE
    ));
    assert!(!FfmpegExecutor::supports(
        GENERATE_PROXY_JOB_KIND,
        THUMBNAIL_640_PROFILE
    ));
}

#[test]
#[ignore = "requires a separately installed ffmpeg executable"]
fn real_ffmpeg_encodes_the_shipped_fixture() {
    let _process = process_lock();
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/fixtures/sample-frame.ppm");
    let request = ExecutionRequest::new(
        JobId::from_bytes([8; 16]),
        JobKind::new(GENERATE_THUMBNAIL_JOB_KIND).expect("valid kind"),
        THUMBNAIL_640_PROFILE,
        fixture,
        temporary.path(),
    )
    .expect("valid real execution request");
    let outcome = FfmpegExecutor::default()
        .execute(request, &mut || Ok(()))
        .expect("run real ffmpeg");
    let ExecutionOutcome::Completed { output, .. } = outcome else {
        panic!("real ffmpeg did not complete: {outcome:?}");
    };
    assert!(output.is_file());
    assert!(fs::metadata(output).expect("output metadata").len() > 0);
}

#[cfg(unix)]
#[test]
fn running_process_invokes_heartbeats() {
    use std::os::unix::fs::PermissionsExt;

    let _process = process_lock();
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let executable = temporary.path().join("ffmpeg-slow");
    fs::write(
        &executable,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo 'ffmpeg version fake'; exit 0; fi\nfor last do :; done\nsleep 0.08\nprintf media > \"$last\"\n",
    )
    .expect("write slow fake");
    let mut permissions = fs::metadata(&executable)
        .expect("fake metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("make fake executable");
    let executor = FfmpegExecutor::with_executable(executable)
        .with_heartbeat_interval(Duration::from_millis(10))
        .expect("heartbeat interval");
    let request = request(
        temporary.path(),
        GENERATE_PROXY_JOB_KIND,
        PROXY_720P_PROFILE,
    );
    let mut heartbeats = 0;
    let outcome = executor
        .execute(request, &mut || {
            heartbeats += 1;
            Ok(())
        })
        .expect("execute slow fake");
    assert!(matches!(outcome, ExecutionOutcome::Completed { .. }));
    assert!(heartbeats > 0);
}
