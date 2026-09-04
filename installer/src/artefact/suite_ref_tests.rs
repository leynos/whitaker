//! Tests for the suite reference newtype.
//!
//! The value reaches a `git checkout` command line, so the rejections matter
//! more than the acceptances: each case here is a string that would either be
//! read as something other than a reference, or refused by git after a clone
//! had already been paid for.

use super::*;
use rstest::rstest;

#[rstest]
#[case::tag("v0.2.7")]
#[case::branch("main")]
#[case::short_sha("2bc0c3f")]
#[case::full_sha("b4d31017a3f9e2c1d0b8a7654321fedcba098765")]
#[case::namespaced_branch("feature/suite-pin")]
#[case::dotted_tag("v1.2.3-rc.1")]
fn accepts_a_usable_reference(#[case] value: &str) {
    let reference: SuiteRef = value.try_into().expect("reference should be accepted");

    assert_eq!(reference.as_str(), value);
}

#[rstest]
#[case::empty("")]
#[case::leading_dash("--upload-pack=evil")]
#[case::leading_slash("/main")]
#[case::trailing_slash("main/")]
#[case::lock_suffix("main.lock")]
#[case::double_dot("v1..v2")]
#[case::double_slash("refs//heads")]
#[case::reflog("main@{yesterday}")]
#[case::space("my branch")]
#[case::tilde("main~1")]
#[case::caret("main^")]
#[case::colon("origin:main")]
#[case::question("main?")]
#[case::asterisk("refs/*")]
#[case::bracket("main[0]")]
#[case::backslash("main\\x")]
fn rejects_a_reference_git_would_not_take(#[case] value: &str) {
    let result: Result<SuiteRef> = value.try_into();

    assert!(result.is_err(), "{value:?} should have been rejected");
}

#[rstest]
fn rejects_a_reference_longer_than_the_limit() {
    let value = "v".repeat(MAX_LEN + 1);

    let result: Result<SuiteRef> = value.as_str().try_into();

    assert!(result.is_err());
}

#[rstest]
fn the_error_names_the_value_and_the_reason() {
    let result: Result<SuiteRef> = "main~1".try_into();

    let error = result.expect_err("should be rejected");
    let rendered = error.to_string();

    assert!(rendered.contains("main~1"), "{rendered}");
    assert!(rendered.contains('~'), "{rendered}");
}

#[rstest]
fn a_leading_dash_is_refused_before_it_reaches_git() {
    // The case that matters: `git checkout --upload-pack=...` would run an
    // arbitrary command, so this is refused as a value rather than passed on
    // and separated with `--` somewhere downstream.
    let result: Result<SuiteRef> = "--upload-pack=touch /tmp/pwned".try_into();

    assert!(result.is_err());
}

#[rstest]
fn round_trips_through_its_inner_string() {
    let reference: SuiteRef = "v0.2.7".try_into().expect("valid reference");

    assert_eq!(reference.clone().into_inner(), "v0.2.7");
    assert_eq!(reference.to_string(), "v0.2.7");
}
