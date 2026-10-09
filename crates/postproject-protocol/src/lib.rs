//! Experimental, backend-neutral portable knowledge exchange.
//!
//! Parsing a JSON document validates framing only. Domain command decoding must
//! additionally use checked core constructors and current storage guards.

mod assertion;
mod asset;
mod checkpoint;
mod chunk;
mod command;
mod conflict;
mod conflict_floor;
mod creation;
mod digest;
mod effect;
mod error;
mod event;
mod extensions;
mod fields;
mod fingerprint;
mod frame;
mod identifier;
mod json;
mod locator;
mod media_change;
mod metadata;
mod outcome;
mod position;
mod production;
mod proposal;
mod receipt;
mod record;
mod rejection;
mod representation;
mod resource;
mod role;
mod root;
mod scope;
mod structure;
mod version;

pub use assertion::SnapshotAssertion;
pub use asset::{decode_asset, encode_asset};
pub use checkpoint::{
    CheckpointChunk, CheckpointChunkChain, CheckpointManifest, CheckpointSection, SectionSummary,
};
pub use chunk::{
    ChunkSummary, MAX_RECORD_CHUNK_BYTES, MAX_RECORD_CHUNK_PAYLOAD, RecordChunk, RecordChunkChain,
};
pub use command::Command;
pub use conflict::{decode_transaction_conflict, encode_transaction_conflict};
pub use conflict_floor::{decode_conflict_floor, encode_conflict_floor};
pub use creation::{
    CreationDecoder, CreationFact, RepresentationCreationStart, ResourceCreationStart,
    decode_original_creation_start, encode_original_creation, encode_representation_creation,
};
pub use digest::{Digest, DigestDomain};
pub use effect::{
    MetadataCanonicalParts, MetadataChange, MetadataEffect, MetadataEffectStart, MetadataOperation,
};
pub use error::{FailureKind, ProtocolError, Result};
pub use event::{decode_event, encode_event};
pub use extensions::Extensions;
pub use fingerprint::{
    FingerprintObservation, FingerprintRecomputation, FingerprintState,
    decode_fingerprint_snapshot, encode_fingerprint_snapshot,
};
pub use frame::FrameDecoder;
pub use identifier::IdentifierAttachment;
pub use json::{Document, Limits};
pub use locator::{decode_locator, encode_locator};
pub use media_change::MediaChange;
pub use metadata::{decode_metadata, encode_metadata};
pub use outcome::{Outcome, OutcomeStatus};
pub use position::Position;
pub use production::ProductionHeader;
pub use proposal::{MAX_PROPOSAL_COMMANDS, Proposal};
pub use receipt::{
    decode_receipt, decode_revision_observation, encode_receipt, encode_revision_observation,
};
pub use record::{RecordFeature, RecordManifest};
pub use rejection::{Rejection, RejectionKind};
pub use representation::RepresentationHeader;
pub use resource::ResourceHeader;
pub use role::StoreRole;
pub use root::{decode_root, encode_root};
pub use scope::{
    CheckpointId, ClientId, HistoryId, MirrorInstanceId, ProtocolBase, RequestId, Scope,
};
pub use structure::{
    SequenceException, StructureAssembler, StructureHeader, StructureMember, encode_structure,
};
pub use version::ConflictVersion;
