//! Complete authored media aggregates with bounded continuation framing.

mod decode;
mod encode;
mod header;

pub use decode::{CreationDecoder, CreationFact};
pub use encode::encode_representation_creation;
pub use header::{RepresentationCreationStart, ResourceCreationStart};
