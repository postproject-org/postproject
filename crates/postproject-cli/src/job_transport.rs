//! Explicit file/stdin transport; ordinary job output never contains credentials.

use std::{fs::{self, File, OpenOptions}, io::{self, Read, Write}, path::{Path, PathBuf}, time::Duration};
use anyhow::{Context, Result};
use postproject_core::{Error, ErrorKind, JobId, ProductionId, validate_job_lease_duration};
use postproject_storage_sqlite::SqliteJobLease;

pub(super) struct TokenInput {
    pub token: String,
    pub production: ProductionId,
}

pub(super) fn read_token(path: &Path, expected_job: JobId) -> Result<TokenInput> {
    let reader: Box<dyn Read> = if path == Path::new("-") {
        Box::new(io::stdin())
    } else {
        Box::new(File::open(path).context("open lease-token file")?)
    };
    let mut bytes = Vec::with_capacity(118);
    reader.take(118).read_to_end(&mut bytes).context("read lease token")?;
    // Accept one conventional line ending, not arbitrary surrounding whitespace.
    if bytes.ends_with(b"\r\n") { bytes.truncate(bytes.len() - 2); }
    else if bytes.ends_with(b"\n") { bytes.pop(); }
    let invalid = || Error::new(ErrorKind::InvalidArgument, "invalid scoped job lease token");
    let token = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
    let (production, job) = SqliteJobLease::token_scope(token)?;
    if job != expected_job {
        return Err(Error::new(ErrorKind::InvalidArgument, "lease token belongs to another job").into());
    }
    Ok(TokenInput { token: token.to_owned(), production })
}

/// An exclusive output reservation, cleaned up unless credential delivery succeeds.
pub(super) struct TokenOutput {
    path: PathBuf,
    file: Option<File>,
    delivered: bool,
}

impl TokenOutput {
    pub fn reserve(path: &Path) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        Ok(Self { path: path.to_owned(), file: Some(file), delivered: false })
    }

    pub fn deliver(mut self, token: &str) -> io::Result<()> {
        let file = self.file.as_mut().ok_or_else(|| io::Error::other("lease-token output is closed"))?;
        file.write_all(token.as_bytes())?;
        file.sync_all()?;
        self.delivered = true;
        Ok(())
    }
}

impl Drop for TokenOutput {
    fn drop(&mut self) {
        self.file.take();
        if !self.delivered { let _ = fs::remove_file(&self.path); }
    }
}

pub(super) fn parse_duration(value: &str) -> Result<Duration, String> {
    let (digits, unit) = value.split_at(value.find(|character: char| !character.is_ascii_digit()).unwrap_or(value.len()));
    let factor = match unit {
        "us" => 1, "ms" => 1_000, "s" => 1_000_000,
        "m" => 60_000_000, "h" => 3_600_000_000,
        _ => return Err("lease duration needs an integer followed by us, ms, s, m or h".to_owned()),
    };
    let micros = digits.parse::<u64>().ok().and_then(|count| count.checked_mul(factor))
        .ok_or_else(|| "invalid or overflowing lease duration".to_owned())?;
    let duration = Duration::from_micros(micros);
    validate_job_lease_duration(duration).map_err(|error| error.to_string())?;
    Ok(duration)
}
