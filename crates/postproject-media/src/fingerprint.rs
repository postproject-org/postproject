//! Versioned BLAKE3 file fingerprints.

use std::{
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::UNIX_EPOCH,
};

use postproject_core::{Error, ErrorKind, FileFacts, ResourceFingerprint, Result, Timestamp};

/// Files at or below this size receive a complete content hash.
pub const FULL_HASH_LIMIT_BYTES: u64 = 1024 * 1024;

/// Number of bytes read from each selected region of a larger file.
pub const REGION_SIZE_BYTES: usize = 64 * 1024;

/// Algorithm identifier for complete BLAKE3 file hashes.
pub const FULL_FINGERPRINT_ALGORITHM: &str = "pp-blake3-full-file";

/// Algorithm identifier for deterministic three-region BLAKE3 fingerprints.
pub const SAMPLED_FINGERPRINT_ALGORITHM: &str = "pp-blake3-sampled-regions";
const ALGORITHM_VERSION: u16 = 1;

/// Returns whether [`fingerprint_file`] can produce a value in this
/// fingerprint's algorithm/version domain.
pub(crate) fn is_file_fingerprint_domain(fingerprint: &ResourceFingerprint) -> bool {
    fingerprint.version() == ALGORITHM_VERSION
        && matches!(
            fingerprint.algorithm(),
            FULL_FINGERPRINT_ALGORITHM | SAMPLED_FINGERPRINT_ALGORITHM
        )
}
const SAMPLED_CONTEXT: &[u8] = b"PostProject sampled file fingerprint v1\0";

/// How much of a file contributed to a fingerprint.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FingerprintCoverage {
    /// Every content byte was hashed.
    Full,
    /// Fixed-size regions at the beginning, middle, and end were hashed.
    Sampled,
}

/// A calculated fingerprint together with cheap facts and coverage evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FingerprintReport {
    fingerprint: ResourceFingerprint,
    facts: FileFacts,
    coverage: FingerprintCoverage,
}

impl FingerprintReport {
    /// Returns the versioned content fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> &ResourceFingerprint {
        &self.fingerprint
    }

    /// Returns cheap file facts observed during hashing.
    #[must_use]
    pub const fn facts(&self) -> FileFacts {
        self.facts
    }

    /// Returns whether all bytes or selected regions were hashed.
    #[must_use]
    pub const fn coverage(&self) -> FingerprintCoverage {
        self.coverage
    }

    /// Splits the report into persistable domain values.
    #[must_use]
    pub fn into_parts(self) -> (ResourceFingerprint, FileFacts, FingerprintCoverage) {
        (self.fingerprint, self.facts, self.coverage)
    }
}

/// Calculates a deterministic, versioned fingerprint for a regular file.
///
/// Files no larger than [`FULL_HASH_LIMIT_BYTES`] receive a standard full-file
/// BLAKE3 digest. Larger files hash their byte length and three
/// [`REGION_SIZE_BYTES`] regions at deterministic offsets. Symlinks are rejected,
/// and a file that changes while being read produces an error.
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidArgument`] when `path` is not a regular file,
/// [`ErrorKind::Unsupported`] for symlinks, [`ErrorKind::Io`] for filesystem
/// failures, or [`ErrorKind::Fingerprint`] when the file changes while hashing.
pub fn fingerprint_file(path: impl AsRef<Path>) -> Result<FingerprintReport> {
    let path = path.as_ref();
    let link_metadata = fs::symlink_metadata(path).map_err(io_error(path, "read metadata"))?;
    if link_metadata.file_type().is_symlink() {
        return Err(Error::new(
            ErrorKind::Unsupported,
            format!("symbolic-link media is not supported: {}", path.display()),
        ));
    }
    if !link_metadata.is_file() {
        return Err(Error::new(
            ErrorKind::InvalidArgument,
            format!("media path is not a regular file: {}", path.display()),
        ));
    }

    let mut file = File::open(path).map_err(io_error(path, "open"))?;
    let before = file.metadata().map_err(io_error(path, "read metadata"))?;
    let size = before.len();
    let (digest, coverage, algorithm) = if size <= FULL_HASH_LIMIT_BYTES {
        (
            hash_full(&mut file, path)?,
            FingerprintCoverage::Full,
            FULL_FINGERPRINT_ALGORITHM,
        )
    } else {
        (
            hash_sampled(&mut file, path, size)?,
            FingerprintCoverage::Sampled,
            SAMPLED_FINGERPRINT_ALGORITHM,
        )
    };

    let after = file
        .metadata()
        .map_err(io_error(path, "re-read metadata"))?;
    if file_changed(&before, &after) {
        return Err(Error::new(
            ErrorKind::Fingerprint,
            format!(
                "media changed while it was being fingerprinted: {}",
                path.display()
            ),
        ));
    }

    let fingerprint =
        ResourceFingerprint::new(algorithm, ALGORITHM_VERSION, digest.as_bytes().to_vec())?;
    Ok(FingerprintReport {
        fingerprint,
        facts: FileFacts::new(size, modified_timestamp(&before)),
        coverage,
    })
}

fn hash_full(file: &mut File, path: &Path) -> Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; REGION_SIZE_BYTES].into_boxed_slice();
    loop {
        let bytes_read = file.read(&mut buffer).map_err(io_error(path, "read"))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(hasher.finalize())
}

fn hash_sampled(file: &mut File, path: &Path, size: u64) -> Result<blake3::Hash> {
    let region_size = u64::try_from(REGION_SIZE_BYTES).map_err(|error| {
        Error::new(
            ErrorKind::Fingerprint,
            format!("fingerprint region size is unsupported: {error}"),
        )
    })?;
    let offsets = [0, (size - region_size) / 2, size - region_size];
    let mut hasher = blake3::Hasher::new();
    hasher.update(SAMPLED_CONTEXT);
    hasher.update(&size.to_le_bytes());
    let mut buffer = vec![0_u8; REGION_SIZE_BYTES].into_boxed_slice();
    for offset in offsets {
        file.seek(SeekFrom::Start(offset))
            .map_err(io_error(path, "seek"))?;
        file.read_exact(&mut buffer)
            .map_err(io_error(path, "read sampled region"))?;
        hasher.update(&offset.to_le_bytes());
        hasher.update(&buffer);
    }
    Ok(hasher.finalize())
}

fn modified_timestamp(metadata: &Metadata) -> Option<Timestamp> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    let micros = i64::try_from(duration.as_micros()).ok()?;
    Some(Timestamp::from_unix_micros(micros))
}

fn file_changed(before: &Metadata, after: &Metadata) -> bool {
    before.len() != after.len() || before.modified().ok() != after.modified().ok()
}

fn io_error<'a>(path: &'a Path, action: &'static str) -> impl FnOnce(std::io::Error) -> Error + 'a {
    move |error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot {action} media {}: {error}", path.display()),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn empty_file_receives_a_full_hash() {
        let file = NamedTempFile::new().expect("create file");
        let report = fingerprint_file(file.path()).expect("fingerprint empty file");

        assert_eq!(report.coverage(), FingerprintCoverage::Full);
        assert_eq!(report.facts().size_bytes(), 0);
        assert_eq!(report.fingerprint().algorithm(), FULL_FINGERPRINT_ALGORITHM);
        assert_eq!(report.fingerprint().value(), blake3::hash(&[]).as_bytes());
    }

    #[test]
    fn same_bytes_produce_the_same_fingerprint() {
        let mut first = NamedTempFile::new().expect("create first file");
        let mut second = NamedTempFile::new().expect("create second file");
        first
            .write_all(b"deterministic media")
            .expect("write first");
        second
            .write_all(b"deterministic media")
            .expect("write second");

        assert_eq!(
            fingerprint_file(first.path())
                .expect("fingerprint first")
                .fingerprint(),
            fingerprint_file(second.path())
                .expect("fingerprint second")
                .fingerprint()
        );
    }

    #[test]
    fn sampled_hash_detects_changes_in_each_selected_region() {
        let size = usize::try_from(FULL_HASH_LIMIT_BYTES).expect("limit fits usize") + 1;
        let baseline = vec![0_u8; size];
        let baseline_hash = fingerprint_bytes(&baseline);

        for offset in [0, size / 2, size - 1] {
            let mut changed = baseline.clone();
            changed[offset] = 1;
            assert_ne!(baseline_hash, fingerprint_bytes(&changed));
        }
    }

    fn fingerprint_bytes(bytes: &[u8]) -> ResourceFingerprint {
        let mut file = NamedTempFile::new().expect("create file");
        file.write_all(bytes).expect("write file");
        let report = fingerprint_file(file.path()).expect("fingerprint file");
        assert_eq!(report.coverage(), FingerprintCoverage::Sampled);
        report.fingerprint().clone()
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links_deliberately() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("create directory");
        let target = directory.path().join("target.mov");
        let link = directory.path().join("link.mov");
        fs::write(&target, b"media").expect("write target");
        symlink(&target, &link).expect("create link");

        let error = fingerprint_file(&link).expect_err("symlink must be rejected");
        assert_eq!(error.kind(), ErrorKind::Unsupported);
    }
}
