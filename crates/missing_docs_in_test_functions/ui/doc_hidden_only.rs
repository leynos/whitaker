//! A list-form `doc` attribute is not documentation.
//!
//! `#[doc(hidden)]` is written with the same path as a doc comment but carries
//! no prose, so the classification must reject it. This fixture is the seeded
//! fault's witness: a mapper that treats every `doc` attribute as
//! documentation marks this function as documented and reports nothing.

#![expect(dead_code, reason = "UI test fixture; functions are analysed but never invoked")]

#[doc(hidden)]
fn hidden_but_undocumented() {}

// A conditional attribute whose final path segment is `doc` is not
// documentation. Accepting a bare `doc` identifier would silence this
// function, which is why the whole path has to be checked.
#[cfg_attr(test, clippy::doc = "tool metadata")]
fn conditional_non_doc_path() {}

// A qualified `cfg_attr` is inert tool metadata too, so its payload is not
// documentation and must not silence this function. Matching only the final
// `cfg_attr` segment would accept it.
#[clippy::cfg_attr(test, doc = "tool metadata")]
fn qualified_cfg_attr_path() {}

/// Entry point, documented so it is not reported.
fn main() {}