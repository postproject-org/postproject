//! Experimental, backend-neutral portable knowledge exchange.
//!
//! Parsing a JSON document validates framing only. Domain command decoding must
//! additionally use checked core constructors and current storage guards.

mod chunk;
mod command;
mod conflict;
mod digest;
mod effect;
mod error;
mod event;
mod extensions;
mod fields;
mod frame;
mod json;
mod metadata;
mod outcome;
mod position;
mod production;
mod proposal;
mod receipt;
mod record;
mod rejection;
mod role;
mod scope;
mod version;

pub use chunk::{
    ChunkSummary, MAX_RECORD_CHUNK_BYTES, MAX_RECORD_CHUNK_PAYLOAD, RecordChunk, RecordChunkChain,
};
pub use command::Command;
pub use conflict::{decode_transaction_conflict, encode_transaction_conflict};
pub use digest::{Digest, DigestDomain};
pub use effect::{MetadataChange, MetadataEffect, MetadataEffectStart, MetadataOperation};
pub use error::{FailureKind, ProtocolError, Result};
pub use event::{decode_event, encode_event};
pub use extensions::Extensions;
pub use frame::FrameDecoder;
pub use json::{Document, Limits};
pub use metadata::{decode_metadata, encode_metadata};
pub use outcome::{Outcome, OutcomeStatus};
pub use position::Position;
pub use production::ProductionHeader;
pub use proposal::{MAX_PROPOSAL_COMMANDS, Proposal};
pub use receipt::{decode_receipt, encode_receipt};
pub use record::RecordManifest;
pub use rejection::{Rejection, RejectionKind};
pub use role::StoreRole;
pub use scope::{
    CheckpointId, ClientId, HistoryId, MirrorInstanceId, ProtocolBase, RequestId, Scope,
};
pub use version::ConflictVersion;
