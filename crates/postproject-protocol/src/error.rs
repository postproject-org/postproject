//! Typed protocol failures independent of persistence diagnostics.

use std::fmt;

/// Machine-readable exchange failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FailureKind {
    /// Invalid framing or exact value representation.
    Malformed,
    /// Unsupported required version, feature or operation.
    Unsupported,
    /// A checked document, work or domain limit was exceeded.
    LimitExceeded,
    /// Production or source-history identity differs.
    ScopeMismatch,
    /// A decision base is absent, forged or outside the retained scope.
    InvalidBase,
    /// An existing request identity was reused for different intent.
    RequestIdentityMismatch,
    /// A predecessor is missing or predates the retained replay floor.
    HistoryGap,
    /// A chunk, manifest or content digest does not match.
    Integrity,
    /// An applied sequence has different authoritative contents.
    Divergence,
    /// An ordinary write was attempted on a passive mirror.
    MirrorReadOnly,
}

impl FailureKind {
    /// Returns the stable protocol failure code used by public adapters.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::Unsupported => "unsupported",
            Self::LimitExceeded => "limit_exceeded",
            Self::ScopeMismatch => "scope_mismatch",
            Self::InvalidBase => "invalid_base",
            Self::RequestIdentityMismatch => "request_identity_mismatch",
            Self::HistoryGap => "history_gap",
            Self::Integrity => "integrity",
            Self::Divergence => "divergence",
            Self::MirrorReadOnly => "mirror_read_only",
        }
    }
}

/// A bounded diagnostic that never includes the input document or credentials.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolError {
    kind: FailureKind,
    message: &'static str,
}

impl ProtocolError {
    /// Creates a typed failure without retaining caller input.
    #[must_use]
    pub const fn new(kind: FailureKind, message: &'static str) -> Self {
        Self { kind, message }
    }

    /// Returns the failure category without parsing diagnostic text.
    #[must_use]
    pub const fn kind(&self) -> FailureKind {
        self.kind
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for ProtocolError {}

/// The result of validating or encoding a protocol value.
pub type Result<T> = std::result::Result<T, ProtocolError>;
