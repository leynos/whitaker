// force-host
// no-prefer-dynamic
//! Derive macro for the generated-implementation wrapper-depth fixture.
#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_derive(GenerateLiteral)]
pub fn generate_literal(_input: TokenStream) -> TokenStream {
    concat!(
        "impl Generated { pub const VALUE: &'static str = \"alpha \\\n",
        "    beta\"; }"
    )
    .parse()
    .expect("the derive expansion is valid Rust")
}
