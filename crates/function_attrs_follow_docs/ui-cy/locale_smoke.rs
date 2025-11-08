//! Welsh locale smoke test exercising the `function_attrs_follow_docs` lint
//! diagnostics when rendered under `DYLINT_LOCALE=cy`.
#![warn(function_attrs_follow_docs)]
#![allow(dead_code)]

#[inline]
/// Function doc comment appears after `#[inline]`.
fn function_doc_after_attribute() {}

fn main() {}
