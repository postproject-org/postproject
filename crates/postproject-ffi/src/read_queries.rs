//! Snapshot point reads and bounded pages using existing owned projections.

use crate::{
    AbiAsset, PpAssetSet, PpError, PpReadSession, PpRepresentationSet, PpUuid, ffi_call,
    initialize_output, invalid_argument, lock_production, query_cursor_to_cstring,
    query_page_request, require_output,
};
use postproject_core::{AssetId, Error, RepresentationId};
use std::ffi::c_char;

// Reuse live-read validation and owned projections on the pinned connection.
// NULL sessions delegate as NULL productions, which clear every output.
unsafe fn forward_read(
    session: *const PpReadSession,
    out_error: *mut *mut PpError,
    operation: impl FnOnce(*const crate::PpProduction) -> u32,
) -> u32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: A non-null session is live by the exported caller contract.
        let reader = unsafe { session.as_ref() }
            .map_or(std::ptr::null(), |session| &raw const session.reader);
        operation(reader)
    }))
    .unwrap_or_else(|_| {
        // SAFETY: Error output validity is the exported caller contract.
        unsafe {
            ffi_call(out_error, || {
                Err(Error::new(
                    postproject_core::ErrorKind::Internal,
                    "read projection panicked",
                ))
            })
        }
    })
}

/// Plans a bounded artifact list against the pinned view without enqueuing jobs.
///
/// # Safety
/// Session must be live, IDs readable for count (null for zero), outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_plan_regeneration(
    session: *const PpReadSession,
    artifact_representation_ids: *const PpUuid,
    artifact_count: u64,
    out_plans: *mut *mut crate::PpRegenerationPlanSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_plan_regeneration(
                reader,
                artifact_representation_ids,
                artifact_count,
                out_plans,
                out_error,
            )
        })
    }
}

/// Copies one bounded page of a revision's events from the pinned view.
///
/// # Safety
/// Session must be live, cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_revision_events_page(
    session: *const PpReadSession,
    revision_id: crate::PpRevisionId,
    limit: u32,
    cursor: *const c_char,
    out_events: *mut *mut crate::PpRevisionEventSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::revision_events::pp_production_revision_events_page(
                reader,
                revision_id,
                limit,
                cursor,
                out_events,
                out_error,
            )
        })
    }
}

/// Copies a bounded filtered journal page ending at the pinned view's head.
///
/// # Safety
/// Session must be live; kinds readable for count, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_changes_since_filtered(
    session: *const PpReadSession,
    sequence: u64,
    kinds: *const u32,
    kind_count: u64,
    limit: u32,
    out_revisions: *mut *mut crate::PpRevisionSet,
    out_through_sequence: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::revision_waits::pp_production_changes_since_filtered(
                reader,
                sequence,
                kinds,
                kind_count,
                limit,
                out_revisions,
                out_through_sequence,
                out_error,
            )
        })
    }
}

/// Copies the latest revision retained by the pinned view, or an empty set.
///
/// # Safety
/// Session must be live; outputs writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_latest_revision(
    session: *const PpReadSession,
    out_revisions: *mut *mut crate::PpRevisionSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_latest_revision(reader, out_revisions, out_error)
        })
    }
}

/// Copies bounded ascending revisions through the pinned journal head.
///
/// # Safety
/// Session must be live; outputs writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_changes_since(
    session: *const PpReadSession,
    sequence: u64,
    limit: u32,
    out_revisions: *mut *mut crate::PpRevisionSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_changes_since(reader, sequence, limit, out_revisions, out_error)
        })
    }
}

/// Copies a bounded page of producing activities from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_activities_producing_page(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_activities: *mut *mut crate::PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_activities_producing_page(
                reader,
                representation_id,
                limit,
                cursor,
                out_activities,
                out_error,
            )
        })
    }
}

/// Copies a bounded page of consuming activities from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_activities_consuming_page(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_activities: *mut *mut crate::PpActivitySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_activities_consuming_page(
                reader,
                representation_id,
                limit,
                cursor,
                out_activities,
                out_error,
            )
        })
    }
}

/// Copies outputs matching one exact activity kind in the pinned view.
///
/// # Safety
/// Session must be live; strings null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_outputs_by_activity_kind(
    session: *const PpReadSession,
    kind: *const c_char,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_outputs_by_activity_kind(
                reader,
                kind,
                limit,
                cursor,
                out_objects,
                out_error,
            )
        })
    }
}

/// Copies outputs matching one exact tool identity in the pinned view.
///
/// # Safety
/// Session must be live; strings null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_outputs_by_tool(
    session: *const PpReadSession,
    name: *const c_char,
    version: *const c_char,
    uri: *const c_char,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_outputs_by_tool(
                reader,
                name,
                version,
                uri,
                limit,
                cursor,
                out_objects,
                out_error,
            )
        })
    }
}

/// Copies bounded shortest-depth provenance ancestors from the pinned view.
///
/// # Safety
/// Session/IDs must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_provenance_ancestors_page(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_provenance_ancestors_page(
                reader,
                representation_id,
                max_depth,
                max_representations,
                limit,
                cursor,
                out_objects,
                out_error,
            )
        })
    }
}

/// Copies bounded shortest-depth provenance descendants from the pinned view.
///
/// # Safety
/// Session/IDs must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_provenance_descendants_page(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_provenance_descendants_page(
                reader,
                representation_id,
                max_depth,
                max_representations,
                limit,
                cursor,
                out_objects,
                out_error,
            )
        })
    }
}

/// Evaluates a bounded stale-artifact page from the pinned knowledge.
///
/// # Safety
/// Session/IDs must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_stale_artifacts(
    session: *const PpReadSession,
    source_representation_id: *const PpUuid,
    evaluation_max_depth: u32,
    evaluation_max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_stale_artifacts(
                reader,
                source_representation_id,
                evaluation_max_depth,
                evaluation_max_representations,
                limit,
                cursor,
                out_objects,
                out_error,
            )
        })
    }
}

/// Copies a bounded page of representations associated with a logical root.
///
/// # Safety
/// Session must be live; strings null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_representations_under_media_root(
    session: *const PpReadSession,
    root_name: *const c_char,
    limit: u32,
    cursor: *const c_char,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::representations::pp_production_representations_under_media_root(
                reader,
                root_name,
                limit,
                cursor,
                out_representations,
                out_error,
            )
        })
    }
}

/// Copies a bounded page of unresolved representation knowledge.
///
/// # Safety
/// Session must be live; strings null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_unresolved_media(
    session: *const PpReadSession,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_unresolved_media(reader, limit, cursor, out_objects, out_error)
        })
    }
}

/// Copies changed objects through the pinned journal head.
///
/// # Safety
/// Session must be live; strings null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_objects_changed_since(
    session: *const PpReadSession,
    sequence: u64,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_objects_changed_since(
                reader,
                sequence,
                limit,
                cursor,
                out_objects,
                out_error,
            )
        })
    }
}

/// Copies recorded dependency knowledge, preserving absence versus an empty set.
///
/// # Safety
/// Session/ID must be live/readable; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_dependency_set(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    out_dependencies: *mut *mut crate::PpDependencySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_dependency_set(
                reader,
                representation_id,
                out_dependencies,
                out_error,
            )
        })
    }
}

/// Traverses dependency knowledge in the pinned view with explicit bounds.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_dependencies(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut crate::PpDependencyQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_dependencies(
                reader,
                representation_id,
                max_depth,
                max_representations,
                limit,
                cursor,
                out_matches,
                out_error,
            )
        })
    }
}

/// Traverses reverse dependency knowledge within the same bounded pinned view.
///
/// # Safety
/// Session/target must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_dependents(
    session: *const PpReadSession,
    target: *const crate::PpObjectRef,
    max_depth: u32,
    max_representations: u32,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut crate::PpDependencyQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_dependents(
                reader,
                target,
                max_depth,
                max_representations,
                limit,
                cursor,
                out_matches,
                out_error,
            )
        })
    }
}

/// Evaluates stored artifact evidence through the pinned view, without media I/O.
///
/// # Safety
/// Session/ID must be live/readable; outputs writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_evaluate_artifact(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    max_depth: u32,
    max_representations: u32,
    out_evaluation: *mut *mut crate::PpArtifactEvaluation,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_evaluate_artifact(
                reader,
                representation_id,
                max_depth,
                max_representations,
                out_evaluation,
                out_error,
            )
        })
    }
}

/// Copies artifact reproducibility knowledge from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; outputs writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_artifact_reproducibility(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    out_report: *mut *mut crate::PpArtifactReproducibility,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_artifact_reproducibility(
                reader,
                representation_id,
                out_report,
                out_error,
            )
        })
    }
}

/// Copies one job from the pinned view; absent identities return not found.
///
/// # Safety
/// Session/ID must be live/readable; output writable, error nullable/writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_job(
    session: *const PpReadSession,
    job_id: *const PpUuid,
    out_jobs: *mut *mut crate::PpJobSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_job(reader, job_id, out_jobs, out_error)
        })
    }
}

/// Copies a bounded filtered job page from the pinned view.
///
/// # Safety
/// Session must be live; kind/cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_jobs(
    session: *const PpReadSession,
    state: u32,
    kind: *const c_char,
    limit: u32,
    cursor: *const c_char,
    out_jobs: *mut *mut crate::PpJobSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_jobs(reader, state, kind, limit, cursor, out_jobs, out_error)
        })
    }
}

/// Copies a bounded resource page from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_resources_page(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_objects: *mut *mut crate::PpObjectQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_resources_page(
                reader,
                representation_id,
                limit,
                cursor,
                out_objects,
                out_error,
            )
        })
    }
}

/// Copies a bounded locator page from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_locators_page(
    session: *const PpReadSession,
    resource_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_locators: *mut *mut crate::PpLocatorQuerySet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_locators_page(
                reader,
                resource_id,
                limit,
                cursor,
                out_locators,
                out_error,
            )
        })
    }
}

/// Copies representations using a resource from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_representations_using_resource(
    session: *const PpReadSession,
    resource_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::representations::pp_production_representations_using_resource(
                reader,
                resource_id,
                limit,
                cursor,
                out_representations,
                out_error,
            )
        })
    }
}

/// Copies logical roots from the pinned view into an owned set.
///
/// # Safety
/// Session must be live; outputs writable, with a nullable error output.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_media_roots(
    session: *const PpReadSession,
    out_roots: *mut *mut crate::PpMediaRootSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_media_roots(reader, out_roots, out_error)
        })
    }
}

/// Resolves current files using storage knowledge from the pinned view.
///
/// # Safety
/// Session must be live; IDs readable for count (null for zero), options null
/// or live, outputs writable. The filesystem is not part of the database view.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_resolve_assets(
    session: *const PpReadSession,
    asset_ids: *const PpUuid,
    asset_count: u64,
    options: *const crate::PpResolutionOptions,
    out_resolutions: *mut *mut crate::PpResolutionSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::resolution::pp_production_resolve_assets(
                reader,
                asset_ids,
                asset_count,
                options,
                out_resolutions,
                out_error,
            )
        })
    }
}

/// Compares current files with resource fingerprints from the pinned view.
///
/// # Safety
/// Session/ID must be live/readable; path UTF-8/NUL-terminated, naming null or
/// readable, outputs writable. Filesystem contents may change independently.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_verify_resource(
    session: *const PpReadSession,
    resource_id: *const PpUuid,
    path: *const c_char,
    sequence_naming: *const crate::PpSequenceNaming,
    out_verification: *mut u32,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::content::pp_production_verify_resource(
                reader,
                resource_id,
                path,
                sequence_naming,
                out_verification,
                out_error,
            )
        })
    }
}

/// Copies an object's metadata from the pinned view.
///
/// # Safety
/// Session/target must be live/readable; outputs writable or error output null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_metadata(
    session: *const PpReadSession,
    target: *const crate::PpObjectRef,
    out_metadata: *mut *mut crate::PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_metadata(reader, target, out_metadata, out_error)
        })
    }
}

/// Copies assertions of one property from the pinned view.
///
/// # Safety
/// Session must be live; strings UTF-8/NUL-terminated, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_find_metadata(
    session: *const PpReadSession,
    vocabulary: *const c_char,
    property: *const c_char,
    out_metadata: *mut *mut crate::PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_find_metadata(
                reader,
                vocabulary,
                property,
                out_metadata,
                out_error,
            )
        })
    }
}

/// Copies a bounded metadata page from the pinned view.
///
/// # Safety
/// Session must be live; strings UTF-8/NUL-terminated, exact value readable or
/// null, cursor nullable UTF-8/NUL-terminated, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_query_metadata(
    session: *const PpReadSession,
    vocabulary: *const c_char,
    property: *const c_char,
    exact_value: *const crate::PpMetadataInput,
    limit: u32,
    cursor: *const c_char,
    out_metadata: *mut *mut crate::PpMetadataSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_query_metadata(
                reader,
                vocabulary,
                property,
                exact_value,
                limit,
                cursor,
                out_metadata,
                out_error,
            )
        })
    }
}

/// Copies external identifiers from the pinned view into an owned set.
///
/// # Safety
/// Session/target must be live/readable; outputs writable or error output null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_external_identifiers(
    session: *const PpReadSession,
    target: *const crate::PpObjectRef,
    out_identifiers: *mut *mut crate::PpExternalIdentifierSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_external_identifiers(reader, target, out_identifiers, out_error)
        })
    }
}

/// Looks up exact external identifiers in the pinned view.
///
/// # Safety
/// Session must be live; strings UTF-8/NUL-terminated (qualifier nullable), outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_find_by_external_identifier(
    session: *const PpReadSession,
    scheme: *const c_char,
    value: *const c_char,
    qualifier: *const c_char,
    out_objects: *mut *mut crate::PpObjectRefSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::pp_production_find_by_external_identifier(
                reader,
                scheme,
                value,
                qualifier,
                out_objects,
                out_error,
            )
        })
    }
}

/// Finds known media by exact locator identity in the pinned view.
///
/// # Safety
/// Session must be live; URI UTF-8/NUL-terminated, optional naming readable,
/// cursor nullable UTF-8/NUL-terminated, outputs writable or error output null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_find_known_media_by_locator(
    session: *const PpReadSession,
    uri: *const c_char,
    sequence_naming: *const crate::PpSequenceNaming,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut crate::PpKnownMediaSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::known_media::pp_production_find_known_media_by_locator(
                reader,
                uri,
                sequence_naming,
                limit,
                cursor,
                out_matches,
                out_error,
            )
        })
    }
}

/// Finds known media by fingerprint evidence in the pinned view.
///
/// # Safety
/// Session must be live; algorithm UTF-8/NUL-terminated, value readable for length,
/// cursor nullable UTF-8/NUL-terminated, outputs writable or error output null.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_find_known_media_by_fingerprint(
    session: *const PpReadSession,
    algorithm: *const c_char,
    version: u16,
    value: *const u8,
    value_length: u64,
    limit: u32,
    cursor: *const c_char,
    out_matches: *mut *mut crate::PpKnownMediaSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The delegate validates pointers and contains panics.
    unsafe {
        forward_read(session, out_error, |reader| {
            crate::known_media::pp_production_find_known_media_by_fingerprint(
                reader,
                algorithm,
                version,
                value,
                value_length,
                limit,
                cursor,
                out_matches,
                out_error,
            )
        })
    }
}

/// Reads copied values from the session's pinned view.
///
/// # Safety
///
/// Session and IDs must be live/readable, cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_assets_page(
    session: *const PpReadSession,
    limit: u32,
    cursor: *const c_char,
    out_assets: *mut *mut PpAssetSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_assets);
        ffi_call(out_error, || {
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            require_output(out_assets, "out_assets")?;
            let page_request = query_page_request(limit, cursor)?;
            let page = lock_production(&session.reader.state).assets_page(&page_request)?;
            let assets = page
                .items()
                .iter()
                .map(AbiAsset::try_from)
                .collect::<Result<Vec<_>, Error>>()?;
            out_assets.write(Box::into_raw(Box::new(PpAssetSet {
                assets,
                next_cursor: query_cursor_to_cstring(page.next_cursor())?,
            })));
            Ok(())
        })
    }
}

/// Reads copied values from the session's pinned view.
///
/// # Safety
///
/// Session and IDs must be live/readable, cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_asset(
    session: *const PpReadSession,
    asset_id: *const PpUuid,
    out_assets: *mut *mut PpAssetSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are initialized and pointers checked before use.
    unsafe {
        initialize_output(out_assets);
        ffi_call(out_error, || {
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            let asset_id = asset_id
                .as_ref()
                .ok_or_else(|| invalid_argument("asset_id must not be null"))?;
            require_output(out_assets, "out_assets")?;
            let asset = lock_production(&session.reader.state)
                .asset(AssetId::from_bytes(asset_id.bytes))?;
            out_assets.write(Box::into_raw(Box::new(PpAssetSet {
                assets: vec![AbiAsset::try_from(&asset)?],
                next_cursor: None,
            })));
            Ok(())
        })
    }
}

/// Reads copied values from the session's pinned view.
///
/// # Safety
///
/// Session and IDs must be live/readable, cursor null or UTF-8, outputs writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_representations_page(
    session: *const PpReadSession,
    asset_id: *const PpUuid,
    limit: u32,
    cursor: *const c_char,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            let asset_id = asset_id
                .as_ref()
                .ok_or_else(|| invalid_argument("asset_id must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let page_request = query_page_request(limit, cursor)?;
            let inner = lock_production(&session.reader.state);
            let page =
                inner.representations_page(AssetId::from_bytes(asset_id.bytes), &page_request)?;
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet::new_page(
                &*inner,
                page.items(),
                page.next_cursor(),
            )?)));
            Ok(())
        })
    }
}

/// Reads an owned representation from the pinned view.
///
/// # Safety
///
/// Session and ID must be live/readable and output pointers valid.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_read_session_representation(
    session: *const PpReadSession,
    representation_id: *const PpUuid,
    out_representations: *mut *mut PpRepresentationSet,
    out_error: *mut *mut PpError,
) -> u32 {
    unsafe {
        initialize_output(out_representations);
        ffi_call(out_error, || {
            let session = session
                .as_ref()
                .ok_or_else(|| invalid_argument("session must not be null"))?;
            let representation_id = representation_id
                .as_ref()
                .ok_or_else(|| invalid_argument("representation_id must not be null"))?;
            require_output(out_representations, "out_representations")?;
            let inner = lock_production(&session.reader.state);
            let representation =
                inner.representation(RepresentationId::from_bytes(representation_id.bytes))?;
            out_representations.write(Box::into_raw(Box::new(PpRepresentationSet::new_page(
                &*inner,
                &[representation],
                None,
            )?)));
            Ok(())
        })
    }
}
