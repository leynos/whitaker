#![crate_type = "proc-macro"]

//! Tokio UI aux crate: exposes a pass-through `#[tokio::test]` for fixtures.

use proc_macro::TokenStream;

/// Pass-through attribute used in UI tests to emulate `tokio::test`.
#[proc_macro_attribute]
pub fn test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
