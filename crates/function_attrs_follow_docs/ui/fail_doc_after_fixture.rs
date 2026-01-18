// aux-build: rstest.rs
//! Negative UI fixture: doc comment after fixture attribute.
#![warn(function_attrs_follow_docs)]

extern crate rstest;
use rstest::fixture;

#[fixture]
/// Factory fixture used by tests.
fn message_factory() {}

fn main() {
    message_factory();
}
