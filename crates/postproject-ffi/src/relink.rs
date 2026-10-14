//! Borrowed verified selections, copied atomically into a read-bound edit.

use std::{collections::BTreeSet, ffi::c_char, ptr};

use postproject_core::{DecisionBase, Error, ErrorKind, ResourceId};

use crate::{
    PpDecisionBase, PpError, PpRepresentationId, PpResolutionSet, PpResourceId, PpSequenceNaming,
    PpTransaction, StagedMutation, ffi_call, initialize_value, invalid_argument, optional_utf8,
    require_output, required_utf8,
};

/// One selected location; strings borrow the resolution set until its release.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PpRelinkLocation {
    /// Resource being located.
    pub resource_id: PpResourceId,
    /// Required absolute locator URI.
    pub uri: *const c_char,
    /// Optional logical root.
    pub media_root: *const c_char,
    /// Exactly zero or one; absent naming has null strings and zero padding.
    pub has_sequence_naming: u8,
    /// Naming when present.
    pub sequence_naming: PpSequenceNaming,
}

/// Copyable selection; locations and their strings remain caller-owned/borrowed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PpRelinkSelection {
    /// Original coherent storage decision.
    pub base: PpDecisionBase,
    /// Explicitly selected representation.
    pub representation_id: PpRepresentationId,
    /// Borrowed location array; never retained by confirmation.
    pub locations: *const PpRelinkLocation,
    /// Positive count, at most 100000.
    pub location_count: u64,
}

/// Selects complete uniquely verified content into caller-provided location storage.
///
/// Capacity must cover the selected locations; the resource count is a safe
/// upper bound. Failure clears the selection and writes no location slots.
/// No additional filesystem work is performed. Optional unverified members
/// remain in diagnostics and are omitted. Selection does not commit anything.
///
/// # Safety
/// Set must be live, location storage writable for capacity, selection writable,
/// error nullable/writable. Returned strings borrow the set until release.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_resolution_set_select_verified(
    resolutions: *const PpResolutionSet,
    representation_index: u64,
    locations: *mut PpRelinkLocation,
    capacity: u64,
    out_selection: *mut PpRelinkSelection,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Zero is valid for every field; pointers and bounds checked before writes.
    unsafe {
        initialize_value(out_selection, std::mem::zeroed());
        ffi_call(out_error, || {
            require_output(out_selection, "out_selection")?;
            let set = resolutions
                .as_ref()
                .ok_or_else(|| invalid_argument("resolutions must not be null"))?;
            let index = usize::try_from(representation_index)
                .map_err(|_| invalid_argument("representation index is too large"))?;
            let result = set
                .representations
                .get(index)
                .ok_or_else(|| invalid_argument("representation index is out of range"))?;
            let indices = result.verified_resources.as_ref().map_err(Clone::clone)?;
            let base = set
                .base
                .ok_or_else(|| invalid_argument("resolution has no coherent decision"))?;
            if capacity > 100_000 || capacity < indices.len() as u64 || locations.is_null() {
                return Err(invalid_argument(
                    "location storage must cover the verified selection within 100000 items",
                ));
            }
            for (output_index, resource_index) in indices.iter().enumerate() {
                let resource = &result.resources[*resource_index];
                let candidate = &resource.candidates[0];
                locations.add(output_index).write(PpRelinkLocation {
                    resource_id: PpResourceId {
                        bytes: resource.resource_id.into_bytes(),
                    },
                    uri: candidate.uri.as_ptr(),
                    media_root: candidate
                        .media_root
                        .as_ref()
                        .map_or(ptr::null(), |root| root.as_ptr()),
                    has_sequence_naming: u8::from(candidate.sequence_naming.is_some()),
                    sequence_naming: candidate
                        .sequence_naming
                        .as_ref()
                        .map_or(std::mem::zeroed(), crate::AbiSequenceNaming::borrowed),
                });
            }
            out_selection.write(PpRelinkSelection {
                base: base.into(),
                representation_id: PpRepresentationId {
                    bytes: result.representation_id.into_bytes(),
                },
                locations,
                location_count: indices.len() as u64,
            });
            Ok(())
        })
    }
}

/// Copies all selected locations into an edit carrying exactly the original base.
///
/// Failure leaves staged work unchanged. Commit remains explicit. The borrowed
/// selection can be released immediately afterwards. This does not recheck files.
///
/// # Safety
/// Transaction live, selection and its bounded array readable with valid strings,
/// error nullable/writable. Inputs are borrowed only during this call.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_transaction_confirm_selection(
    transaction: *mut PpTransaction,
    selection: *const PpRelinkSelection,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Nulls, tags and bounded counts validated before borrowing inputs.
    unsafe {
        ffi_call(out_error, || {
            let transaction = transaction
                .as_mut()
                .ok_or_else(|| invalid_argument("transaction must not be null"))?;
            transaction.lifecycle.ensure_open()?;
            let selection = selection
                .as_ref()
                .ok_or_else(|| invalid_argument("selection must not be null"))?;
            let base = DecisionBase::try_from(selection.base)?;
            if transaction.decision_base != Some(base) {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "edit must carry the selection's original decision base",
                ));
            }
            if selection.location_count == 0
                || selection.location_count > 100_000
                || selection.locations.is_null()
            {
                return Err(invalid_argument("selection requires 1..100000 locations"));
            }
            let count = usize::try_from(selection.location_count)
                .map_err(|_| invalid_argument("location count is not addressable"))?;
            let inputs = std::slice::from_raw_parts(selection.locations, count);
            let mut prepared = Vec::with_capacity(count);
            let mut resources = BTreeSet::new();
            for input in inputs {
                let resource = ResourceId::from_bytes(input.resource_id.bytes);
                if !resources.insert(resource) {
                    return Err(invalid_argument("selection repeats a resource"));
                }
                let naming = match input.has_sequence_naming {
                    1 => crate::sequence_naming::optional_naming(
                        &raw const input.sequence_naming,
                        "sequence_naming",
                    )?,
                    0 if input.sequence_naming.prefix.is_null()
                        && input.sequence_naming.suffix.is_null()
                        && input.sequence_naming.padding == 0 =>
                    {
                        None
                    }
                    _ => return Err(invalid_argument("invalid selection naming tag or payload")),
                };
                prepared.push(postproject_media::prepare_confirmed_locator(
                    resource,
                    required_utf8(input.uri, "locator URI")?,
                    optional_utf8(input.media_root, "root name")?,
                    naming,
                )?);
            }
            let inner = crate::lock_production(&transaction.state);
            let representation = inner.representation(
                postproject_core::RepresentationId::from_bytes(selection.representation_id.bytes),
            )?;
            let structure = representation.content_structure();
            let members: BTreeSet<_> = structure.resource_ids().into_iter().collect();
            if !resources.is_subset(&members)
                || structure.members().map_or_else(
                    || resources != members,
                    |members| {
                        members.iter().any(|member| {
                            member.is_required() && !resources.contains(&member.resource_id())
                        })
                    },
                )
            {
                return Err(invalid_argument(
                    "selection must cover every required member of its representation",
                ));
            }
            drop(inner);
            transaction
                .mutations
                .extend(prepared.into_iter().map(StagedMutation::ConfirmedLocator));
            Ok(())
        })
    }
}

pub(crate) fn verified_indices(
    resolution: &postproject_core::RepresentationResolution,
    base: Option<DecisionBase>,
) -> Result<Vec<usize>, Error> {
    let selection = resolution.select_verified(
        base.ok_or_else(|| invalid_argument("resolution has no coherent decision"))?,
    )?;
    let chosen: BTreeSet<_> = selection.locations().iter().map(|(id, _)| *id).collect();
    Ok(resolution
        .resources()
        .iter()
        .enumerate()
        .filter_map(|(index, resource)| chosen.contains(&resource.resource_id()).then_some(index))
        .collect())
}
