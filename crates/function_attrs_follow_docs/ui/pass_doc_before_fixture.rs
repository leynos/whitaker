// aux-build: rstest.rs
//! UI fixture: doc comments stay before `#[fixture]` attributes.
#![warn(function_attrs_follow_docs)]
#![allow(dead_code)]

extern crate rstest;
use rstest::fixture;

/// Factory fixture used by tests.
///
/// The doc comment stays before the attribute macro.
#[fixture]
fn message_factory() {}

fn main() {}
