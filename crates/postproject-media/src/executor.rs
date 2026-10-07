//! Bounded local job execution through an `ffmpeg` subprocess.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use postproject_core::{Error, ErrorKind, JobId, JobKind, Result};

/// Job kind handled by the reference proxy executor.
pub const GENERATE_PROXY_JOB_KIND: &str = "org.postproject:generate-proxy";
/// Job kind handled by the reference thumbnail executor.
pub const GENERATE_THUMBNAIL_JOB_KIND: &str = "org.postproject:generate-thumbnail";
/// Metadata vocabulary carrying reference-executor job parameters.
pub const EXECUTOR_PARAMETER_VOCABULARY: &str = "https://postproject.org/ns/executor-parameters/1";
/// Metadata property carrying the exact named executor profile.
pub const EXECUTOR_PROFILE_PROPERTY: &str = "profile";
/// Stable proxy profile producing a padded 1280 by 720 MPEG-4/AAC MP4.
pub const PROXY_720P_PROFILE: &str = "proxy-720p";
/// Stable proxy profile producing a padded 1920 by 1080 MPEG-4/AAC MP4.
pub const PROXY_1080P_PROFILE: &str = "proxy-1080p";
/// Stable thumbnail profile producing a JPEG within 640 by 640 pixels.
pub const THUMBNAIL_640_PROFILE: &str = "thumbnail-640";
/// Stable thumbnail profile producing a JPEG within 1280 by 1280 pixels.
pub const THUMBNAIL_1280_PROFILE: &str = "thumbnail-1280";

const MAX_CAPTURE_BYTES: usize = 1024 * 1024;
const MAX_DIAGNOSTIC_CHARS: usize = 1_024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Availability of the optional local execution capability.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExecutorCapability {
    /// The executable ran and reported its version.
    Available {
        /// Bounded version token reported by `ffmpeg -version`.
        version: String,
    },
    /// The configured executable is not installed.
    Unavailable {
        /// Human-readable capability diagnostic.
        reason: String,
    },
    /// The executable exists but its capability probe failed safely.
    Failed {
        /// Human-readable probe diagnostic.
        reason: String,
    },
}

/// Validated filesystem request for one claimed local job.
///
/// An execution attempt cannot be copied or replayed by cloning:
/// ```compile_fail
/// fn duplicate(request: postproject_media::ExecutionRequest) {
///     let _another_attempt = request.clone();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct ExecutionRequest {
    job_id: JobId,
    attempt: uuid::Uuid,
    kind: JobKind,
    profile: String,
    input: PathBuf,
    target_root: PathBuf,
}

impl ExecutionRequest {
    /// Creates a request after validating the supported kind/profile pair and paths.
    ///
    /// The target root must already exist. Temporary and final filenames use
    /// a private execution nonce, never a claim credential. Different workers
    /// cannot overwrite or clean up each other's output for the same job.
    ///
    /// # Errors
    ///
    /// Returns a domain error for an unsupported profile, inaccessible input,
    /// or non-directory target root.
    pub fn new(
        job_id: JobId,
        kind: JobKind,
        profile: impl Into<String>,
        input: impl AsRef<Path>,
        target_root: impl AsRef<Path>,
    ) -> Result<Self> {
        let profile = profile.into();
        profile_spec(kind.as_str(), &profile)?;
        let input = canonical_regular_file(input.as_ref(), "executor input")?;
        let target_root = canonical_directory(target_root.as_ref(), "executor target root")?;
        Ok(Self {
            job_id,
            attempt: uuid::Uuid::new_v4(),
            kind,
            profile,
            input,
            target_root,
        })
    }

    /// Returns the durable job identity.
    #[must_use]
    pub const fn job_id(&self) -> JobId {
        self.job_id
    }

    /// Returns the exact supported job kind.
    #[must_use]
    pub const fn kind(&self) -> &JobKind {
        &self.kind
    }

    /// Returns the exact named profile.
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Returns the canonical input file.
    #[must_use]
    pub fn input(&self) -> &Path {
        &self.input
    }

    /// Returns the canonical target root.
    #[must_use]
    pub fn target_root(&self) -> &Path {
        &self.target_root
    }

    fn output_paths(&self) -> Result<(PathBuf, PathBuf)> {
        let spec = profile_spec(self.kind.as_str(), &self.profile)?;
        let final_path = self.target_root.join(format!(
            "{}.{}.{}",
            self.job_id, self.attempt, spec.extension
        ));
        let temporary_path = self.target_root.join(format!(
            ".{}.{}.tmp.{}",
            self.job_id, self.attempt, spec.extension
        ));
        Ok((temporary_path, final_path))
    }
}

/// Result of invoking the optional reference executor.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExecutionOutcome {
    /// `ffmpeg` succeeded and its temporary output was published atomically.
    Completed {
        /// Final local output path.
        output: PathBuf,
        /// Bounded version token reported by the invoked executable.
        ffmpeg_version: String,
    },
    /// The configured executable was absent when execution started.
    Unavailable {
        /// Human-readable capability diagnostic.
        reason: String,
    },
    /// `ffmpeg` failed, timed out, or produced an invalid output.
    Failed {
        /// Bounded diagnostic suitable for a durable job failure.
        diagnostic: String,
    },
}

/// Adapter boundary for local execution of supported production jobs.
pub trait Executor {
    /// Reports whether the configured executable is available and usable.
    ///
    /// # Errors
    ///
    /// Returns an error only when the adapter cannot safely manage the probe.
    fn capability(&self) -> Result<ExecutorCapability>;

    /// Consumes one execution attempt and invokes `heartbeat` while it runs.
    ///
    /// The callback is responsible for renewing the durable job claim through
    /// the ordinary storage contract. No storage-specific access exists here.
    ///
    /// # Errors
    ///
    /// Returns an error only when the adapter or heartbeat cannot be managed
    /// safely. Tool failures are returned as [`ExecutionOutcome::Failed`].
    fn execute(
        &self,
        request: ExecutionRequest,
        heartbeat: &mut dyn FnMut() -> Result<()>,
    ) -> Result<ExecutionOutcome>;
}

/// `ffmpeg` subprocess implementation of [`Executor`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FfmpegExecutor {
    executable: PathBuf,
    timeout: Duration,
    heartbeat_interval: Duration,
}

impl Default for FfmpegExecutor {
    fn default() -> Self {
        Self {
            executable: PathBuf::from("ffmpeg"),
            timeout: DEFAULT_TIMEOUT,
            heartbeat_interval: DEFAULT_HEARTBEAT_INTERVAL,
        }
    }
}

impl FfmpegExecutor {
    /// Creates an executor for a specific executable, primarily for hosts and tests.
    #[must_use]
    pub fn with_executable(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            ..Self::default()
        }
    }

    /// Sets the maximum duration of one `ffmpeg` invocation.
    ///
    /// # Errors
    ///
    /// Returns an invalid-argument error for zero or unrepresentable durations.
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self> {
        validate_duration(timeout, "ffmpeg timeout")?;
        self.timeout = timeout;
        Ok(self)
    }

    /// Sets how often a running invocation asks its caller to renew the claim.
    ///
    /// # Errors
    ///
    /// Returns an invalid-argument error for zero or unrepresentable durations.
    pub fn with_heartbeat_interval(mut self, interval: Duration) -> Result<Self> {
        validate_duration(interval, "executor heartbeat interval")?;
        self.heartbeat_interval = interval;
        Ok(self)
    }

    /// Returns whether this executor recognizes an exact kind/profile pair.
    #[must_use]
    pub fn supports(kind: &str, profile: &str) -> bool {
        profile_spec(kind, profile).is_ok()
    }

    fn probe_capability(
        &self,
        heartbeat: Option<(Duration, &mut dyn FnMut() -> Result<()>)>,
    ) -> Result<ExecutorCapability> {
        let mut child = match command_with_pipes(&self.executable).arg("-version").spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ExecutorCapability::Unavailable {
                    reason: format!("{} is not installed", self.executable.display()),
                });
            }
            Err(error) => {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("cannot start {}: {error}", self.executable.display()),
                ));
            }
        };
        let result = monitor_child(&mut child, self.timeout, heartbeat)?;
        if let Some(error) = result.heartbeat_error {
            return Err(error);
        }
        if result.timed_out {
            return Ok(ExecutorCapability::Failed {
                reason: format!(
                    "ffmpeg capability probe exceeded its {} ms timeout",
                    self.timeout.as_millis()
                ),
            });
        }
        if result.output_exceeded {
            return Ok(ExecutorCapability::Failed {
                reason: format!("ffmpeg capability output exceeded {MAX_CAPTURE_BYTES} bytes"),
            });
        }
        let Some(status) = result.status else {
            return Ok(ExecutorCapability::Failed {
                reason: "ffmpeg capability probe ended without a status".to_owned(),
            });
        };
        if !status.success() {
            return Ok(ExecutorCapability::Failed {
                reason: bounded_message("ffmpeg capability probe failed", &result.stderr),
            });
        }
        let version = parse_version(&result.stdout).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidArgument,
                "ffmpeg capability output did not contain a version",
            )
        })?;
        Ok(ExecutorCapability::Available { version })
    }
}

impl Executor for FfmpegExecutor {
    fn capability(&self) -> Result<ExecutorCapability> {
        self.probe_capability(None)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "capability, subprocess, cleanup, and publication form one execution boundary"
    )]
    fn execute(
        &self,
        request: ExecutionRequest,
        heartbeat: &mut dyn FnMut() -> Result<()>,
    ) -> Result<ExecutionOutcome> {
        let version =
            match self.probe_capability(Some((self.heartbeat_interval, &mut *heartbeat)))? {
                ExecutorCapability::Available { version } => version,
                ExecutorCapability::Unavailable { reason } => {
                    return Ok(ExecutionOutcome::Unavailable { reason });
                }
                ExecutorCapability::Failed { reason } => {
                    return Ok(ExecutionOutcome::Failed { diagnostic: reason });
                }
            };
        let spec = profile_spec(request.kind.as_str(), request.profile())?;
        let (temporary_path, final_path) = request.output_paths()?;
        if final_path.exists() {
            return Ok(ExecutionOutcome::Failed {
                diagnostic: format!("executor output already exists: {}", final_path.display()),
            });
        }
        if temporary_path.exists() {
            return Ok(ExecutionOutcome::Failed {
                diagnostic: format!(
                    "executor temporary output already exists: {}",
                    temporary_path.display()
                ),
            });
        }
        let temporary = TemporaryOutput::new(temporary_path);
        let mut command = command_with_pipes(&self.executable);
        command
            .args(["-nostdin", "-hide_banner", "-v", "error", "-n", "-i"])
            .arg(request.input());
        command.args(spec.arguments).arg(temporary.path());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ExecutionOutcome::Unavailable {
                    reason: format!("{} is not installed", self.executable.display()),
                });
            }
            Err(error) => {
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("cannot start {}: {error}", self.executable.display()),
                ));
            }
        };
        let result = monitor_child(
            &mut child,
            self.timeout,
            Some((self.heartbeat_interval, heartbeat)),
        )?;
        if let Some(error) = result.heartbeat_error {
            return Err(error);
        }
        if result.timed_out {
            return Ok(ExecutionOutcome::Failed {
                diagnostic: format!(
                    "ffmpeg exceeded its {} ms timeout",
                    self.timeout.as_millis()
                ),
            });
        }
        if result.output_exceeded {
            return Ok(ExecutionOutcome::Failed {
                diagnostic: format!("ffmpeg output exceeded {MAX_CAPTURE_BYTES} bytes"),
            });
        }
        let Some(status) = result.status else {
            return Ok(ExecutionOutcome::Failed {
                diagnostic: "ffmpeg ended without a status".to_owned(),
            });
        };
        if !status.success() {
            return Ok(ExecutionOutcome::Failed {
                diagnostic: bounded_message("ffmpeg failed", &result.stderr),
            });
        }
        match fs::metadata(temporary.path()) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {}
            Ok(_) => {
                return Ok(ExecutionOutcome::Failed {
                    diagnostic: "ffmpeg did not produce a non-empty regular file".to_owned(),
                });
            }
            Err(error) => {
                return Ok(ExecutionOutcome::Failed {
                    diagnostic: format!("ffmpeg output cannot be inspected: {error}"),
                });
            }
        }
        fs::rename(temporary.path(), &final_path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(
                    "cannot publish executor output {}: {error}",
                    final_path.display()
                ),
            )
        })?;
        temporary.publish();
        Ok(ExecutionOutcome::Completed {
            output: final_path,
            ffmpeg_version: version,
        })
    }
}

struct ProfileSpec {
    extension: &'static str,
    arguments: &'static [&'static str],
}

const PROXY_720P_ARGUMENTS: &[&str] = &[
    "-map",
    "0:v:0",
    "-map",
    "0:a?",
    "-vf",
    "scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2",
    "-c:v",
    "mpeg4",
    "-q:v",
    "5",
    "-c:a",
    "aac",
    "-movflags",
    "+faststart",
    "-f",
    "mp4",
];
const PROXY_1080P_ARGUMENTS: &[&str] = &[
    "-map",
    "0:v:0",
    "-map",
    "0:a?",
    "-vf",
    "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2",
    "-c:v",
    "mpeg4",
    "-q:v",
    "5",
    "-c:a",
    "aac",
    "-movflags",
    "+faststart",
    "-f",
    "mp4",
];
const THUMBNAIL_640_ARGUMENTS: &[&str] = &[
    "-map",
    "0:v:0",
    "-frames:v",
    "1",
    "-vf",
    "scale=640:640:force_original_aspect_ratio=decrease",
    "-q:v",
    "2",
    "-f",
    "image2",
];
const THUMBNAIL_1280_ARGUMENTS: &[&str] = &[
    "-map",
    "0:v:0",
    "-frames:v",
    "1",
    "-vf",
    "scale=1280:1280:force_original_aspect_ratio=decrease",
    "-q:v",
    "2",
    "-f",
    "image2",
];

fn profile_spec(kind: &str, profile: &str) -> Result<ProfileSpec> {
    let spec = match (kind, profile) {
        (GENERATE_PROXY_JOB_KIND, PROXY_720P_PROFILE) => ProfileSpec {
            extension: "mp4",
            arguments: PROXY_720P_ARGUMENTS,
        },
        (GENERATE_PROXY_JOB_KIND, PROXY_1080P_PROFILE) => ProfileSpec {
            extension: "mp4",
            arguments: PROXY_1080P_ARGUMENTS,
        },
        (GENERATE_THUMBNAIL_JOB_KIND, THUMBNAIL_640_PROFILE) => ProfileSpec {
            extension: "jpg",
            arguments: THUMBNAIL_640_ARGUMENTS,
        },
        (GENERATE_THUMBNAIL_JOB_KIND, THUMBNAIL_1280_PROFILE) => ProfileSpec {
            extension: "jpg",
            arguments: THUMBNAIL_1280_ARGUMENTS,
        },
        _ => {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!("unsupported executor kind/profile pair: {kind} / {profile}"),
            ));
        }
    };
    Ok(spec)
}

fn canonical_regular_file(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot canonicalize {label} {}: {error}", path.display()),
        )
    })?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot inspect {label} {}: {error}", canonical.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("{label} is not a regular file: {}", canonical.display()),
        ));
    }
    Ok(canonical)
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot canonicalize {label} {}: {error}", path.display()),
        )
    })?;
    let metadata = fs::metadata(&canonical).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot inspect {label} {}: {error}", canonical.display()),
        )
    })?;
    if !metadata.is_dir() {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("{label} is not a directory: {}", canonical.display()),
        ));
    }
    Ok(canonical)
}

fn validate_duration(duration: Duration, label: &str) -> Result<()> {
    if duration.is_zero() || Instant::now().checked_add(duration).is_none() {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("{label} must be greater than zero and representable"),
        ));
    }
    Ok(())
}

fn command_with_pipes(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

struct ChildResult {
    status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    timed_out: bool,
    output_exceeded: bool,
    heartbeat_error: Option<Error>,
}

fn monitor_child(
    child: &mut Child,
    timeout: Duration,
    heartbeat: Option<(Duration, &mut dyn FnMut() -> Result<()>)>,
) -> Result<ChildResult> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::new(ErrorKind::Internal, "ffmpeg stdout pipe was not created"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::new(ErrorKind::Internal, "ffmpeg stderr pipe was not created"))?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout));
    let stderr_reader = thread::spawn(move || read_bounded(stderr));
    let started = Instant::now();
    let deadline = started
        .checked_add(timeout)
        .ok_or_else(|| Error::new(ErrorKind::InvalidArgument, "ffmpeg timeout is too large"))?;
    let mut heartbeat = heartbeat;
    let mut next_heartbeat = heartbeat
        .as_ref()
        .and_then(|(interval, _)| started.checked_add(*interval));
    let mut timed_out = false;
    let mut heartbeat_error = None;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                terminate_child(child);
                break None;
            }
            Ok(None) => {
                if next_heartbeat.is_some_and(|instant| Instant::now() >= instant) {
                    if let Some((interval, callback)) = heartbeat.as_mut() {
                        if let Err(error) = callback() {
                            heartbeat_error = Some(error);
                            terminate_child(child);
                            break None;
                        }
                        next_heartbeat = Instant::now().checked_add(*interval);
                    }
                }
                thread::sleep(POLL_INTERVAL);
            }
            Err(error) => {
                terminate_child(child);
                let _ = join_reader(stdout_reader);
                let _ = join_reader(stderr_reader);
                return Err(Error::new(
                    ErrorKind::Io,
                    format!("wait for ffmpeg: {error}"),
                ));
            }
        }
    };
    let stdout = join_reader(stdout_reader)?;
    let stderr = join_reader(stderr_reader)?;
    Ok(ChildResult {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        timed_out,
        output_exceeded: stdout.exceeded || stderr.exceeded,
        heartbeat_error,
    })
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

struct BoundedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn read_bounded(reader: impl Read) -> std::io::Result<BoundedOutput> {
    let mut bytes = Vec::new();
    let limit = u64::try_from(MAX_CAPTURE_BYTES).unwrap_or(u64::MAX);
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    let exceeded = bytes.len() > MAX_CAPTURE_BYTES;
    if exceeded {
        bytes.truncate(MAX_CAPTURE_BYTES);
    }
    Ok(BoundedOutput { bytes, exceeded })
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<BoundedOutput>>,
) -> Result<BoundedOutput> {
    reader
        .join()
        .map_err(|_| Error::new(ErrorKind::Internal, "ffmpeg output reader panicked"))?
        .map_err(|error| Error::new(ErrorKind::Io, format!("read ffmpeg output: {error}")))
}

fn parse_version(stdout: &[u8]) -> Option<String> {
    let first_line = String::from_utf8_lossy(stdout)
        .lines()
        .next()?
        .trim()
        .to_owned();
    let version = first_line
        .strip_prefix("ffmpeg version ")
        .unwrap_or(&first_line)
        .split_whitespace()
        .next()?;
    (!version.is_empty()
        && version.len() <= 128
        && version.bytes().all(|byte| byte.is_ascii_graphic()))
    .then(|| version.to_owned())
}

fn bounded_message(prefix: &str, bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    if text.is_empty() {
        prefix.to_owned()
    } else {
        format!(
            "{prefix}: {}",
            text.chars().take(MAX_DIAGNOSTIC_CHARS).collect::<String>()
        )
    }
}

struct TemporaryOutput {
    path: PathBuf,
    published: std::cell::Cell<bool>,
}

impl TemporaryOutput {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            published: std::cell::Cell::new(false),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn publish(&self) {
        self.published.set(true);
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if !self.published.get() {
            let _ = fs::remove_file(&self.path);
        }
    }
}
