use std::fmt;

use postproject_core::Error;
use postproject_protocol::ProtocolError;

/// A protocol rejection or persistence failure with uncertain/retryable status.
///
/// Terminal domain rejections are returned as durable public outcomes instead.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExchangeError {
    /// A checked framing, identity, role or integrity boundary failed.
    Protocol(ProtocolError),
    /// The store failed; recover with lookup or the same request identity.
    Store(Error),
}

impl fmt::Display for ExchangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => error.fmt(formatter),
            Self::Store(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExchangeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::Store(error) => Some(error),
        }
    }
}

impl From<Error> for ExchangeError {
    fn from(error: Error) -> Self {
        Self::Store(error)
    }
}

impl From<ProtocolError> for ExchangeError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

/// The result of an authority exchange operation.
pub type ExchangeResult<T> = std::result::Result<T, ExchangeError>;
