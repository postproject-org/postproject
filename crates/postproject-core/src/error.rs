//! Domain-level errors that do not expose backend implementation details.

/// Stable categories shared by core services and external adapters.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A caller supplied invalid input.
    InvalidArgument,
    /// A requested domain object does not exist.
    NotFound,
    /// An object or relationship already exists.
    AlreadyExists,
    /// An operating-system I/O operation failed.
    Io,
    /// A persistence backend failed.
    Storage,
    /// A schema migration failed.
    Migration,
    /// Current state conflicts with the requested operation.
    Conflict,
    /// Media resolution produced multiple credible candidates.
    AmbiguousResolution,
    /// Media fingerprint calculation or validation failed.
    Fingerprint,
    /// The requested operation is not supported.
    Unsupported,
    /// The caller cancelled the operation before it completed.
    Cancelled,
    /// An internal domain invariant was violated.
    Internal,
}

/// An application-neutral error with a stable category and useful context.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    /// Creates an error in `kind` with a human-readable message.
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the human-readable context.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// The result type returned by core domain operations.
pub type Result<T> = std::result::Result<T, Error>;
