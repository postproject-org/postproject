//! Owned typed metadata values supplied by C callers.

use std::ffi::c_char;

use postproject_core::{
    DecimalValue, Error, MAX_METADATA_COLLECTION_ITEMS, MetadataField, MetadataValue, PropertyId,
    RationalValue, Timestamp,
};

use crate::{
    PpError, PpObjectRef, ffi_call, initialize_output, invalid_argument, object_ref_from_abi,
    optional_utf8, require_output, required_utf8,
};

/// Opaque owned metadata input handle.
pub struct PpMetadataInput {
    pub(crate) value: MetadataValue,
}

unsafe fn create_input(
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
    operation: impl FnOnce() -> Result<MetadataValue, Error>,
) -> u32 {
    // SAFETY: The exported caller guarantees writable output pointers.
    unsafe {
        initialize_output(out_input);
        ffi_call(out_error, || {
            require_output(out_input, "out_input")?;
            let value = operation()?;
            out_input.write(Box::into_raw(Box::new(PpMetadataInput { value })));
            Ok(())
        })
    }
}

/// Creates an owned plain or language-tagged string input.
///
/// # Safety
///
/// `text` must be readable NUL-terminated UTF-8, `language` may be null or
/// readable NUL-terminated UTF-8, `out_input` must be writable, and `out_error`
/// may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_string(
    text: *const c_char,
    language: *const c_char,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointer contracts are forwarded to the checked conversion helpers.
    unsafe {
        create_input(out_input, out_error, || {
            let text = required_utf8(text, "text")?;
            optional_utf8(language, "language")?.map_or_else(
                || MetadataValue::string(text),
                |language| MetadataValue::language_string(text, language),
            )
        })
    }
}

/// Creates an owned signed-integer input.
///
/// # Safety
///
/// `out_input` must be writable and `out_error` may be null or writable.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_i64(
    value: i64,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller upholds the output-pointer contract.
    unsafe { create_input(out_input, out_error, || Ok(MetadataValue::i64(value))) }
}

/// Creates an owned unsigned-integer input.
///
/// # Safety
///
/// Pointer rules match [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_u64(
    value: u64,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller upholds the output-pointer contract.
    unsafe { create_input(out_input, out_error, || Ok(MetadataValue::u64(value))) }
}

/// Creates an owned exact decimal input.
///
/// # Safety
///
/// `coefficient` must be readable NUL-terminated UTF-8. Output pointers follow
/// [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_decimal(
    coefficient: *const c_char,
    scale: u32,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointer contracts are forwarded to the checked conversion helpers.
    unsafe {
        create_input(out_input, out_error, || {
            let coefficient = required_utf8(coefficient, "coefficient")?
                .parse::<i128>()
                .map_err(|_| invalid_argument("coefficient must be a signed base-10 integer"))?;
            Ok(MetadataValue::decimal(DecimalValue::new(
                coefficient,
                scale,
            )?))
        })
    }
}

/// Creates an owned Boolean input from exactly zero or one.
///
/// # Safety
///
/// Pointer rules match [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_bool(
    value: u8,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller upholds the output-pointer contract.
    unsafe {
        create_input(out_input, out_error, || match value {
            0 => Ok(MetadataValue::boolean(false)),
            1 => Ok(MetadataValue::boolean(true)),
            _ => Err(invalid_argument("Boolean metadata must be zero or one")),
        })
    }
}

/// Creates an owned UTC timestamp input from Unix microseconds.
///
/// # Safety
///
/// Pointer rules match [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_timestamp(
    unix_micros: i64,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller upholds the output-pointer contract.
    unsafe {
        create_input(out_input, out_error, || {
            Ok(MetadataValue::timestamp(Timestamp::from_unix_micros(
                unix_micros,
            )))
        })
    }
}

/// Creates an owned absolute-URI input.
///
/// # Safety
///
/// `uri` must be readable NUL-terminated UTF-8. Output pointers follow
/// [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_uri(
    uri: *const c_char,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointer contracts are forwarded to the checked conversion helpers.
    unsafe {
        create_input(out_input, out_error, || {
            MetadataValue::uri(required_utf8(uri, "uri")?)
        })
    }
}

/// Creates an owned byte-string input.
///
/// # Safety
///
/// `bytes` must address `length` readable bytes when `length` is nonzero.
/// Output pointers follow [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_bytes(
    bytes: *const u8,
    length: u64,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Pointer contracts are checked before constructing the borrowed slice.
    unsafe {
        create_input(out_input, out_error, || {
            let length = usize::try_from(length)
                .map_err(|_| invalid_argument("byte length is too large"))?;
            let value = if length == 0 {
                Vec::new()
            } else {
                if bytes.is_null() {
                    return Err(invalid_argument(
                        "bytes must not be null when length is nonzero",
                    ));
                }
                std::slice::from_raw_parts(bytes, length).to_vec()
            };
            MetadataValue::bytes(value)
        })
    }
}

/// Creates an owned exact rational input.
///
/// # Safety
///
/// Pointer rules match [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_rational(
    numerator: i64,
    denominator: u64,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The caller upholds the output-pointer contract.
    unsafe {
        create_input(out_input, out_error, || {
            Ok(MetadataValue::rational(RationalValue::new(
                numerator,
                denominator,
            )?))
        })
    }
}

/// Creates an owned object-reference input.
///
/// # Safety
///
/// `target` must be readable. Output pointers follow
/// [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_reference(
    target: *const PpObjectRef,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The pointer is checked before it is read.
    unsafe {
        create_input(out_input, out_error, || {
            let target = target
                .as_ref()
                .ok_or_else(|| invalid_argument("target must not be null"))?;
            Ok(MetadataValue::reference(object_ref_from_abi(*target)?))
        })
    }
}

/// Creates an owned ordered-list input by copying borrowed child inputs.
///
/// # Safety
///
/// `items` must address `count` readable live input pointers when `count` is
/// nonzero. Output pointers follow [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_list(
    items: *const *const PpMetadataInput,
    count: u64,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: The pointer array is validated before its elements are borrowed.
    unsafe {
        create_input(out_input, out_error, || {
            MetadataValue::list(input_values(items, count)?)
        })
    }
}

/// Creates an owned structured input by copying names and child inputs.
///
/// # Safety
///
/// `names` and `values` must each address `count` readable pointers when
/// `count` is nonzero. Names are NUL-terminated UTF-8 and values are live input
/// handles. Output pointers follow [`pp_metadata_input_create_i64`].
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_create_struct(
    names: *const *const c_char,
    values: *const *const PpMetadataInput,
    count: u64,
    out_input: *mut *mut PpMetadataInput,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Both arrays are validated before their elements are borrowed.
    unsafe {
        create_input(out_input, out_error, || {
            let count = input_count(count)?;
            if count == 0 {
                return MetadataValue::structure(Vec::new());
            }
            if names.is_null() || values.is_null() {
                return Err(invalid_argument(
                    "names and values must not be null when count is nonzero",
                ));
            }
            let names = std::slice::from_raw_parts(names, count);
            let values = std::slice::from_raw_parts(values, count);
            let fields = names
                .iter()
                .zip(values)
                .map(|(name, value)| {
                    let name = PropertyId::new(required_utf8(*name, "field name")?)?;
                    let value = value
                        .as_ref()
                        .ok_or_else(|| invalid_argument("field value must not be null"))?;
                    Ok(MetadataField::new(name, value.value.clone()))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            MetadataValue::structure(fields)
        })
    }
}

unsafe fn input_values(
    inputs: *const *const PpMetadataInput,
    count: u64,
) -> Result<Vec<MetadataValue>, Error> {
    let count = input_count(count)?;
    if count == 0 {
        return Ok(Vec::new());
    }
    if inputs.is_null() {
        return Err(invalid_argument(
            "items must not be null when count is nonzero",
        ));
    }
    // SAFETY: The caller guarantees `count` readable contiguous pointers.
    unsafe { std::slice::from_raw_parts(inputs, count) }
        .iter()
        .map(|input| {
            // SAFETY: Each array element must be a live borrowed input handle.
            unsafe { input.as_ref() }
                .map(|input| input.value.clone())
                .ok_or_else(|| invalid_argument("list item must not be null"))
        })
        .collect()
}

fn input_count(count: u64) -> Result<usize, Error> {
    let count = usize::try_from(count).map_err(|_| invalid_argument("input count is too large"))?;
    if count > MAX_METADATA_COLLECTION_ITEMS {
        return Err(invalid_argument(format!(
            "input count must not exceed {MAX_METADATA_COLLECTION_ITEMS}"
        )));
    }
    Ok(count)
}

/// Releases an owned metadata input. Null is a no-op.
///
/// # Safety
///
/// A non-null pointer must have been returned by a metadata-input constructor
/// and not previously released.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_metadata_input_release(input: *mut PpMetadataInput) {
    if input.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: Ownership of this allocation is transferred exactly once.
        drop(unsafe { Box::from_raw(input) });
    }));
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, ptr};

    use super::*;
    use crate::{PP_ERROR_INVALID_ARGUMENT, PP_OK, pp_error_release};

    #[test]
    fn decimal_constructor_owns_an_exact_domain_value() {
        let coefficient = CString::new("-12345").expect("valid C string");
        let mut input = ptr::null_mut();
        let mut error = ptr::null_mut();

        // SAFETY: Inputs remain live and outputs are writable for the call.
        let status = unsafe {
            pp_metadata_input_create_decimal(
                coefficient.as_ptr(),
                3,
                &raw mut input,
                &raw mut error,
            )
        };

        assert_eq!(status, PP_OK);
        assert!(error.is_null());
        // SAFETY: The successful call returned a live input handle.
        let decimal = unsafe { input.as_ref() }
            .expect("input")
            .value
            .as_decimal()
            .expect("decimal value");
        assert_eq!(decimal.coefficient(), -12_345);
        assert_eq!(decimal.scale(), 3);
        // SAFETY: The input is released exactly once.
        unsafe { pp_metadata_input_release(input) };
    }

    #[test]
    fn bool_constructor_rejects_non_boolean_bytes() {
        let mut input = ptr::null_mut();
        let mut error = ptr::null_mut();

        // SAFETY: Outputs are writable for the call.
        let status = unsafe { pp_metadata_input_create_bool(2, &raw mut input, &raw mut error) };

        assert_eq!(status, PP_ERROR_INVALID_ARGUMENT);
        assert!(input.is_null());
        assert!(!error.is_null());
        // SAFETY: The failed call returned one owned error.
        unsafe { pp_error_release(error) };
    }

    #[test]
    fn collection_constructors_deep_copy_children() {
        let mut child = ptr::null_mut();
        let mut list = ptr::null_mut();
        let mut structure = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: Outputs are writable and remain live for each call.
        assert_eq!(
            unsafe { pp_metadata_input_create_i64(42, &raw mut child, &raw mut error) },
            PP_OK
        );
        let children = [child.cast_const()];
        // SAFETY: The pointer array and output remain live for the call.
        assert_eq!(
            unsafe {
                pp_metadata_input_create_list(children.as_ptr(), 1, &raw mut list, &raw mut error)
            },
            PP_OK
        );
        let field_name = CString::new("numbers").expect("valid C string");
        let names = [field_name.as_ptr()];
        let values = [list.cast_const()];
        // SAFETY: Both pointer arrays and the output remain live for the call.
        assert_eq!(
            unsafe {
                pp_metadata_input_create_struct(
                    names.as_ptr(),
                    values.as_ptr(),
                    1,
                    &raw mut structure,
                    &raw mut error,
                )
            },
            PP_OK
        );
        // SAFETY: Children can be released because collection constructors copy.
        unsafe {
            pp_metadata_input_release(child);
            pp_metadata_input_release(list);
        }
        // SAFETY: The successful call returned a live structure handle.
        let fields = unsafe { structure.as_ref() }
            .expect("structure")
            .value
            .as_structure()
            .expect("structured value");
        assert_eq!(fields[0].name().as_str(), "numbers");
        assert_eq!(
            fields[0].value().as_list().expect("list")[0].as_i64(),
            Some(42)
        );
        // SAFETY: The remaining handle is released exactly once.
        unsafe { pp_metadata_input_release(structure) };
    }
}
