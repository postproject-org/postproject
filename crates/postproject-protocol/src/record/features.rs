use std::collections::BTreeSet;

use crate::{
    Result,
    fields::{array, malformed, text, unsupported},
};

/// Domain codecs a record's body requires from its receiver.
///
/// The wire names are ordered lexically and covered by the record digest.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RecordFeature {
    /// Asset and representation creation, structures, locators and fingerprints.
    Media,
    /// Typed, ordered metadata changes.
    Metadata,
    /// Bounded, chained chunks containing length-prefixed documents.
    RecordChunks,
}

impl RecordFeature {
    /// Returns the exact versioned wire name.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Media => "media.v1",
            Self::Metadata => "metadata.v1",
            Self::RecordChunks => "record-chunks.v1",
        }
    }
}

pub(super) fn validate(features: &BTreeSet<RecordFeature>) -> Result<()> {
    if !features.contains(&RecordFeature::RecordChunks) || features.len() < 2 {
        return Err(malformed());
    }
    Ok(())
}

pub(super) fn decode(value: &serde_json::Value) -> Result<BTreeSet<RecordFeature>> {
    let mut features = BTreeSet::new();
    let mut previous = None;
    for value in array(value, 64)? {
        let name = text(value)?;
        if previous.is_some_and(|previous| previous >= name) {
            return Err(malformed());
        }
        let feature = match name {
            "media.v1" => RecordFeature::Media,
            "metadata.v1" => RecordFeature::Metadata,
            "record-chunks.v1" => RecordFeature::RecordChunks,
            _ => return Err(unsupported()),
        };
        features.insert(feature);
        previous = Some(name);
    }
    validate(&features)?;
    Ok(features)
}
