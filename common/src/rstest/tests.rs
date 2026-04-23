//! Unit tests for strict `rstest` detection helpers.

use super::{
    ExpansionTrace, ParameterBinding, RstestDetectionOptions, RstestParameter, RstestParameterKind,
    SpanRecoveryFrame, UserEditableSpan, classify_rstest_parameter, fixture_local_names,
    is_rstest_fixture, is_rstest_fixture_with, is_rstest_test, is_rstest_test_with,
    recover_user_editable_span,
};
use crate::attributes::{Attribute, AttributeKind, AttributePath};
use crate::span::{SourceLocation, SourceSpan};
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
#[case::rstest("rstest", true)]
#[case::qualified("rstest::rstest", true)]
#[case::plain_test("test", false)]
#[case::tokio("tokio::test", false)]
#[case::case("case", false)]
#[case::fixture("rstest::fixture", false)]
fn detects_strict_rstest_tests_with_multiple_attributes(
    #[case] path: &str,
    #[case] expected: bool,
) {
    // non-rstest attribute preceding the rstest-related attribute
    assert_eq!(is_rstest_test(&[outer("allow"), outer(path)]), expected);
    // non-rstest attribute following the rstest-related attribute
    assert_eq!(is_rstest_test(&[outer(path), outer("allow")]), expected);
    // multiple non-rstest attributes surrounding the rstest-related attribute
    assert_eq!(
        is_rstest_test(&[outer("allow"), outer(path), outer("warn")]),
        expected
    );
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
fn classifies_custom_provider_parameters() {
    let parameter = provider_parameter("custom::provider");
    let options = RstestDetectionOptions::new(
        vec![
            AttributePath::from("custom::provider"),
            AttributePath::from("another::custom"),
        ],
        false,
    );

    assert_eq!(
        classify_rstest_parameter(&parameter, &options),
        RstestParameterKind::Provider
    );
}

#[rstest]
fn rejects_unknown_custom_provider_parameters() {
    let parameter = provider_parameter("unknown::provider");
    let options = RstestDetectionOptions::new(vec![AttributePath::from("custom::provider")], false);

    // Should classify as fixture local since it's not in the custom provider list
    assert_eq!(
        classify_rstest_parameter(&parameter, &options),
        RstestParameterKind::FixtureLocal {
            name: "value".to_string()
        }
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

type TraceFallbackDetect =
    fn(&[Attribute], Option<&ExpansionTrace>, &RstestDetectionOptions) -> bool;

#[rstest]
#[case::single_frame_test(
    is_rstest_test_with as TraceFallbackDetect,
    &["rstest"] as &[&str]
)]
#[case::multi_frame_test(
    is_rstest_test_with as TraceFallbackDetect,
    &["outer_macro", "rstest"]
)]
#[case::deeply_nested_test(
    is_rstest_test_with as TraceFallbackDetect,
    &["macro_a", "macro_b", "macro_c", "rstest::rstest"]
)]
#[case::single_frame_fixture(
    is_rstest_fixture_with as TraceFallbackDetect,
    &["fixture"]
)]
#[case::multi_frame_fixture(
    is_rstest_fixture_with as TraceFallbackDetect,
    &["outer_macro", "fixture"]
)]
fn honours_trace_when_fallback_is_enabled(
    #[case] detect: TraceFallbackDetect,
    #[case] frame_paths: &[&str],
) {
    let trace = ExpansionTrace::new(frame_paths.iter().copied().map(AttributePath::from));
    let options = RstestDetectionOptions::new(Vec::new(), true);

    assert!(detect(&[outer("allow")], Some(&trace), &options));
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

fn source_span(line: usize, start: usize, end: usize) -> SourceSpan {
    SourceSpan::new(
        SourceLocation::new(line, start),
        SourceLocation::new(line, end),
    )
    .expect("test spans should always be valid")
}

fn assert_span_recovery(
    frame_specs: impl IntoIterator<Item = (SourceSpan, bool)>,
    expected: UserEditableSpan<SourceSpan>,
) {
    let frames: Vec<SpanRecoveryFrame<SourceSpan>> = frame_specs
        .into_iter()
        .map(|(span, is_macro)| SpanRecoveryFrame::new(span, is_macro))
        .collect();
    assert_eq!(recover_user_editable_span(&frames), expected);
}

#[rstest]
#[case::keeps_direct_user_editable_span(
    vec![(source_span(1, 1, 8), false)],
    UserEditableSpan::Direct(source_span(1, 1, 8)),
)]
#[case::recovers_macro_frame_to_first_user_span(
    vec![(source_span(2, 1, 8), true), (source_span(10, 1, 12), false)],
    UserEditableSpan::Recovered(source_span(10, 1, 12)),
)]
#[case::recovers_first_user_span_from_nested_macro_chain(
    vec![
        (source_span(2, 1, 4), true),
        (source_span(3, 1, 5), true),
        (source_span(14, 1, 6), false),
        (source_span(20, 1, 9), false),
    ],
    UserEditableSpan::Recovered(source_span(14, 1, 6)),
)]
#[case::treats_empty_frame_list_as_macro_only(vec![], UserEditableSpan::MacroOnly)]
#[case::treats_all_expansion_frames_as_macro_only(
    vec![(source_span(4, 1, 4), true), (source_span(5, 1, 6), true)],
    UserEditableSpan::MacroOnly,
)]
fn recovers_user_editable_span_from_frame_sequences(
    #[case] frames: Vec<(SourceSpan, bool)>,
    #[case] expected: UserEditableSpan<SourceSpan>,
) {
    assert_span_recovery(frames, expected);
}
