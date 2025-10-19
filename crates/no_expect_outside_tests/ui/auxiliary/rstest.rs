#![crate_type = "proc-macro"]

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn rstest(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
