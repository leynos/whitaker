// force-host
// no-prefer-dynamic
#![crate_type = "proc-macro"]

//! rstest UI aux crate: emits `#[test]` to mark functions as test contexts.

extern crate proc_macro;
use proc_macro::TokenStream;

/// Emits `#[test]` before the item to mark it as a test context.
#[proc_macro_attribute]
pub fn rstest(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut output: TokenStream = "#[test]".parse().unwrap();
    output.extend(item);
    output
}
