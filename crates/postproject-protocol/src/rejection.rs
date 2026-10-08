//! Terminal public failure facts, excluding backend diagnostics and credentials.

use postproject_core::{Error, ErrorKind, TransactionConflict};
use serde_json::json;

use crate::{
    Document, Result, conflict,
    fields::{malformed, nullable, object, text, unsupported},
};

/// A terminal decision category retained for one decoded request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RejectionKind {
    /// The scoped base is absent from history, forged or beyond its head.
    InvalidBase,
    /// An existing domain operation rejected the complete request.
    Domain(ErrorKind),
}

/// A safe public rejection; original backend error messages are not retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rejection {
    kind: RejectionKind,
    conflict: Option<TransactionConflict>,
}

impl Rejection {
    /// Creates a terminal invalid-base decision.
    #[must_use]
    pub const fn invalid_base() -> Self {
        Self {
            kind: RejectionKind::InvalidBase,
            conflict: None,
        }
    }

    /// Retains a semantic category and optional structured conflict detail.
    ///
    /// # Errors
    /// Storage, I/O, migration, cancellation and internal failures are uncertain
    /// or retryable and must not be cached as terminal domain rejections.
    pub fn domain(error: &Error) -> Result<Self> {
        domain_name(error.kind())?;
        let conflict = error.transaction_conflict_detail().cloned();
        if let Some(conflict) = &conflict {
            conflict::encode(conflict)?;
        }
        Ok(Self {
            kind: RejectionKind::Domain(error.kind()),
            conflict,
        })
    }

    /// Returns the stable terminal category.
    #[must_use]
    pub const fn kind(&self) -> RejectionKind {
        self.kind
    }

    /// Returns exact semantic conflict boundaries and key, when present.
    #[must_use]
    pub const fn conflict(&self) -> Option<&TransactionConflict> {
        self.conflict.as_ref()
    }

    /// Encodes safe public detail with no diagnostic string field.
    ///
    /// # Errors
    /// Rejects unsupported future categories or contradictory conflict detail.
    pub fn document(&self) -> Result<Document> {
        let kind = match self.kind {
            RejectionKind::InvalidBase => "invalid_base",
            RejectionKind::Domain(kind) => domain_name(kind)?,
        };
        Ok(Document {
            value: json!({"kind":kind,"conflict":self.conflict.as_ref().map(conflict::encode).transpose()?}),
        })
    }

    /// Decodes checked terminal categories and optional complete conflict detail.
    ///
    /// # Errors
    /// Rejects transient categories, unknown fields and inconsistent detail.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "conflict"])?;
        let kind = match text(&fields["kind"])? {
            "invalid_base" => RejectionKind::InvalidBase,
            "invalid_argument" => RejectionKind::Domain(ErrorKind::InvalidArgument),
            "not_found" => RejectionKind::Domain(ErrorKind::NotFound),
            "already_exists" => RejectionKind::Domain(ErrorKind::AlreadyExists),
            "conflict" => RejectionKind::Domain(ErrorKind::Conflict),
            "unsupported" => RejectionKind::Domain(ErrorKind::Unsupported),
            "ambiguous_resolution" => RejectionKind::Domain(ErrorKind::AmbiguousResolution),
            "fingerprint" => RejectionKind::Domain(ErrorKind::Fingerprint),
            _ => return Err(unsupported()),
        };
        let conflict = nullable(&fields["conflict"], conflict::decode)?;
        if conflict.is_some() && kind != RejectionKind::Domain(ErrorKind::Conflict) {
            return Err(malformed());
        }
        Ok(Self { kind, conflict })
    }
}

fn domain_name(kind: ErrorKind) -> Result<&'static str> {
    Ok(match kind {
        ErrorKind::InvalidArgument => "invalid_argument",
        ErrorKind::NotFound => "not_found",
        ErrorKind::AlreadyExists => "already_exists",
        ErrorKind::Conflict => "conflict",
        ErrorKind::Unsupported => "unsupported",
        ErrorKind::AmbiguousResolution => "ambiguous_resolution",
        ErrorKind::Fingerprint => "fingerprint",
        _ => return Err(unsupported()),
    })
}
