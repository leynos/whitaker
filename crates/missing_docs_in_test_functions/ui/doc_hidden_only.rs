//! A list-form `doc` attribute is not documentation.
//!
//! `#[doc(hidden)]` is written with the same path as a doc comment but carries
//! no prose, so the classification must reject it. This fixture is the seeded
//! fault's witness: a mapper that treats every `doc` attribute as
//! documentation marks this function as documented and reports nothing.

#![allow(dead_code)]

#[doc(hidden)]
fn hidden_but_undocumented() {}

/// Entry point, documented so it is not reported.
fn main() {}