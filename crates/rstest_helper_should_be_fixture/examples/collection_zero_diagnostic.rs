//! Zero-diagnostic fixture that exercises helper-call collection.
#![feature(rustc_private)]

use rstest::{fixture, rstest};

const SUFFIX: &str = "suffix";
// Keep a `static` path here so the collector exercises both constant forms.
static PREFIX: &str = "prefix";

fn main() {
    let value = helper(fixture(), "literal", PREFIX, SUFFIX);
    assert_eq!(value, "prefix-fixture-literal-suffix");

    let built = Builder { fixture: fixture() }.build(SUFFIX);
    assert_eq!(built, "fixture-suffix");

    assert_eq!(nested_helper(fixture()), "fixture");
}

#[fixture]
fn fixture() -> &'static str {
    const FIXTURE: &str = "fixture";
    FIXTURE
}

fn helper(fixture: &str, literal: &str, prefix: &str, suffix: &str) -> String {
    format!("{prefix}-{fixture}-{literal}-{suffix}")
}

struct Builder<'a> {
    fixture: &'a str,
}

impl Builder<'_> {
    fn build(&self, suffix: &str) -> String {
        format!("{}-{suffix}", self.fixture)
    }
}

#[rstest]
fn rstest_helper_call_collection_stays_silent(fixture: &str) {
    let value = helper(fixture, "literal", PREFIX, SUFFIX);
    assert_eq!(value, "prefix-fixture-literal-suffix");

    let deferred = || helper(fixture, "literal", PREFIX, SUFFIX);
    assert_eq!(deferred(), "prefix-fixture-literal-suffix");

    let built = Builder { fixture }.build(SUFFIX);
    assert_eq!(built, "fixture-suffix");
}

fn nested_helper(fixture: &str) -> &str {
    fixture
}

#[rstest]
#[case("first")]
#[case("second")]
fn case_generated_collection_stays_silent(#[case] input: &str, fixture: &str) {
    let value = helper(fixture, input, PREFIX, SUFFIX);
    assert!(value.contains(input));

    let outer = || {
        let inner = || nested_helper(fixture);
        inner()
    };
    assert_eq!(outer(), "fixture");

    let built = Builder { fixture }.build(input);
    assert!(built.ends_with(input));
}
