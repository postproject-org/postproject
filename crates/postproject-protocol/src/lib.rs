//! Experimental, backend-neutral portable knowledge exchange.
//!
//! Parsing a JSON document validates framing only. Domain command decoding must
//! additionally use checked core constructors and current storage guards.

mod command;
mod digest;
mod error;
mod extensions;
mod fields;
mod json;
mod metadata;
mod position;
mod proposal;
mod scope;

pub use command::Command;
pub use digest::{Digest, DigestDomain};
pub use error::{FailureKind, ProtocolError, Result};
pub use extensions::Extensions;
pub use json::{Document, Limits};
pub use metadata::{decode_metadata, encode_metadata};
pub use position::Position;
pub use proposal::{MAX_PROPOSAL_COMMANDS, Proposal};
pub use scope::{
    CheckpointId, ClientId, HistoryId, MirrorInstanceId, ProtocolBase, RequestId, Scope,
};
