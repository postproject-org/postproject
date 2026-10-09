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
    /// Complete ordered dependency observations.
    Dependencies,
    /// Public work requests and inert lifecycle observations.
    Jobs,
    /// Media identity, structures, roots, locators and fingerprint evidence.
    Media,
    /// Typed, ordered metadata changes.
    Metadata,
    /// Immutable activities and their original fingerprint/dependency evidence.
    Provenance,
    /// Bounded, chained chunks containing length-prefixed documents.
    RecordChunks,
}

impl RecordFeature {
    /// Returns the exact versioned wire name.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Dependencies => "dependencies.v1",
            Self::Jobs => "jobs.v1",
            Self::Media => "media.v1",
            Self::Metadata => "metadata.v1",
            Self::Provenance => "provenance.v1",
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
            "dependencies.v1" => RecordFeature::Dependencies,
            "jobs.v1" => RecordFeature::Jobs,
            "media.v1" => RecordFeature::Media,
            "metadata.v1" => RecordFeature::Metadata,
            "provenance.v1" => RecordFeature::Provenance,
            "record-chunks.v1" => RecordFeature::RecordChunks,
            _ => return Err(unsupported()),
        };
        features.insert(feature);
        previous = Some(name);
    }
    validate(&features)?;
    Ok(features)
}
