//! C-ABI-owned projections of semantic revision events.

use std::{ffi::CString, ptr};

use postproject_core::{Error, QueryPage, RevisionEvent, RevisionEventKind};

use crate::{
    PP_REVISION_ACTIVITY_CREATED, PP_REVISION_ACTIVITY_INPUT_ADDED,
    PP_REVISION_ACTIVITY_OUTPUT_ADDED, PP_REVISION_ASSET_IMPORTED,
    PP_REVISION_DEPENDENCY_SET_RECORDED, PP_REVISION_EXTERNAL_IDENTIFIER_ADDED,
    PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED, PP_REVISION_JOB_CANCELLED,
    PP_REVISION_JOB_CLAIM_RELEASED, PP_REVISION_JOB_CLAIM_RENEWED, PP_REVISION_JOB_CLAIMED,
    PP_REVISION_JOB_FAILED, PP_REVISION_JOB_REQUESTED, PP_REVISION_JOB_SUCCEEDED,
    PP_REVISION_LOCATOR_ADDED, PP_REVISION_LOCATOR_RETIRED, PP_REVISION_MEDIA_ROOT_ADDED,
    PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED, PP_REVISION_MEDIA_ROOT_REMOVED,
    PP_REVISION_METADATA_ADDED_OR_REPLACED, PP_REVISION_METADATA_REMOVED,
    PP_REVISION_REPRESENTATION_ADDED, PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED,
    PP_REVISION_REPRESENTATION_RESOURCE_ADDED, PP_REVISION_RESOURCE_ADDED,
    PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED, PpObjectRef, PpRevisionEvent, PpUuid, exact_cstring,
    object_ref_to_abi,
};

/// Opaque immutable revision-event result set owned by the C caller.
pub struct PpRevisionEventSet {
    pub(crate) events: Vec<AbiRevisionEvent>,
    pub(crate) next_cursor: Option<CString>,
}

pub(crate) struct AbiRevisionEvent {
    kind: u32,
    position: u32,
    asset_id: Option<PpUuid>,
    representation_id: Option<PpUuid>,
    resource_id: Option<PpUuid>,
    locator_id: Option<PpUuid>,
    media_root_id: Option<PpUuid>,
    activity_id: Option<PpUuid>,
    job_id: Option<PpUuid>,
    target: Option<PpObjectRef>,
    structural_position: Option<u32>,
    enabled: Option<bool>,
    identifier_scheme: Option<CString>,
    identifier_value: Option<CString>,
    identifier_qualifier: Option<CString>,
    vocabulary: Option<CString>,
    property: Option<CString>,
    activity_kind: Option<CString>,
    role: Option<CString>,
    fingerprint_algorithm: Option<CString>,
    fingerprint_version: Option<u16>,
}

impl PpRevisionEventSet {
    pub(crate) fn new(events: &[RevisionEvent]) -> Result<Self, Error> {
        let events = events
            .iter()
            .map(AbiRevisionEvent::try_from)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            events,
            next_cursor: None,
        })
    }

    pub(crate) fn from_page(page: &QueryPage<RevisionEvent>) -> Result<Self, Error> {
        let mut set = Self::new(page.items())?;
        set.next_cursor = crate::query_cursor_to_cstring(page.next_cursor())?;
        Ok(set)
    }
}

/// Returns a borrowed continuation cursor, or null at the end of a page.
///
/// # Safety
/// Events must be null or a live handle; the string lives with that handle.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_revision_event_set_next_cursor(
    events: *const PpRevisionEventSet,
) -> *const std::ffi::c_char {
    std::panic::catch_unwind(|| {
        // SAFETY: Non-null handle validity is guaranteed by the caller.
        unsafe { events.as_ref() }
            .and_then(|set| set.next_cursor.as_ref())
            .map_or(ptr::null(), |cursor| cursor.as_ptr())
    })
    .unwrap_or(ptr::null())
}

/// Copies one bounded page of a revision's immutable semantic events.
///
/// # Safety
/// Production/ID must be live/readable, cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_revision_events_page(
    production: *const crate::PpProduction,
    revision_id: *const PpUuid,
    limit: u32,
    cursor: *const std::ffi::c_char,
    out_events: *mut *mut PpRevisionEventSet,
    out_error: *mut *mut crate::PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and inputs checked before use.
    unsafe {
        crate::initialize_output(out_events);
        crate::ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| crate::invalid_argument("production must not be null"))?;
            let revision_id = revision_id
                .as_ref()
                .ok_or_else(|| crate::invalid_argument("revision_id must not be null"))?;
            crate::require_output(out_events, "out_events")?;
            let page = crate::query_page_request(limit, cursor)?;
            let reader = crate::lock_production(&production.state);
            let events = reader.events_for_revision_page(
                postproject_core::RevisionId::from_bytes(revision_id.bytes),
                &page,
            )?;
            out_events.write(Box::into_raw(Box::new(PpRevisionEventSet::from_page(
                &events,
            )?)));
            Ok(())
        })
    }
}

impl AbiRevisionEvent {
    pub(crate) fn as_abi(&self) -> PpRevisionEvent {
        PpRevisionEvent {
            kind: self.kind,
            position: self.position,
            asset_id: self.asset_id.unwrap_or_else(zero_uuid),
            representation_id: self.representation_id.unwrap_or_else(zero_uuid),
            resource_id: self.resource_id.unwrap_or_else(zero_uuid),
            locator_id: self.locator_id.unwrap_or_else(zero_uuid),
            media_root_id: self.media_root_id.unwrap_or_else(zero_uuid),
            activity_id: self.activity_id.unwrap_or_else(zero_uuid),
            job_id: self.job_id.unwrap_or_else(zero_uuid),
            target: self.target.unwrap_or_else(zero_object_ref),
            structural_position: self.structural_position.unwrap_or(0),
            enabled: self.enabled.map_or(0, u8::from),
            identifier_scheme: c_string_ptr(self.identifier_scheme.as_ref()),
            identifier_value: c_string_ptr(self.identifier_value.as_ref()),
            identifier_qualifier: c_string_ptr(self.identifier_qualifier.as_ref()),
            vocabulary: c_string_ptr(self.vocabulary.as_ref()),
            property: c_string_ptr(self.property.as_ref()),
            activity_kind: c_string_ptr(self.activity_kind.as_ref()),
            role: c_string_ptr(self.role.as_ref()),
            fingerprint_algorithm: c_string_ptr(self.fingerprint_algorithm.as_ref()),
            fingerprint_version: self.fingerprint_version.unwrap_or(0),
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the complete semantic event projection is clearest as one exhaustive mapping"
)]
impl TryFrom<&RevisionEvent> for AbiRevisionEvent {
    type Error = Error;

    fn try_from(event: &RevisionEvent) -> Result<Self, Self::Error> {
        let mut projected = Self {
            kind: 0,
            position: event.position(),
            asset_id: None,
            representation_id: None,
            resource_id: None,
            locator_id: None,
            media_root_id: None,
            activity_id: None,
            job_id: None,
            target: None,
            structural_position: None,
            enabled: None,
            identifier_scheme: None,
            identifier_value: None,
            identifier_qualifier: None,
            vocabulary: None,
            property: None,
            activity_kind: None,
            role: None,
            fingerprint_algorithm: None,
            fingerprint_version: None,
        };
        match event.kind() {
            RevisionEventKind::AssetImported { asset_id } => {
                projected.kind = PP_REVISION_ASSET_IMPORTED;
                projected.asset_id = Some(uuid(asset_id.into_bytes()));
            }
            RevisionEventKind::RepresentationAdded {
                asset_id,
                representation_id,
            } => {
                projected.kind = PP_REVISION_REPRESENTATION_ADDED;
                projected.asset_id = Some(uuid(asset_id.into_bytes()));
                projected.representation_id = Some(uuid(representation_id.into_bytes()));
            }
            RevisionEventKind::ResourceAdded { resource_id } => {
                projected.kind = PP_REVISION_RESOURCE_ADDED;
                projected.resource_id = Some(uuid(resource_id.into_bytes()));
            }
            RevisionEventKind::RepresentationResourceAdded {
                representation_id,
                resource_id,
                position,
            } => {
                projected.kind = PP_REVISION_REPRESENTATION_RESOURCE_ADDED;
                projected.representation_id = Some(uuid(representation_id.into_bytes()));
                projected.resource_id = Some(uuid(resource_id.into_bytes()));
                projected.structural_position = Some(*position);
            }
            RevisionEventKind::LocatorAdded {
                resource_id,
                locator_id,
            } => {
                projected.kind = PP_REVISION_LOCATOR_ADDED;
                projected.resource_id = Some(uuid(resource_id.into_bytes()));
                projected.locator_id = Some(uuid(locator_id.into_bytes()));
            }
            RevisionEventKind::MediaRootAdded { media_root_id } => {
                projected.kind = PP_REVISION_MEDIA_ROOT_ADDED;
                projected.media_root_id = Some(uuid(media_root_id.into_bytes()));
            }
            RevisionEventKind::LocatorRetired {
                resource_id,
                locator_id,
            } => {
                projected.kind = PP_REVISION_LOCATOR_RETIRED;
                projected.resource_id = Some(uuid(resource_id.into_bytes()));
                projected.locator_id = Some(uuid(locator_id.into_bytes()));
            }
            RevisionEventKind::MediaRootEnabledChanged {
                media_root_id,
                enabled,
            } => {
                projected.kind = PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED;
                projected.media_root_id = Some(uuid(media_root_id.into_bytes()));
                projected.enabled = Some(*enabled);
            }
            RevisionEventKind::MediaRootRemoved { media_root_id } => {
                projected.kind = PP_REVISION_MEDIA_ROOT_REMOVED;
                projected.media_root_id = Some(uuid(media_root_id.into_bytes()));
            }
            RevisionEventKind::ExternalIdentifierAdded { target, identifier }
            | RevisionEventKind::ExternalIdentifierRemoved { target, identifier } => {
                projected.kind = if matches!(
                    event.kind(),
                    RevisionEventKind::ExternalIdentifierAdded { .. }
                ) {
                    PP_REVISION_EXTERNAL_IDENTIFIER_ADDED
                } else {
                    PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED
                };
                projected.target = Some(object_ref_to_abi(*target)?);
                projected.identifier_scheme = Some(exact_cstring(
                    identifier.scheme().as_str(),
                    "revision identifier scheme",
                )?);
                projected.identifier_value = Some(exact_cstring(
                    identifier.value(),
                    "revision identifier value",
                )?);
                projected.identifier_qualifier = identifier
                    .qualifier()
                    .map(|value| exact_cstring(value, "revision identifier qualifier"))
                    .transpose()?;
            }
            RevisionEventKind::MetadataAddedOrReplaced { target, property }
            | RevisionEventKind::MetadataRemoved { target, property } => {
                projected.kind = if matches!(
                    event.kind(),
                    RevisionEventKind::MetadataAddedOrReplaced { .. }
                ) {
                    PP_REVISION_METADATA_ADDED_OR_REPLACED
                } else {
                    PP_REVISION_METADATA_REMOVED
                };
                projected.target = Some(object_ref_to_abi(*target)?);
                projected.vocabulary = Some(exact_cstring(
                    property.vocabulary().as_str(),
                    "revision metadata vocabulary",
                )?);
                projected.property = Some(exact_cstring(
                    property.property().as_str(),
                    "revision metadata property",
                )?);
            }
            RevisionEventKind::ActivityCreated { activity_id, kind } => {
                projected.kind = PP_REVISION_ACTIVITY_CREATED;
                projected.activity_id = Some(uuid(activity_id.into_bytes()));
                projected.activity_kind =
                    Some(exact_cstring(kind.as_str(), "revision activity kind")?);
            }
            RevisionEventKind::ActivityInputAdded {
                activity_id,
                representation_id,
                role,
            }
            | RevisionEventKind::ActivityOutputAdded {
                activity_id,
                representation_id,
                role,
            } => {
                projected.kind =
                    if matches!(event.kind(), RevisionEventKind::ActivityInputAdded { .. }) {
                        PP_REVISION_ACTIVITY_INPUT_ADDED
                    } else {
                        PP_REVISION_ACTIVITY_OUTPUT_ADDED
                    };
                projected.activity_id = Some(uuid(activity_id.into_bytes()));
                projected.representation_id = Some(uuid(representation_id.into_bytes()));
                projected.role = role
                    .as_ref()
                    .map(|value| exact_cstring(value.as_str(), "revision activity role"))
                    .transpose()?;
            }
            RevisionEventKind::ResourceFingerprintObserved {
                resource_id,
                algorithm,
                version,
            } => {
                projected.kind = PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED;
                projected.resource_id = Some(uuid(resource_id.into_bytes()));
                projected.fingerprint_algorithm =
                    Some(exact_cstring(algorithm, "revision fingerprint algorithm")?);
                projected.fingerprint_version = Some(*version);
            }
            RevisionEventKind::RepresentationFingerprintObserved {
                representation_id,
                algorithm,
                version,
            } => {
                projected.kind = PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED;
                projected.representation_id = Some(uuid(representation_id.into_bytes()));
                projected.fingerprint_algorithm =
                    Some(exact_cstring(algorithm, "revision fingerprint algorithm")?);
                projected.fingerprint_version = Some(*version);
            }
            RevisionEventKind::DependencySetRecorded { representation_id } => {
                projected.kind = PP_REVISION_DEPENDENCY_SET_RECORDED;
                projected.representation_id = Some(uuid(representation_id.into_bytes()));
            }
            RevisionEventKind::JobRequested { job_id }
            | RevisionEventKind::JobClaimed { job_id }
            | RevisionEventKind::JobClaimRenewed { job_id }
            | RevisionEventKind::JobClaimReleased { job_id }
            | RevisionEventKind::JobSucceeded { job_id }
            | RevisionEventKind::JobFailed { job_id }
            | RevisionEventKind::JobCancelled { job_id } => {
                projected.kind = match event.kind() {
                    RevisionEventKind::JobRequested { .. } => PP_REVISION_JOB_REQUESTED,
                    RevisionEventKind::JobClaimed { .. } => PP_REVISION_JOB_CLAIMED,
                    RevisionEventKind::JobClaimRenewed { .. } => PP_REVISION_JOB_CLAIM_RENEWED,
                    RevisionEventKind::JobClaimReleased { .. } => PP_REVISION_JOB_CLAIM_RELEASED,
                    RevisionEventKind::JobSucceeded { .. } => PP_REVISION_JOB_SUCCEEDED,
                    RevisionEventKind::JobFailed { .. } => PP_REVISION_JOB_FAILED,
                    RevisionEventKind::JobCancelled { .. } => PP_REVISION_JOB_CANCELLED,
                    _ => unreachable!("job event arm only contains job events"),
                };
                projected.job_id = Some(uuid(job_id.into_bytes()));
            }
            _ => {
                return Err(postproject_core::Error::new(
                    postproject_core::ErrorKind::Unsupported,
                    "revision event kind is not supported by this ABI",
                ));
            }
        }
        Ok(projected)
    }
}

const fn uuid(bytes: [u8; 16]) -> PpUuid {
    PpUuid { bytes }
}

const fn zero_uuid() -> PpUuid {
    uuid([0; 16])
}

const fn zero_object_ref() -> PpObjectRef {
    PpObjectRef {
        kind: 0,
        id: zero_uuid(),
    }
}

fn c_string_ptr(value: Option<&CString>) -> *const std::ffi::c_char {
    value.map_or(ptr::null(), |value| value.as_ptr())
}

#[cfg(test)]
mod tests {
    use postproject_core::{JobId, RevisionEvent, RevisionEventKind, RevisionId};

    use super::AbiRevisionEvent;
    use crate::{
        PP_REVISION_JOB_CANCELLED, PP_REVISION_JOB_CLAIM_RELEASED, PP_REVISION_JOB_CLAIM_RENEWED,
        PP_REVISION_JOB_CLAIMED, PP_REVISION_JOB_FAILED, PP_REVISION_JOB_REQUESTED,
        PP_REVISION_JOB_SUCCEEDED,
    };

    #[test]
    fn job_events_project_only_the_job_identity() {
        let job_id = JobId::from_bytes([7; 16]);
        let kinds = [
            (
                RevisionEventKind::JobRequested { job_id },
                PP_REVISION_JOB_REQUESTED,
            ),
            (
                RevisionEventKind::JobClaimed { job_id },
                PP_REVISION_JOB_CLAIMED,
            ),
            (
                RevisionEventKind::JobClaimRenewed { job_id },
                PP_REVISION_JOB_CLAIM_RENEWED,
            ),
            (
                RevisionEventKind::JobClaimReleased { job_id },
                PP_REVISION_JOB_CLAIM_RELEASED,
            ),
            (
                RevisionEventKind::JobSucceeded { job_id },
                PP_REVISION_JOB_SUCCEEDED,
            ),
            (
                RevisionEventKind::JobFailed { job_id },
                PP_REVISION_JOB_FAILED,
            ),
            (
                RevisionEventKind::JobCancelled { job_id },
                PP_REVISION_JOB_CANCELLED,
            ),
        ];
        for (position, (kind, expected_kind)) in kinds.into_iter().enumerate() {
            let event = RevisionEvent::new(
                RevisionId::from_bytes([1; 16]),
                u32::try_from(position).expect("small position"),
                kind,
            );
            let projected = AbiRevisionEvent::try_from(&event)
                .expect("project job event")
                .as_abi();
            assert_eq!(projected.kind, expected_kind);
            assert_eq!(projected.job_id.bytes, job_id.into_bytes());
            assert_eq!(projected.activity_id.bytes, [0; 16]);
            assert_eq!(projected.target.kind, 0);
        }
    }
}
