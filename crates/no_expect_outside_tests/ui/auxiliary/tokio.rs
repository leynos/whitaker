// force-host
// no-prefer-dynamic
#![crate_type = "proc-macro"]

//! Tokio UI aux crate: emits Tokio's generated prelude-qualified test marker.

extern crate proc_macro;
use proc_macro::TokenStream;

/// Emits Tokio's generated prelude-qualified test attribute.
#[proc_macro_attribute]
pub fn test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut output: TokenStream = "#[::core::prelude::v1::test]".parse().unwrap();
    output.extend(item);
    output
}
