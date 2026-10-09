//! Complete authored media aggregates with bounded continuation framing.

mod decode;
mod encode;
mod header;

pub use decode::{CreationDecoder, CreationFact};
pub use encode::{encode_original_creation, encode_representation_creation};
pub use header::{RepresentationCreationStart, ResourceCreationStart};

use postproject_core::Asset;

use crate::{
    Document, Result, decode_asset,
    fields::{object, text, unsupported},
};

/// Decodes an original creation's exact asset facts; its representation follows.
///
/// # Errors
/// Rejects unknown fields/kinds and malformed asset facts.
pub fn decode_original_creation_start(document: &Document) -> Result<Asset> {
    let fields = object(&document.value, &["kind", "asset"])?;
    if text(&fields["kind"])? != "original.creation" {
        return Err(unsupported());
    }
    decode_asset(&Document {
        value: fields["asset"].clone(),
    })
}
