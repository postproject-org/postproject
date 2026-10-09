//! Complete authored media aggregates with bounded continuation framing.

mod encode;
mod header;

pub use encode::encode_representation_creation;
pub use header::{RepresentationCreationStart, ResourceCreationStart};
