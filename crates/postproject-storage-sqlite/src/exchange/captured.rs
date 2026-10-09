//! Ordered authored operations, retained before their original atomic commit.

use postproject_core::{
    Locator, OriginalMediaImport, Representation, RepresentationImport, Resource, RevisionEventKind,
};
use postproject_protocol::{
    Document, MetadataChange, MetadataEffect, RecordFeature, encode_original_creation,
    encode_representation_creation,
};

pub(crate) enum CapturedEffect {
    Metadata(MetadataEffect),
    OriginalCreated(Box<OriginalMediaImport>),
    RepresentationCreated(Box<RepresentationImport>),
}

impl From<MetadataEffect> for CapturedEffect {
    fn from(effect: MetadataEffect) -> Self {
        Self::Metadata(effect)
    }
}

impl CapturedEffect {
    pub(crate) const fn metadata(&self) -> Option<&MetadataEffect> {
        match self {
            Self::Metadata(effect) => Some(effect),
            Self::OriginalCreated(_) | Self::RepresentationCreated(_) => None,
        }
    }

    pub(crate) const fn feature(&self) -> RecordFeature {
        match self {
            Self::Metadata(_) => RecordFeature::Metadata,
            Self::OriginalCreated(_) | Self::RepresentationCreated(_) => RecordFeature::Media,
        }
    }

    pub(crate) fn frames(
        &self,
        sequence: u64,
    ) -> postproject_protocol::Result<
        Box<dyn Iterator<Item = postproject_protocol::Result<Document>> + '_>,
    > {
        Ok(match self {
            Self::Metadata(effect) => Box::new(effect.frames()),
            Self::OriginalCreated(import) => Box::new(encode_original_creation(import, sequence)?),
            Self::RepresentationCreated(import) => {
                Box::new(encode_representation_creation(import, sequence)?)
            }
        })
    }

    pub(crate) fn observations(&self) -> Box<dyn Iterator<Item = RevisionEventKind> + '_> {
        match self {
            Self::Metadata(effect) => {
                let removed = matches!(effect.change(), MetadataChange::Removed)
                    || matches!(effect.change(), MetadataChange::Replaced(values) if values.is_empty());
                let event = if removed {
                    RevisionEventKind::MetadataRemoved {
                        target: effect.target(),
                        property: effect.property().clone(),
                    }
                } else {
                    RevisionEventKind::MetadataAddedOrReplaced {
                        target: effect.target(),
                        property: effect.property().clone(),
                    }
                };
                Box::new(std::iter::once(event))
            }
            Self::OriginalCreated(import) => Box::new(
                std::iter::once(RevisionEventKind::AssetImported {
                    asset_id: import.asset().id(),
                })
                .chain(representation_observations(
                    import.representation(),
                    import.resources(),
                    import.locators(),
                )),
            ),
            Self::RepresentationCreated(import) => Box::new(representation_observations(
                import.representation(),
                import.resources(),
                import.locators(),
            )),
        }
    }
}

fn representation_observations<'a>(
    representation: &'a Representation,
    resources: &'a [Resource],
    locators: &'a [Locator],
) -> impl Iterator<Item = RevisionEventKind> + 'a {
    let added = RevisionEventKind::RepresentationAdded {
        asset_id: representation.asset_id(),
        representation_id: representation.id(),
    };
    let resources = resources
        .iter()
        .map(|resource| RevisionEventKind::ResourceAdded {
            resource_id: resource.id(),
        });
    let members = representation
        .content_structure()
        .resource_ids()
        .into_iter()
        .enumerate()
        .map(
            move |(position, resource_id)| RevisionEventKind::RepresentationResourceAdded {
                representation_id: representation.id(),
                resource_id,
                // Core bounds every content aggregate at 100,000 members.
                position: u32::try_from(position).expect("core content member bound"),
            },
        );
    let locators = locators
        .iter()
        .map(|locator| RevisionEventKind::LocatorAdded {
            resource_id: locator.resource_id(),
            locator_id: locator.id(),
        });
    std::iter::once(added)
        .chain(resources)
        .chain(members)
        .chain(locators)
}
