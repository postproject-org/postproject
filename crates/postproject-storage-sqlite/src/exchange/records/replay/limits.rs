use postproject_protocol::{FailureKind, Limits, ProtocolError};

/// Caller-controlled record staging and individual document budgets.
///
/// These are receiver budgets, not limits on authoritative native transactions.
/// Increase them explicitly to accept larger records; no partial apply occurs
/// when a budget is exceeded. Encoded bytes include all chunk envelopes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayLimits {
    pub(super) encoded_bytes: u64,
    pub(super) document: Limits,
}

impl ReplayLimits {
    /// Creates positive staging and checked document budgets.
    ///
    /// # Errors
    /// Rejects a zero total encoded byte budget.
    pub fn new(encoded_bytes: u64, document: Limits) -> postproject_protocol::Result<Self> {
        if encoded_bytes == 0 {
            return Err(ProtocolError::new(
                FailureKind::LimitExceeded,
                "invalid replay staging budget",
            ));
        }
        Ok(Self {
            encoded_bytes,
            document,
        })
    }
}

impl Default for ReplayLimits {
    fn default() -> Self {
        Self {
            encoded_bytes: 1024 * 1024 * 1024,
            document: Limits::default(),
        }
    }
}
