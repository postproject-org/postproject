//! Cross-platform conversion from native paths to canonical file URIs.

use std::{
    fs,
    path::{Path, PathBuf},
};

use postproject_core::{Error, ErrorKind, Result};
use url::Url;

/// Resolves an existing native path and returns its canonical `file:` URI.
///
/// Relative components and filesystem aliases are resolved by the operating
/// system. Percent encoding and platform-specific path syntax are handled by the
/// `url` crate.
///
/// # Errors
///
/// Returns [`ErrorKind::Io`] when the path cannot be canonicalized, or
/// [`ErrorKind::InvalidArgument`] when the canonical path cannot be represented
/// as a file URI.
pub fn canonical_file_uri(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let canonical = fs::canonicalize(path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot canonicalize media path {}: {error}", path.display()),
        )
    })?;
    Url::from_file_path(&canonical)
        .map(|url| url.to_string())
        .map_err(|()| {
            Error::new(
                ErrorKind::InvalidArgument,
                format!(
                    "media path cannot be represented as a file URI: {}",
                    canonical.display()
                ),
            )
        })
}

/// Converts an absolute local `file:` URI to a native path.
///
/// # Errors
///
/// Returns [`ErrorKind::InvalidArgument`] for malformed, non-file, or
/// non-local URIs. This conversion does not require the path to exist.
pub fn local_file_path(uri: &str) -> Result<PathBuf> {
    let url = Url::parse(uri).map_err(|error| {
        Error::new(
            ErrorKind::InvalidArgument,
            format!("invalid file URI {uri}: {error}"),
        )
    })?;
    url.to_file_path().map_err(|()| {
        Error::new(
            ErrorKind::InvalidArgument,
            format!("URI is not a local file path: {uri}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_native_path_characters() {
        let directory = tempfile::tempdir().expect("create directory");
        let path = directory.path().join("clip with spaces.mov");
        fs::write(&path, b"media").expect("write media");

        let uri = canonical_file_uri(&path).expect("convert path");
        assert!(uri.starts_with("file:"));
        assert!(uri.ends_with("clip%20with%20spaces.mov"));
        // Windows canonical paths carry a verbatim `\\?\` prefix that a file
        // URI cannot express, so compare the canonical forms of both paths.
        assert_eq!(
            local_file_path(&uri)
                .expect("decode file URI")
                .canonicalize()
                .expect("canonical decoded path"),
            path.canonicalize().expect("canonical path")
        );
    }

    #[test]
    fn rejects_non_file_uris() {
        assert!(local_file_path("https://example.com/media.mov").is_err());
    }
}
