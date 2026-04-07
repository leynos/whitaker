//! Regression example covering real `#[rstest]` handling for
//! `no_expect_outside_tests`.
//!
//! This uses the actual `rstest` crate (not a stub auxiliary proc-macro) to
//! validate that Whitaker correctly classifies functions annotated with
//! `#[rstest]` and `#[case]` as test-only code.
//!
//! `rstest_parametrize` was removed from the `rstest` crate in version 0.5.0
//! and replaced by the unified `#[rstest]` attribute with `#[case(...)]`.
//! Whitaker still recognises `rstest_parametrize` in the attribute registry for
//! backwards compatibility with older projects, but this example covers only the
//! current `#[rstest]` form since rstest 0.26.1 is the declared version.
//!
//! Regression test for <https://github.com/leynos/whitaker/issues/189>.

use rstest::rstest;

#[rstest]
#[case(1)]
#[case(42)]
fn rstest_allows_expect_in_test_context(#[case] value: i32) {
    let parsed = Some(value).expect("value should be present");
    assert_eq!(parsed, value);
}

#[rstest]
fn rstest_allows_expect_without_cases() {
    let result: Result<&str, ()> = Ok("ok");
    result.expect("rstest without cases should be test-only");
}

fn main() {}
