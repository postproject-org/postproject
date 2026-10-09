#[cfg(test)]
mod tests;

use postproject_core::{RevisionEventKind, SemanticConflictKey};

use crate::ExchangeResult;

pub(super) fn from_event(event: &RevisionEventKind) -> ExchangeResult<Option<SemanticConflictKey>> {
    use RevisionEventKind as Event;
    Ok(Some(match event {
        Event::LocatorAdded { resource_id, .. } | Event::LocatorRetired { resource_id, .. } => {
            SemanticConflictKey::LocatorSet(*resource_id)
        }
        Event::MediaRootAdded { media_root_id }
        | Event::MediaRootEnabledChanged { media_root_id, .. }
        | Event::MediaRootRemoved { media_root_id } => {
            SemanticConflictKey::MediaRoot(*media_root_id)
        }
        Event::ExternalIdentifierAdded { target, identifier }
        | Event::ExternalIdentifierRemoved { target, identifier } => {
            SemanticConflictKey::ExternalIdentifier {
                target: *target,
                identifier: identifier.clone(),
            }
        }
        Event::MetadataAddedOrReplaced { target, property }
        | Event::MetadataRemoved { target, property } => SemanticConflictKey::MetadataProperty {
            target: *target,
            property: property.clone(),
        },
        Event::ResourceFingerprintObserved {
            resource_id,
            algorithm,
            version,
        } => SemanticConflictKey::ResourceFingerprint {
            resource_id: *resource_id,
            algorithm: algorithm.clone(),
            version: *version,
        },
        Event::RepresentationFingerprintObserved {
            representation_id,
            algorithm,
            version,
        } => SemanticConflictKey::RepresentationFingerprint {
            representation_id: *representation_id,
            algorithm: algorithm.clone(),
            version: *version,
        },
        Event::ResourceFileFactsObserved { resource_id } => {
            SemanticConflictKey::ResourceFileFacts(*resource_id)
        }
        Event::DependencySetRecorded { representation_id } => {
            SemanticConflictKey::DependencySet(*representation_id)
        }
        Event::AssetImported { .. }
        | Event::RepresentationAdded { .. }
        | Event::ResourceAdded { .. }
        | Event::RepresentationResourceAdded { .. }
        | Event::ActivityCreated { .. }
        | Event::ActivityInputAdded { .. }
        | Event::ActivityOutputAdded { .. }
        | Event::JobRequested { .. }
        | Event::JobClaimed { .. }
        | Event::JobClaimRenewed { .. }
        | Event::JobClaimReleased { .. }
        | Event::JobSucceeded { .. }
        | Event::JobFailed { .. }
        | Event::JobCancelled { .. } => return Ok(None),
        _ => {
            return Err(postproject_protocol::ProtocolError::new(
                postproject_protocol::FailureKind::Unsupported,
                "unsupported checkpoint observation guard policy",
            )
            .into());
        }
    }))
}
