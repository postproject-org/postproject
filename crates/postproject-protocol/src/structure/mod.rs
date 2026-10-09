//! Bounded content headers and independently framed aggregate facts.

mod assembly;
mod header;
mod item;

pub use assembly::{StructureAssembler, encode_structure};
pub use header::StructureHeader;
pub use item::{SequenceException, StructureMember};
