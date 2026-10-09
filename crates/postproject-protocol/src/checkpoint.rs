//! Checkpoint completeness framing is separate from domain snapshot validation.

mod manifest;
mod section;
pub use manifest::CheckpointManifest;
pub use section::{CheckpointSection, SectionSummary};

mod chunk;
mod wire;
pub use chunk::CheckpointChunk;
