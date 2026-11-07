// aux-build: rstest.rs
//! UI fixture: doc comment after an attribute-macro attribute.
//!
//! Until `nightly-2026-10-30` rustc kept the invoking `#[fixture]` attribute
//! in HIR, so the lint reported the doc comment as misplaced. From that
//! nightly the attribute is consumed by the macro and no longer visible to a
//! late lint, so the expected output is empty. The file keeps its `fail_`
//! name so the history of the diagnostic stays findable.
#![warn(function_attrs_follow_docs)]

extern crate rstest;
use rstest::fixture;

#[fixture]
/// Factory fixture used by tests.
fn message_factory() {}

fn main() {
    message_factory();
}
