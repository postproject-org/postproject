use std::{
    os::raw::c_char,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
};

use postproject_core::{AssetId, CancellationToken, Error, ErrorKind, RepresentationResolution};
use postproject_media::{
    MediaResolver, MediaRootMapping, ResolutionItem, ResolverOptions, SearchScope, VerificationMode,
};

use crate::{
    PpAssetId, PpError, PpProduction, PpResolutionSet, ffi_call, initialize_output,
    invalid_argument, lock_production, require_output, required_utf8,
};

const PP_VERIFY_PRESENCE: u32 = 1;
const PP_VERIFY_CONTENT: u32 = 2;

/// Opaque cancellation flag shared by the caller and running operations.
pub struct PpCancelToken {
    token: CancellationToken,
}

/// Opaque, caller-owned options for one or more resolution calls.
#[derive(Default)]
pub struct PpResolutionOptions {
    root_mappings: Vec<MediaRootMapping>,
    search_directories: Vec<PathBuf>,
    resolver: ResolverOptions,
}

/// Creates a cancellation token that is not cancelled.
///
/// # Safety
///
/// `out_token` must be writable; `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_cancel_token_create(
    out_token: *mut *mut PpCancelToken,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The output is validated before it is written.
    unsafe {
        initialize_output(out_token);
        ffi_call(out_error, || {
            require_output(out_token, "out_token")?;
            out_token.write(Box::into_raw(Box::new(PpCancelToken {
                token: CancellationToken::new(),
            })));
            Ok(())
        })
    }
}

/// Requests cancellation. Callable from any thread; null is a no-op.
///
/// # Safety
///
/// `token` must be null or live for the duration of the call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_cancel_token_cancel(token: *const PpCancelToken) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: A non-null token is live for this call by the caller contract.
        if let Some(token) = unsafe { token.as_ref() } {
            token.token.cancel();
        }
    }));
}

/// Releases a token. Options that captured it keep observing the same flag.
///
/// # Safety
///
/// `token` must be null or a live handle that no other thread is using.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_cancel_token_release(token: *mut PpCancelToken) {
    if token.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership of a live allocation is transferred exactly once.
        drop(unsafe { Box::from_raw(token) });
    }));
}

/// Creates resolution options with presence verification, default limits,
/// no mappings, no search directories, and no cancellation token.
///
/// # Safety
///
/// `out_options` must be writable; `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_options_create(
    out_options: *mut *mut PpResolutionOptions,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The output is validated before it is written.
    unsafe {
        initialize_output(out_options);
        ffi_call(out_error, || {
            require_output(out_options, "out_options")?;
            out_options.write(Box::into_raw(Box::default()));
            Ok(())
        })
    }
}

/// Releases resolution options. Null is a no-op.
///
/// # Safety
///
/// `options` must be null or a live handle not used after this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_options_release(options: *mut PpResolutionOptions) {
    if options.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership of a live allocation is transferred exactly once.
        drop(unsafe { Box::from_raw(options) });
    }));
}

/// Maps a logical media root to an existing directory on this machine.
///
/// # Safety
///
/// `options` must be live; `name` and `directory` must be NUL-terminated UTF-8.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_options_add_root_mapping(
    options: *mut PpResolutionOptions,
    name: *const c_char,
    directory: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointers are checked before dereference; strings are copied.
    unsafe {
        ffi_call(out_error, || {
            let options = options
                .as_mut()
                .ok_or_else(|| invalid_argument("options must not be null"))?;
            let mapping = MediaRootMapping::new(
                required_utf8(name, "root mapping name")?,
                Path::new(required_utf8(directory, "root mapping directory")?),
            )?;
            if options
                .root_mappings
                .iter()
                .any(|existing| existing.name() == mapping.name())
            {
                return Err(Error::new(
                    ErrorKind::AlreadyExists,
                    format!("media root {} is already mapped", mapping.name()),
                ));
            }
            options.root_mappings.push(mapping);
            Ok(())
        })
    }
}

/// Adds an unnamed, machine-local directory searched after the mapped roots.
///
/// # Safety
///
/// `options` must be live; `directory` must be NUL-terminated UTF-8.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_options_add_search_directory(
    options: *mut PpResolutionOptions,
    directory: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointers are checked before dereference; strings are copied.
    unsafe {
        ffi_call(out_error, || {
            let options = options
                .as_mut()
                .ok_or_else(|| invalid_argument("options must not be null"))?;
            let directory = required_utf8(directory, "search directory")?;
            if directory.is_empty() {
                return Err(invalid_argument("search directory must not be empty"));
            }
            options.search_directories.push(PathBuf::from(directory));
            Ok(())
        })
    }
}

/// Selects presence or content verification for resources at known locators.
///
/// # Safety
///
/// `options` must be live.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_options_set_verification(
    options: *mut PpResolutionOptions,
    verification: u32,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointers are checked before dereference.
    unsafe {
        ffi_call(out_error, || {
            let options = options
                .as_mut()
                .ok_or_else(|| invalid_argument("options must not be null"))?;
            options.resolver.verification = match verification {
                PP_VERIFY_PRESENCE => VerificationMode::Presence,
                PP_VERIFY_CONTENT => VerificationMode::Content,
                _ => return Err(invalid_argument("unknown verification mode")),
            };
            Ok(())
        })
    }
}

/// Sets the depth and entry budget applied to each searched directory.
///
/// # Safety
///
/// `options` must be live.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_options_set_limits(
    options: *mut PpResolutionOptions,
    max_depth: u32,
    max_entries_per_directory: u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointers are checked before dereference.
    unsafe {
        ffi_call(out_error, || {
            let options = options
                .as_mut()
                .ok_or_else(|| invalid_argument("options must not be null"))?;
            if max_depth == 0 || max_entries_per_directory == 0 {
                return Err(invalid_argument(
                    "resolution depth and entry limits must be greater than zero",
                ));
            }
            let max_depth = usize::try_from(max_depth)
                .map_err(|_| invalid_argument("max_depth is too large"))?;
            let max_entries_per_directory = usize::try_from(max_entries_per_directory)
                .map_err(|_| invalid_argument("max_entries_per_directory is too large"))?;
            options.resolver.max_depth = max_depth;
            options.resolver.max_entries_per_directory = max_entries_per_directory;
            Ok(())
        })
    }
}

/// Observes a cancellation token; a null token removes it. The options share
/// the token's flag, so the token may be released independently.
///
/// # Safety
///
/// `options` must be live; `token` must be null or live for this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_options_set_cancel_token(
    options: *mut PpResolutionOptions,
    token: *const PpCancelToken,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointers are checked before dereference.
    unsafe {
        ffi_call(out_error, || {
            let options = options
                .as_mut()
                .ok_or_else(|| invalid_argument("options must not be null"))?;
            options.resolver.cancellation = token.as_ref().map(|token| token.token.clone());
            Ok(())
        })
    }
}

/// Resolves every representation of each asset with one scan of the scope.
///
/// # Safety
///
/// `production` must be live; `asset_ids` must point to `asset_count`
/// readable IDs (it may be null only when the count is zero); `options` must
/// be null or live; `out_resolutions` must be writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_resolve_assets(
    production: *const PpProduction,
    asset_ids: *const PpAssetId,
    asset_count: u64,
    options: *const PpResolutionOptions,
    out_resolutions: *mut *mut PpResolutionSet,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference; the ID array is
    // read only within the caller-provided bounds.
    unsafe {
        initialize_output(out_resolutions);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            require_output(out_resolutions, "out_resolutions")?;
            let count = usize::try_from(asset_count)
                .map_err(|_| invalid_argument("asset_count is too large"))?;
            if count > (isize::MAX as usize) / size_of::<PpAssetId>() {
                return Err(invalid_argument("asset_count is not addressable"));
            }
            if count != 0 && asset_ids.is_null() {
                return Err(invalid_argument(
                    "asset_ids must not be null when asset_count is nonzero",
                ));
            }
            let asset_ids = if count == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(asset_ids, count)
            };
            let default_options = PpResolutionOptions::default();
            let options = options.as_ref().unwrap_or(&default_options);
            let resolutions = resolve_assets(production, asset_ids, options)?;
            out_resolutions.write(Box::into_raw(Box::new(PpResolutionSet::new(resolutions))));
            Ok(())
        })
    }
}

type AssetWork = (
    AssetId,
    postproject_core::Representation,
    Vec<(postproject_core::Resource, Vec<postproject_core::Locator>)>,
);

fn resolve_assets(
    production: &PpProduction,
    asset_ids: &[PpAssetId],
    options: &PpResolutionOptions,
) -> Result<Vec<(AssetId, RepresentationResolution)>, Error> {
    // Snapshot the stored knowledge, then scan and hash without the lock.
    let (media_roots, work) = {
        let inner = lock_production(&production.state);
        let mut work: Vec<AssetWork> = Vec::new();
        for asset_id in asset_ids {
            let asset_id = AssetId::from_bytes(asset_id.bytes);
            for representation in inner.representations(asset_id)? {
                let mut resources = Vec::new();
                for resource in inner.resources(representation.id())? {
                    let locators = inner.locators(resource.id())?;
                    resources.push((resource, locators));
                }
                work.push((asset_id, representation, resources));
            }
            if !work.iter().any(|(owner, _, _)| *owner == asset_id)
                && !inner.assets()?.iter().any(|asset| asset.id() == asset_id)
            {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("asset {asset_id} does not exist"),
                ));
            }
        }
        (inner.production().media_roots().to_vec(), work)
    };

    let scope = options.search_directories.iter().fold(
        SearchScope::new(media_roots, options.root_mappings.clone()),
        SearchScope::with_search_directory,
    );
    let items = work
        .iter()
        .flat_map(|(_, representation, resources)| {
            resources.iter().map(|(resource, locators)| {
                ResolutionItem::new(resource, representation.content_structure(), locators)
            })
        })
        .collect::<Vec<_>>();
    let mut results = MediaResolver::new(options.resolver.clone())?
        .resolve(&items, &scope)?
        .into_iter();
    work.iter()
        .map(|(asset_id, representation, resources)| {
            RepresentationResolution::aggregate(
                representation.id(),
                representation.content_structure(),
                results.by_ref().take(resources.len()).collect(),
            )
            .map(|resolution| (*asset_id, resolution))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::*;
    use crate::{PP_ERROR_INVALID_ARGUMENT, PP_OK, production_handle};

    #[test]
    fn resolution_rejects_unaddressable_arrays_and_accepts_empty_input() {
        let directory = tempfile::tempdir().expect("create directory");
        let production = production_handle(
            postproject_storage_sqlite::SqliteProduction::create(
                directory.path().join("production.pproj"),
                None,
            )
            .expect("create production"),
        );
        let mut session = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: The production is live and the outputs are writable.
        assert_eq!(
            unsafe {
                crate::read_session::pp_production_read_session(
                    &raw const production,
                    &raw mut session,
                    &raw mut error,
                )
            },
            PP_OK
        );
        let asset = PpAssetId { bytes: [0; 16] };
        let first_unaddressable = (isize::MAX as u64) / size_of::<PpAssetId>() as u64 + 1;
        for retained in [false, true] {
            for count in [first_unaddressable, u64::MAX, 0] {
                let mut resolutions = ptr::dangling_mut();
                // SAFETY: Oversized counts reject before the live asset is
                // accessed; the zero-count case permits a null input.
                let status = unsafe {
                    let ids = if count == 0 {
                        ptr::null()
                    } else {
                        &raw const asset
                    };
                    if retained {
                        crate::read_queries::pp_read_session_resolve_assets(
                            session,
                            ids,
                            count,
                            ptr::null(),
                            &raw mut resolutions,
                            &raw mut error,
                        )
                    } else {
                        pp_production_resolve_assets(
                            &raw const production,
                            ids,
                            count,
                            ptr::null(),
                            &raw mut resolutions,
                            &raw mut error,
                        )
                    }
                };
                if count == 0 {
                    assert_eq!(status, PP_OK);
                    assert!(error.is_null());
                    // SAFETY: The successful call returned a live owned set.
                    unsafe { crate::pp_resolution_set_release(resolutions) };
                } else {
                    assert_eq!(status, PP_ERROR_INVALID_ARGUMENT);
                    assert!(resolutions.is_null());
                    assert!(!error.is_null());
                    // SAFETY: Release the one owned error returned on failure.
                    unsafe { crate::pp_error_release(error) };
                    error = ptr::null_mut();
                }
            }
        }
        // SAFETY: The session is live and released once after its reads.
        unsafe { crate::read_session::pp_read_session_release(session) };
    }
}
