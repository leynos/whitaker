// no-prefer-dynamic
// compile-flags: --crate-type=proc-macro --emit=metadata,link
#![crate_type = "proc-macro"]

//! rstest UI aux crate: pass-through `#[rstest]` for fixtures.

extern crate proc_macro;

use proc_macro::TokenStream;

/// Pass-through attribute used in UI tests to emulate `rstest`.
#[proc_macro_attribute]
pub fn rstest(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
