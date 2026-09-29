//! Internal export annotation for the `PostProject` C ABI.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input, parse_quote};

/// Exports a C ABI function and records opt-in usage at its implementation boundary.
#[proc_macro_attribute]
pub fn ffi_export(_arguments: TokenStream, item: TokenStream) -> TokenStream {
    let mut function = parse_macro_input!(item as ItemFn);
    let operation = function.sig.ident.to_string();

    function.attrs.push(parse_quote!(#[unsafe(no_mangle)]));
    function
        .block
        .stmts
        .insert(0, parse_quote!(crate::abi_trace::record(#operation);));

    quote!(#function).into()
}
