//! Unit tests for strict `rstest` detection helpers.

use super::{
    ExpansionTrace, ParameterBinding, RstestDetectionOptions, RstestParameter, RstestParameterKind,
    classify_rstest_parameter, fixture_local_names, is_rstest_fixture, is_rstest_fixture_with,
    is_rstest_test, is_rstest_test_with,
};
use crate::attributes::{Attribute, AttributeKind, AttributePath};
use rstest::rstest;
use std::collections::BTreeSet;

fn outer(path: &str) -> Attribute {
    Attribute::new(AttributePath::from(path), AttributeKind::Outer)
}

fn provider_parameter(path: &str) -> RstestParameter {
    RstestParameter::new(
        ParameterBinding::Ident("value".to_string()),
        vec![outer(path)],
    )
}

#[rstest]
#[case::rstest("rstest", true)]
#[case::qualified("rstest::rstest", true)]
#[case::plain_test("test", false)]
#[case::tokio("tokio::test", false)]
#[case::case("case", false)]
#[case::fixture("rstest::fixture", false)]
fn detects_strict_rstest_tests(#[case] path: &str, #[case] expected: bool) {
    assert_eq!(is_rstest_test(&[outer(path)]), expected);
}

#[rstest]
#[case::fixture("fixture", true)]
#[case::qualified("rstest::fixture", true)]
#[case::rstest("rstest", false)]
#[case::other("allow", false)]
fn detects_strict_rstest_fixtures(#[case] path: &str, #[case] expected: bool) {
    assert_eq!(is_rstest_fixture(&[outer(path)]), expected);
}

#[rstest]
fn classifies_identifier_parameters_as_fixture_locals() {
    let parameter = RstestParameter::ident("db");

    assert_eq!(
        classify_rstest_parameter(&parameter, &RstestDetectionOptions::default()),
        RstestParameterKind::FixtureLocal {
            name: "db".to_string()
        }
    );
}

#[rstest]
#[case("case")]
#[case("rstest::case")]
#[case("values")]
#[case("rstest::values")]
#[case("files")]
#[case("rstest::files")]
#[case("future")]
#[case("rstest::future")]
#[case("context")]
#[case("rstest::context")]
fn classifies_provider_parameters(#[case] path: &str) {
    let parameter = provider_parameter(path);

    assert_eq!(
        classify_rstest_parameter(&parameter, &RstestDetectionOptions::default()),
        RstestParameterKind::Provider
    );
}

#[rstest]
fn rejects_unsupported_parameter_patterns() {
    let parameter = RstestParameter::unsupported();

    assert_eq!(
        classify_rstest_parameter(&parameter, &RstestDetectionOptions::default()),
        RstestParameterKind::UnsupportedPattern
    );
}

#[rstest]
fn ignores_trace_when_fallback_is_disabled() {
    let trace = ExpansionTrace::new([AttributePath::from("rstest")]);

    assert!(!is_rstest_test_with(
        &[outer("allow")],
        Some(&trace),
        &RstestDetectionOptions::default(),
    ));
}

#[rstest]
fn honours_test_trace_when_fallback_is_enabled() {
    let trace = ExpansionTrace::new([AttributePath::from("rstest")]);
    let options = RstestDetectionOptions::new(Vec::new(), true);

    assert!(is_rstest_test_with(
        &[outer("allow")],
        Some(&trace),
        &options
    ));
}

#[rstest]
fn honours_fixture_trace_when_fallback_is_enabled() {
    let trace = ExpansionTrace::new([AttributePath::from("fixture")]);
    let options = RstestDetectionOptions::new(Vec::new(), true);

    assert!(is_rstest_fixture_with(
        &[outer("allow")],
        Some(&trace),
        &options,
    ));
}

#[rstest]
fn collects_supported_fixture_local_names_in_order() {
    let parameters = vec![
        RstestParameter::ident("db"),
        provider_parameter("case"),
        RstestParameter::unsupported(),
        RstestParameter::ident("clock"),
        RstestParameter::ident("db"),
    ];

    assert_eq!(
        fixture_local_names(&parameters, &RstestDetectionOptions::default()),
        BTreeSet::from(["clock".to_string(), "db".to_string()])
    );
}
