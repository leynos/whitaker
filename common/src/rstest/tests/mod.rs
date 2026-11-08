//! Unit tests for shared `rstest` helpers.

mod fingerprint;
mod fingerprint_props;

use std::collections::BTreeSet;

use rstest::rstest;

use super::{
    ExpansionTrace,
    ParameterBinding,
    RstestDetectionOptions,
    RstestParameter,
    RstestParameterKind,
    SpanRecoveryFrame,
    UserEditableSpan,
    classify_rstest_parameter,
    fixture_local_names,
    is_rstest_fixture,
    is_rstest_fixture_with,
    is_rstest_test,
    is_rstest_test_with,
    recover_user_editable_span,
};
use crate::{
    attributes::{Attribute, AttributeKind, AttributePath},
    span::{SourceLocation, SourceSpan},
};

/// Builds an outer attribute from its typed path.
fn outer(path: AttributePath) -> Attribute { Attribute::new(path, AttributeKind::Outer) }

/// Builds a parameter carrying the supplied provider attribute path.
fn provider_parameter(path: AttributePath) -> RstestParameter {
    RstestParameter::new(
        ParameterBinding::Ident("value".to_owned()),
        vec![outer(path)],
    )
}

/// Classifies known `rstest` test attributes without accepting unrelated ones.
#[rstest]
#[case::rstest(AttributePath::from("rstest"), true)]
#[case::qualified(AttributePath::from("rstest::rstest"), true)]
#[case::plain_test(AttributePath::from("test"), false)]
#[case::tokio(AttributePath::from("tokio::test"), false)]
#[case::case(AttributePath::from("case"), false)]
#[case::fixture(AttributePath::from("rstest::fixture"), false)]
fn detects_strict_rstest_tests(#[case] path: AttributePath, #[case] expected: bool) {
    assert_eq!(is_rstest_test(&[outer(path)]), expected);
}

/// Classifies `rstest` tests when unrelated attributes appear in any order.
#[rstest]
#[case::rstest(AttributePath::from("rstest"), true)]
#[case::qualified(AttributePath::from("rstest::rstest"), true)]
#[case::plain_test(AttributePath::from("test"), false)]
#[case::tokio(AttributePath::from("tokio::test"), false)]
#[case::case(AttributePath::from("case"), false)]
#[case::fixture(AttributePath::from("rstest::fixture"), false)]
fn detects_strict_rstest_tests_with_multiple_attributes(
    #[case] path: AttributePath,
    #[case] expected: bool,
) {
    // non-rstest attribute preceding the rstest-related attribute
    assert_eq!(
        is_rstest_test(&[outer(AttributePath::from("allow")), outer(path.clone())]),
        expected
    );
    // non-rstest attribute following the rstest-related attribute
    assert_eq!(
        is_rstest_test(&[outer(path.clone()), outer(AttributePath::from("allow"))]),
        expected
    );
    // multiple non-rstest attributes surrounding the rstest-related attribute
    assert_eq!(
        is_rstest_test(&[
            outer(AttributePath::from("allow")),
            outer(path),
            outer(AttributePath::from("warn")),
        ]),
        expected
    );
}

/// Classifies fixture attributes without treating tests or other paths as fixtures.
#[rstest]
#[case::fixture(AttributePath::from("fixture"), true)]
#[case::qualified(AttributePath::from("rstest::fixture"), true)]
#[case::rstest(AttributePath::from("rstest"), false)]
#[case::other(AttributePath::from("allow"), false)]
fn detects_strict_rstest_fixtures(#[case] path: AttributePath, #[case] expected: bool) {
    assert_eq!(is_rstest_fixture(&[outer(path)]), expected);
}

#[rstest]
fn classifies_identifier_parameters_as_fixture_locals() {
    let parameter = RstestParameter::ident("db");

    assert_eq!(
        classify_rstest_parameter(&parameter, &RstestDetectionOptions::default()),
        RstestParameterKind::FixtureLocal {
            name: "db".to_owned()
        }
    );
}

/// Recognizes the built-in provider attributes accepted by `rstest`.
#[rstest]
#[case(AttributePath::from("case"))]
#[case(AttributePath::from("rstest::case"))]
#[case(AttributePath::from("values"))]
#[case(AttributePath::from("rstest::values"))]
#[case(AttributePath::from("files"))]
#[case(AttributePath::from("rstest::files"))]
#[case(AttributePath::from("future"))]
#[case(AttributePath::from("rstest::future"))]
#[case(AttributePath::from("context"))]
#[case(AttributePath::from("rstest::context"))]
fn classifies_provider_parameters(#[case] path: AttributePath) {
    let parameter = provider_parameter(path);

    assert_eq!(
        classify_rstest_parameter(&parameter, &RstestDetectionOptions::default()),
        RstestParameterKind::Provider
    );
}

/// Recognizes explicitly configured provider paths.
#[rstest]
fn classifies_custom_provider_parameters() {
    let parameter = provider_parameter(AttributePath::from("custom::provider"));
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

/// Treats unknown custom provider paths as fixture-local parameters.
#[rstest]
fn rejects_unknown_custom_provider_parameters() {
    let parameter = provider_parameter(AttributePath::from("unknown::provider"));
    let options = RstestDetectionOptions::new(vec![AttributePath::from("custom::provider")], false);

    // Should classify as fixture local since it's not in the custom provider list
    assert_eq!(
        classify_rstest_parameter(&parameter, &options),
        RstestParameterKind::FixtureLocal {
            name: "value".to_owned()
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

/// Ignores expansion traces when fallback detection is disabled.
#[rstest]
fn ignores_trace_when_fallback_is_disabled() {
    let trace = ExpansionTrace::new([AttributePath::from("rstest")]);

    assert!(!is_rstest_test_with(
        &[outer(AttributePath::from("allow"))],
        Some(&trace),
        &RstestDetectionOptions::default(),
    ));
}

type TraceFallbackDetect =
    fn(&[Attribute], Option<&ExpansionTrace>, &RstestDetectionOptions) -> bool;

/// Uses expansion traces to recognize test-like attributes through macros.
#[rstest]
#[case::single_frame_test(
    is_rstest_test_with as TraceFallbackDetect,
    vec![AttributePath::from("rstest")]
)]
#[case::multi_frame_test(
    is_rstest_test_with as TraceFallbackDetect,
    vec![AttributePath::from("outer_macro"), AttributePath::from("rstest")]
)]
#[case::deeply_nested_test(
    is_rstest_test_with as TraceFallbackDetect,
    vec![
        AttributePath::from("macro_a"),
        AttributePath::from("macro_b"),
        AttributePath::from("macro_c"),
        AttributePath::from("rstest::rstest"),
    ]
)]
#[case::single_frame_fixture(
    is_rstest_fixture_with as TraceFallbackDetect,
    vec![AttributePath::from("fixture")]
)]
#[case::multi_frame_fixture(
    is_rstest_fixture_with as TraceFallbackDetect,
    vec![AttributePath::from("outer_macro"), AttributePath::from("fixture")]
)]
fn honours_trace_when_fallback_is_enabled(
    #[case] detect: TraceFallbackDetect,
    #[case] frame_paths: Vec<AttributePath>,
) {
    let trace = ExpansionTrace::new(frame_paths);
    let options = RstestDetectionOptions::new(Vec::new(), true);

    assert!(detect(
        &[outer(AttributePath::from("allow"))],
        Some(&trace),
        &options
    ));
}

/// Preserves the first occurrence of each supported fixture-local name.
#[rstest]
fn collects_supported_fixture_local_names_in_order() {
    let parameters = vec![
        RstestParameter::ident("db"),
        provider_parameter(AttributePath::from("case")),
        RstestParameter::unsupported(),
        RstestParameter::ident("clock"),
        RstestParameter::ident("db"),
    ];

    assert_eq!(
        fixture_local_names(&parameters, &RstestDetectionOptions::default()),
        BTreeSet::from(["clock".to_owned(), "db".to_owned()])
    );
}

/// Builds a single-line [`SourceSpan`] for test data.
///
/// This is a macro rather than a helper function so the fallible construction
/// is inlined into the calling `#[rstest]` body, where a failure is the test
/// verdict, and so it can be used inside `#[case(...)]` attributes.
macro_rules! source_span {
    ($line:expr, $start:expr, $end:expr) => {
        SourceSpan::new(
            SourceLocation::new($line, $start),
            SourceLocation::new($line, $end),
        )
        .expect("test spans should always be valid")
    };
}

fn assert_span_recovery(
    frame_specs: impl IntoIterator<Item = (SourceSpan, bool)>,
    expected: &UserEditableSpan<SourceSpan>,
) {
    let frames: Vec<SpanRecoveryFrame<SourceSpan>> = frame_specs
        .into_iter()
        .map(|(span, is_macro)| SpanRecoveryFrame::new(span, is_macro))
        .collect();
    assert_eq!(&recover_user_editable_span(&frames), expected);
}

#[rstest]
#[case::keeps_direct_user_editable_span(
    vec![(source_span!(1, 1, 8), false)],
    UserEditableSpan::Direct(source_span!(1, 1, 8)),
)]
#[case::recovers_macro_frame_to_first_user_span(
    vec![(source_span!(2, 1, 8), true), (source_span!(10, 1, 12), false)],
    UserEditableSpan::Recovered(source_span!(10, 1, 12)),
)]
#[case::recovers_first_user_span_from_nested_macro_chain(
    vec![
        (source_span!(2, 1, 4), true),
        (source_span!(3, 1, 5), true),
        (source_span!(14, 1, 6), false),
        (source_span!(20, 1, 9), false),
    ],
    UserEditableSpan::Recovered(source_span!(14, 1, 6)),
)]
#[case::treats_empty_frame_list_as_macro_only(vec![], UserEditableSpan::MacroOnly)]
#[case::treats_all_expansion_frames_as_macro_only(
    vec![(source_span!(4, 1, 4), true), (source_span!(5, 1, 6), true)],
    UserEditableSpan::MacroOnly,
)]
fn recovers_user_editable_span_from_frame_sequences(
    #[case] frames: Vec<(SourceSpan, bool)>,
    #[case] expected: UserEditableSpan<SourceSpan>,
) {
    assert_span_recovery(frames, &expected);
}
