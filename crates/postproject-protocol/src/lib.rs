//! Experimental, backend-neutral portable knowledge exchange.
//!
//! Parsing a JSON document validates framing only. Domain command decoding must
//! additionally use checked core constructors and current storage guards.

mod digest;
mod error;
mod json;

pub use digest::{Digest, DigestDomain};
pub use error::{FailureKind, ProtocolError, Result};
pub use json::{Document, Limits};
