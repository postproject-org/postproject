use std::{
    ffi::CString,
    os::raw::c_char,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
};

use postproject_core::{
    Error, ErrorKind, Locator, ResourceFingerprint, ResourceId, SequenceNaming,
};
use postproject_media::{
    ContentObservationOutcome, ContentVerification, fingerprint_file, observe_resource_content,
    recorded_sequence_naming, resource_usage, verify_resource_content,
};

use crate::{
    PpError, PpProduction, PpSequenceNaming, PpTransaction, PpUuid, StagedMutation, ffi_call,
    initialize_const_output, initialize_output, initialize_value, invalid_argument,
    lock_production, require_output, required_utf8, sequence_naming::optional_naming,
};

const PP_CONTENT_MATCHES: u32 = 1;
const PP_CONTENT_DIFFERS: u32 = 2;
const PP_CONTENT_NOT_COMPARABLE: u32 = 3;
const PP_OBSERVATION_UNCHANGED: u32 = 1;
const PP_OBSERVATION_CHANGED: u32 = 2;
const PP_OBSERVATION_FIRST: u32 = 3;

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

/// Returns the explicit naming, or else the one recorded for the directory at
/// `path` among `locators` of a sequence resource.
fn naming_at(
    explicit: Option<SequenceNaming>,
    locators: &[Locator],
    path: &str,
) -> Result<Option<SequenceNaming>, Error> {
    if explicit.is_some()
        || locators
            .iter()
            .all(|locator| locator.sequence_naming().is_none())
    {
        return Ok(explicit);
    }
    recorded_sequence_naming(locators, path)
}

/// Compares present content with a resource's stored fingerprints.
///
/// A null `sequence_naming` means the naming recorded for the directory at
/// `path`.
///
/// # Safety
///
/// `production` and `resource_id` must be live; `path` must be NUL-terminated
/// UTF-8; `sequence_naming` must be null or readable with NUL-terminated
/// strings; `out_verification` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_production_verify_resource(
    production: *const PpProduction,
    resource_id: *const PpUuid,
    path: *const c_char,
    sequence_naming: *const PpSequenceNaming,
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
            let naming = optional_naming(sequence_naming, "sequence_naming")?;
            let resource_id = ResourceId::from_bytes(resource_id.bytes);
            let (usage, naming) = {
                let production = lock_production(&production.state);
                let usage = resource_usage(&*production, resource_id)?;
                let locators = production.locators(resource_id)?;
                (usage, naming_at(naming, &locators, path)?)
            };
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
                naming.as_ref(),
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
/// representation using it, and reports whether the content changed.
///
/// # Safety
///
/// `transaction` and `resource_id` must be live; `path` must be
/// NUL-terminated UTF-8; `sequence_naming` must be null or readable with
/// NUL-terminated strings; `out_outcome` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pp_transaction_observe_resource_content(
    transaction: *mut PpTransaction,
    resource_id: *const PpUuid,
    path: *const c_char,
    sequence_naming: *const PpSequenceNaming,
    out_outcome: *mut u32,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Null pointers are rejected before dereference or write.
    unsafe {
        initialize_value(out_outcome, 0);
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let resource_id = resource_id
                .as_ref()
                .ok_or_else(|| invalid_argument("resource_id must not be null"))?;
            require_output(out_outcome, "out_outcome")?;
            let path = required_utf8(path, "path")?;
            let naming = optional_naming(sequence_naming, "sequence_naming")?;
            let resource_id = ResourceId::from_bytes(resource_id.bytes);
            let (mut usage, mut locators) = {
                let production = lock_production(&transaction.state);
                (
                    resource_usage(&*production, resource_id)?,
                    production.locators(resource_id)?,
                )
            };
            // Locators confirmed earlier in this transaction name directories too.
            locators.extend(
                transaction
                    .mutations
                    .iter()
                    .filter_map(|mutation| match mutation {
                        StagedMutation::Locator(locator)
                            if locator.resource_id() == resource_id =>
                        {
                            Some(locator.clone())
                        }
                        _ => None,
                    }),
            );
            let naming = naming_at(naming, &locators, path)?;
            // Earlier observations staged in this transaction are current for
            // the representations recomputed now.
            for (_, resources) in &mut usage {
                for resource in resources.iter_mut() {
                    for fingerprint in staged_resource_fingerprints(transaction, resource.id()) {
                        *resource = resource.with_observed_fingerprint(fingerprint);
                    }
                }
            }
            let observation =
                observe_resource_content(resource_id, &usage, Path::new(path), naming.as_ref())?;
            transaction
                .mutations
                .push(StagedMutation::RecordResourceFingerprint(
                    resource_id,
                    observation.resource().clone(),
                ));
            if let Some(facts) = observation.file_facts() {
                transaction
                    .mutations
                    .push(StagedMutation::RecordResourceFileFacts(resource_id, facts));
            }
            for (representation_id, fingerprint) in observation.representations() {
                transaction
                    .mutations
                    .push(StagedMutation::RecordRepresentationFingerprint(
                        *representation_id,
                        fingerprint.clone(),
                    ));
            }
            out_outcome.write(match observation.outcome() {
                ContentObservationOutcome::Unchanged => PP_OBSERVATION_UNCHANGED,
                ContentObservationOutcome::Changed => PP_OBSERVATION_CHANGED,
                _ => PP_OBSERVATION_FIRST,
            });
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
