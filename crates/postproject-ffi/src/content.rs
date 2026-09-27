use std::{
    ffi::CString,
    os::raw::c_char,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};

use postproject_core::{Error, ErrorKind, ResourceFingerprint, ResourceId};
use postproject_media::{
    ContentVerification, fingerprint_file, observe_resource_content, resource_usage,
    verify_resource_content,
};

use crate::{
    PpError, PpProduction, PpTransaction, PpUuid, StagedMutation, ffi_call,
    initialize_const_output, initialize_output, initialize_value, invalid_argument,
    lock_production, require_output, required_utf8,
};

const PP_CONTENT_MATCHES: u32 = 1;
const PP_CONTENT_DIFFERS: u32 = 2;
const PP_CONTENT_NOT_COMPARABLE: u32 = 3;

/// Opaque immutable fingerprint value owned by the C caller.
pub struct PpFingerprint {
    algorithm: CString,
    version: u16,
    value: Vec<u8>,
}

/// Computes the fingerprint import would record for a regular file.
///
/// # Safety
///
/// `path` must be NUL-terminated UTF-8. `out_fingerprint` must be writable;
/// `out_error` follows the library error contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_fingerprint_file(
    path: *const c_char,
    out_fingerprint: *mut *mut PpFingerprint,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Outputs are validated before writing; inputs are copied.
    unsafe {
        initialize_output(out_fingerprint);
        ffi_call(out_error, || {
            require_output(out_fingerprint, "out_fingerprint")?;
            let path = required_utf8(path, "path")?;
            let report = fingerprint_file(Path::new(path))?;
            let fingerprint = report.fingerprint();
            out_fingerprint.write(Box::into_raw(Box::new(PpFingerprint {
                algorithm: CString::new(fingerprint.algorithm()).map_err(|_| {
                    Error::new(ErrorKind::Internal, "fingerprint algorithm contains NUL")
                })?,
                version: fingerprint.version(),
                value: fingerprint.value().to_vec(),
            })));
            Ok(())
        })
    }
}

/// Reads a fingerprint's domain and bytes, borrowed until release.
///
/// # Safety
///
/// `fingerprint` must be a live handle; every output must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_fingerprint_get(
    fingerprint: *const PpFingerprint,
    out_algorithm: *mut *const c_char,
    out_version: *mut u16,
    out_value: *mut *const u8,
    out_value_length: *mut u64,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference or write.
    unsafe {
        initialize_const_output(out_algorithm);
        initialize_value(out_version, 0);
        initialize_const_output(out_value);
        initialize_value(out_value_length, 0);
        ffi_call(out_error, || {
            let fingerprint = fingerprint
                .as_ref()
                .ok_or_else(|| invalid_argument("fingerprint must not be null"))?;
            require_output(out_algorithm, "out_algorithm")?;
            require_output(out_version, "out_version")?;
            require_output(out_value, "out_value")?;
            require_output(out_value_length, "out_value_length")?;
            out_algorithm.write(fingerprint.algorithm.as_ptr());
            out_version.write(fingerprint.version);
            out_value.write(fingerprint.value.as_ptr());
            out_value_length.write(
                u64::try_from(fingerprint.value.len())
                    .map_err(|_| Error::new(ErrorKind::Internal, "fingerprint is too long"))?,
            );
            Ok(())
        })
    }
}

/// Releases a fingerprint. Null is a no-op.
///
/// # Safety
///
/// `fingerprint` must be null or a live handle not used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_fingerprint_release(fingerprint: *mut PpFingerprint) {
    if fingerprint.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Ownership of a live allocation is required by this function's
        // contract and is reconstructed exactly once here.
        drop(unsafe { Box::from_raw(fingerprint) });
    }));
}

/// Compares present content with a resource's stored fingerprints.
///
/// # Safety
///
/// `production` and `resource_id` must be live; `path` must be NUL-terminated
/// UTF-8; `out_verification` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_production_verify_resource(
    production: *const PpProduction,
    resource_id: *const PpUuid,
    path: *const c_char,
    out_verification: *mut u32,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference or write.
    unsafe {
        initialize_value(out_verification, 0);
        ffi_call(out_error, || {
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            let resource_id = resource_id
                .as_ref()
                .ok_or_else(|| invalid_argument("resource_id must not be null"))?;
            require_output(out_verification, "out_verification")?;
            let path = required_utf8(path, "path")?;
            let resource_id = ResourceId::from_bytes(resource_id.bytes);
            let usage = resource_usage(&*lock_production(&production.state), resource_id)?;
            let (representation, resources) = usage.first().ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!("resource {resource_id} is not used by any representation"),
                )
            })?;
            let resource = resources
                .iter()
                .find(|resource| resource.id() == resource_id)
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::Internal,
                        "representation does not list the resource that uses it",
                    )
                })?;
            let verification = verify_resource_content(
                resource,
                representation.content_structure(),
                Path::new(path),
            )?;
            out_verification.write(match verification {
                ContentVerification::Matches => PP_CONTENT_MATCHES,
                ContentVerification::Differs => PP_CONTENT_DIFFERS,
                _ => PP_CONTENT_NOT_COMPARABLE,
            });
            Ok(())
        })
    }
}

/// Stages present content as a new observation of a resource and of every
/// representation using it.
///
/// # Safety
///
/// `transaction` and `resource_id` must be live; `path` must be
/// NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_transaction_observe_resource_content(
    transaction: *mut PpTransaction,
    resource_id: *const PpUuid,
    path: *const c_char,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let resource_id = resource_id
                .as_ref()
                .ok_or_else(|| invalid_argument("resource_id must not be null"))?;
            let path = required_utf8(path, "path")?;
            let resource_id = ResourceId::from_bytes(resource_id.bytes);
            let mut usage = resource_usage(&*lock_production(&transaction.state), resource_id)?;
            // Earlier observations staged in this transaction are current for
            // the representations recomputed now.
            for (_, resources) in &mut usage {
                for resource in resources.iter_mut() {
                    for fingerprint in staged_resource_fingerprints(transaction, resource.id()) {
                        *resource = resource.with_observed_fingerprint(fingerprint);
                    }
                }
            }
            let observation = observe_resource_content(resource_id, &usage, Path::new(path))?;
            transaction
                .mutations
                .push(StagedMutation::RecordResourceFingerprint(
                    resource_id,
                    observation.resource().clone(),
                ));
            for (representation_id, fingerprint) in observation.representations() {
                transaction
                    .mutations
                    .push(StagedMutation::RecordRepresentationFingerprint(
                        *representation_id,
                        fingerprint.clone(),
                    ));
            }
            Ok(())
        })
    }
}

fn staged_resource_fingerprints(
    transaction: &PpTransaction,
    resource_id: ResourceId,
) -> impl Iterator<Item = &ResourceFingerprint> {
    transaction
        .mutations
        .iter()
        .filter_map(move |mutation| match mutation {
            StagedMutation::RecordResourceFingerprint(id, fingerprint) if *id == resource_id => {
                Some(fingerprint)
            }
            _ => None,
        })
}
