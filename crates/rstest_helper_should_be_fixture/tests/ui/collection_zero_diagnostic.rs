//! Trybuild collection input for the `rstest_helper_should_be_fixture` UI suite.
//!
//! The integration harness compiles this source to retain diagnostic-silent
//! coverage independently of the Cargo-backed collection summary assertions.

use rstest::{fixture, rstest};

const SUFFIX: &str = "suffix";
// Keep a `static` path here so the collector exercises both constant forms.
static PREFIX: &str = "prefix";

fn main() {
    let value = helper(fixture(), "literal", PREFIX, SUFFIX);
    assert_eq!(value, "prefix-fixture-literal-suffix");
}

#[fixture]
fn fixture() -> &'static str { "fixture" }

fn helper(fixture: &str, literal: &str, prefix: &str, suffix: &str) -> String {
    format!("{prefix}-{fixture}-{literal}-{suffix}")
}

#[rstest]
fn rstest_helper_call_collection_stays_silent(fixture: &str) {
    let value = helper(fixture, "literal", PREFIX, SUFFIX);
    assert_eq!(value, "prefix-fixture-literal-suffix");

    let deferred = || helper(fixture, "literal", PREFIX, SUFFIX);
    assert_eq!(deferred(), "prefix-fixture-literal-suffix");
}

#[rstest]
#[case("first")]
#[case("second")]
fn case_generated_collection_stays_silent(#[case] input: &str, fixture: &str) {
    let value = helper(fixture, input, PREFIX, SUFFIX);
    assert!(value.contains(input));
}
