#![warn(function_attrs_follow_docs)]
#![allow(dead_code)]
// Welsh locale smoke test for `function_attrs_follow_docs` under `DYLINT_LOCALE=cy`.
#[inline]
/// Function doc comment appears after `#[inline]`.
fn function_doc_after_attribute() {}

fn main() {}
