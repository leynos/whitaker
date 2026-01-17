// force-host
// no-prefer-dynamic
//! Minimal `rstest`-style proc macro used by UI fixtures.
#![crate_type = "proc-macro"]

extern crate proc_macro;
use proc_macro::TokenStream;

/// Leaves the item unchanged, mirroring a fixture attribute macro.
#[proc_macro_attribute]
pub fn fixture(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
