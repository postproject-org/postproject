use postproject_protocol::{FailureKind, Limits, ProtocolError};

/// Receiver-controlled checkpoint transport, disk and decoding work budgets.
///
/// No authoritative production limit is implied. Raise these explicitly for
/// larger checkpoints. Disk covers the private SQLite database and its journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointLimits {
    pub(super) encoded_bytes: u64,
    pub(super) disk_bytes: u64,
    pub(super) frames: u64,
    pub(super) document: Limits,
}

impl CheckpointLimits {
    /// Constructs explicit positive receiver budgets.
    ///
    /// # Errors
    /// Rejects zero byte or work budgets.
    pub fn new(
        encoded_bytes: u64,
        disk_bytes: u64,
        frames: u64,
        document: Limits,
    ) -> postproject_protocol::Result<Self> {
        if encoded_bytes == 0 || disk_bytes == 0 || frames == 0 {
            return Err(budget());
        }
        Ok(Self {
            encoded_bytes,
            disk_bytes,
            frames,
            document,
        })
    }
}

impl Default for CheckpointLimits {
    fn default() -> Self {
        Self {
            encoded_bytes: 1024 * 1024 * 1024,
            disk_bytes: 2 * 1024 * 1024 * 1024,
            frames: 10_000_000,
            document: Limits::default(),
        }
    }
}

pub(super) fn budget() -> ProtocolError {
    ProtocolError::new(
        FailureKind::LimitExceeded,
        "checkpoint exceeds receiver budget",
    )
}
