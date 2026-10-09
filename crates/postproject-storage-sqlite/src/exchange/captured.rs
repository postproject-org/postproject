//! Ordered authored operations, retained before their original atomic commit.

use postproject_protocol::MetadataEffect;

pub(crate) enum CapturedEffect {
    Metadata(MetadataEffect),
}

impl From<MetadataEffect> for CapturedEffect {
    fn from(effect: MetadataEffect) -> Self {
        Self::Metadata(effect)
    }
}

impl CapturedEffect {
    pub(crate) const fn metadata(&self) -> &MetadataEffect {
        let Self::Metadata(effect) = self;
        effect
    }
}
