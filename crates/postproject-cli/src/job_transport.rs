//! Explicit file/stdin transport; ordinary job output never contains credentials.

use anyhow::{Context, Result};
use postproject_core::{Error, ErrorKind, JobId, ProductionId, validate_job_lease_duration};
use postproject_storage_sqlite::SqliteJobLease;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

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
    reader
        .take(118)
        .read_to_end(&mut bytes)
        .context("read lease token")?;
    // Accept one conventional line ending, not arbitrary surrounding whitespace.
    if bytes.ends_with(b"\r\n") {
        bytes.truncate(bytes.len() - 2);
    } else if bytes.ends_with(b"\n") {
        bytes.pop();
    }
    let invalid = || Error::new(ErrorKind::InvalidArgument, "invalid scoped job lease token");
    let token = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
    let (production, job) = SqliteJobLease::token_scope(token)?;
    if job != expected_job {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            "lease token belongs to another job",
        )
        .into());
    }
    Ok(TokenInput {
        token: token.to_owned(),
        production,
    })
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        Ok(Self {
            path: path.to_owned(),
            file: Some(file),
            delivered: false,
        })
    }

    pub fn deliver(mut self, token: &str) -> io::Result<()> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("lease-token output is closed"))?;
        file.write_all(token.as_bytes())?;
        file.sync_all()?;
        self.delivered = true;
        Ok(())
    }
}

impl Drop for TokenOutput {
    fn drop(&mut self) {
        self.file.take();
        if !self.delivered {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub(super) fn parse_duration(value: &str) -> Result<Duration, String> {
    let (digits, unit) = value.split_at(
        value
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(value.len()),
    );
    let factor = match unit {
        "us" => 1,
        "ms" => 1_000,
        "s" => 1_000_000,
        "m" => 60_000_000,
        "h" => 3_600_000_000,
        _ => {
            return Err("lease duration needs an integer followed by us, ms, s, m or h".to_owned());
        }
    };
    let micros = digits
        .parse::<u64>()
        .ok()
        .and_then(|count| count.checked_mul(factor))
        .ok_or_else(|| "invalid or overflowing lease duration".to_owned())?;
    let duration = Duration::from_micros(micros);
    validate_job_lease_duration(duration).map_err(|error| error.to_string())?;
    Ok(duration)
}

/// A known durable result remains visible when subsequent delivery fails.
#[derive(Debug)]
pub(super) struct CommittedOperationError {
    pub source: anyhow::Error,
    pub receipt: postproject_core::CommitReceipt,
}

impl std::fmt::Display for CommittedOperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "operation committed for production {} at revision {:?}; result delivery failed: {}",
            self.receipt.production_id(),
            self.receipt
                .revision()
                .map(postproject_core::Revision::sequence),
            self.source
        )
    }
}

impl std::error::Error for CommittedOperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_units_are_exact_and_bounded() {
        for (text, micros) in [
            ("1us", 1),
            ("2ms", 2_000),
            ("3s", 3_000_000),
            ("4m", 240_000_000),
            ("24h", 86_400_000_000),
        ] {
            assert_eq!(parse_duration(text).unwrap(), Duration::from_micros(micros));
        }
        for text in [
            "0s",
            "25h",
            "-1s",
            "1.5s",
            "1",
            "1ns",
            "18446744073709551615h",
        ] {
            assert!(parse_duration(text).is_err(), "{text}");
        }
    }

    #[test]
    fn reservation_is_exclusive_private_and_cleans_up_failed_delivery() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lease");
        let mut output = TokenOutput::reserve(&path).unwrap();
        assert!(TokenOutput::reserve(&path).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        output.file.take();
        assert!(output.deliver("credential").is_err());
        assert!(!path.exists());
        TokenOutput::reserve(&path)
            .unwrap()
            .deliver("credential")
            .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "credential");
    }

    #[test]
    fn token_reader_bounds_and_checks_scope_without_echoing_secrets() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input");
        let job = JobId::new();
        let production = ProductionId::new();
        let secret = postproject_core::JobClaimId::new();
        let token = format!("ppl1:{production}:{job}:{secret}");
        fs::write(&path, format!("{token}\r\n")).unwrap();
        let input = read_token(&path, job).unwrap();
        assert_eq!(input.production, production);
        assert_eq!(input.token, token);
        let error = read_token(&path, JobId::new()).err().unwrap();
        assert!(!error.to_string().contains(&secret.to_string()));
        for malformed in [
            format!("{token} "),
            format!("{token}\n\n"),
            "x".repeat(1_000),
        ] {
            fs::write(&path, malformed).unwrap();
            assert!(read_token(&path, job).is_err());
        }
    }
}
