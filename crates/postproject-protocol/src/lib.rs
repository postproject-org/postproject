//! Experimental, backend-neutral portable knowledge exchange.
//!
//! Parsing a JSON document validates framing only. Domain command decoding must
//! additionally use checked core constructors and current storage guards.

mod command;
mod conflict;
mod digest;
mod effect;
mod error;
mod extensions;
mod fields;
mod json;
mod metadata;
mod position;
mod proposal;
mod receipt;
mod rejection;
mod role;
mod scope;

pub use command::Command;
pub use conflict::{decode_transaction_conflict, encode_transaction_conflict};
pub use digest::{Digest, DigestDomain};
pub use effect::{MetadataChange, MetadataEffect};
pub use error::{FailureKind, ProtocolError, Result};
pub use extensions::Extensions;
pub use json::{Document, Limits};
pub use metadata::{decode_metadata, encode_metadata};
pub use position::Position;
pub use proposal::{MAX_PROPOSAL_COMMANDS, Proposal};
pub use receipt::{decode_receipt, encode_receipt};
pub use rejection::{Rejection, RejectionKind};
pub use role::StoreRole;
pub use scope::{
    CheckpointId, ClientId, HistoryId, MirrorInstanceId, ProtocolBase, RequestId, Scope,
};
