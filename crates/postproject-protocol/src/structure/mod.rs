//! Bounded content headers and independently framed aggregate facts.

mod header;
mod item;

pub use header::StructureHeader;
pub use item::{SequenceException, StructureMember};
