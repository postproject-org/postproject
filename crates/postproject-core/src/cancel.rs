//! Cooperative cancellation shared between a caller and a long operation.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{Error, ErrorKind, Result};

/// A cloneable flag that asks a running operation to stop.
///
/// Clones share one flag. Any thread may cancel; an operation polls
/// [`Self::check`] at safe points and returns [`ErrorKind::Cancelled`].
/// Cancellation cannot be undone.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a token that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation of every operation observing this token.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Returns whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Returns [`ErrorKind::Cancelled`] once cancellation was requested.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Cancelled`] after [`Self::cancel`].
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            return Err(Error::new(ErrorKind::Cancelled, "operation was cancelled"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_cancellation() {
        let token = CancellationToken::new();
        let observer = token.clone();
        assert!(observer.check().is_ok());
        token.cancel();
        assert_eq!(
            observer.check().expect_err("cancelled").kind(),
            ErrorKind::Cancelled
        );
    }
}
