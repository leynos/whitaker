//! A list-form `doc` attribute is not documentation.
//!
//! `#[doc(hidden)]` is written with the same path as a doc comment but carries
//! no prose, so the classification must reject it. This fixture is the seeded
//! fault's witness: a mapper that treats every `doc` attribute as
//! documentation marks this function as documented and reports nothing.

#![expect(dead_code, reason = "UI test fixture; functions are analysed but never invoked")]

#[doc(hidden)]
fn hidden_but_undocumented() {}

// The list form is not documentation wherever it is written: inside a
// `cfg_attr` payload it still carries no prose.
#[cfg_attr(test, doc(hidden))]
fn conditional_hidden_but_undocumented() {}

// The bare form carries no arguments and no prose either.
#[doc]
fn bare_doc_attribute() {}

// A conditional attribute whose final path segment is `doc` is not
// documentation. Accepting a bare `doc` identifier would silence this
// function, which is why the whole path has to be checked.
#[cfg_attr(test, clippy::doc = "tool metadata")]
fn conditional_non_doc_path() {}

// A qualified `cfg_attr` is inert tool metadata even at the top level. The
// outer path check already rejects it, so this case guards that check.
#[clippy::cfg_attr(test, doc = "tool metadata")]
fn qualified_cfg_attr_path() {}

// The nested case is the one that reaches the payload scanner: the outer
// `cfg_attr` is unqualified, and Rust's expansion makes the inner qualified
// attribute inert metadata. A scanner that matched only the final `cfg_attr`
// segment would recurse into it and read its payload as documentation,
// silencing this function.
#[cfg_attr(test, clippy::cfg_attr(test, doc = "tool metadata"))]
fn nested_qualified_cfg_attr_path() {}

/// Entry point, documented so it is not reported.
fn main() {}