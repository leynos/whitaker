//! Tests for the diagnostics emitted by the AST lowering adapter.
//!
//! Each case installs a formatting subscriber that writes to a temporary file,
//! invokes one reporting helper, and asserts the rendered message, level, and
//! fields. The decisions that choose a diagnostic are covered by
//! `lowering_tests`; these cases prove the report names the same facts, which
//! is what an operator reading the log actually depends on.
//!
//! Cases are described by small expectation structs rather than bags of string
//! arguments. That keeps each helper's parameter list short and typed, and it
//! states the expected diagnostic once instead of repeating its parts at every
//! call site.

use std::io::{Read, Seek, SeekFrom};

use rstest::rstest;
use tracing_subscriber::fmt;

use super::{
    log_covering_failure,
    log_lowering_depth_budget,
    log_lowering_failure,
    log_lowering_node_budget,
    log_parser_recovery,
    log_selection_depth_budget,
    log_selection_node_budget,
    log_span_validation_failure,
};
use crate::{AstError, ByteSpan};

/// A reporting wrapper that logs one error and returns it unchanged.
type AstReporter = fn(AstError) -> AstError;

/// A factory that builds the error a case should report.
type ErrorFactory = fn() -> AstError;

/// A budget reporter that logs its usage and limit.
type BudgetReporter = fn(ByteSpan, usize, usize);

/// The diagnostic a case expects the subscriber to render.
#[derive(Clone, Copy)]
struct ExpectedReport<'a> {
    /// The level the event must carry.
    level: &'a str,
    /// The message the event must contain.
    message: &'a str,
    /// The structured fields the event must name.
    fields: &'a [&'a str],
}

/// One budget report case: the wrapper under test and what it must say.
#[derive(Clone, Copy)]
struct BudgetCase {
    /// The budget reporter to invoke with an exhausted budget.
    report: BudgetReporter,
    /// The message the rendered event must contain.
    message: &'static str,
    /// The usage field the event must name alongside the limit.
    field: &'static str,
}

/// Render the events emitted while `run` executes, without timestamps' noise.
fn capture_events(run: impl FnOnce()) -> String {
    let mut capture = tempfile::tempfile().expect("diagnostic capture file should be created");
    let subscriber = fmt()
        .with_writer(capture.try_clone().expect("capture file should be cloned"))
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .finish();
    tracing::subscriber::with_default(subscriber, run);
    capture
        .seek(SeekFrom::Start(0))
        .expect("capture file should rewind");
    let mut output = String::new();
    capture
        .read_to_string(&mut output)
        .expect("captured diagnostics should be UTF-8");
    output
}

/// Assert `output` renders the level, message, and fields a case expects.
fn assert_reports(output: &str, expected: ExpectedReport<'_>) {
    assert!(
        output.contains(expected.message),
        "`{}` missing from:\n{output}",
        expected.message
    );
    assert!(
        output.contains(expected.level),
        "`{}` level missing from:\n{output}",
        expected.level
    );
    for field in expected.fields {
        assert!(output.contains(field), "`{field}` missing from:\n{output}");
    }
}

/// A span covering the whole six-byte fixture text.
fn whole_span() -> ByteSpan { ByteSpan::new("abcdef", 0, 6).expect("span should cover text") }

/// A span covering bytes 1..5 of the six-byte fixture text.
fn inner_span() -> ByteSpan { ByteSpan::new("abcdef", 1, 5).expect("span should cover text") }

/// An out-of-bounds span error whose bounds and length are distinguishable.
fn out_of_bounds_error() -> AstError {
    AstError::SpanOutOfBounds {
        start: 2,
        end: 9,
        len: 7,
    }
}

/// An empty-span error whose offset is distinguishable from the other cases.
fn empty_span_error() -> AstError { AstError::EmptySpan { offset: 3 } }

/// A non-character-boundary error whose offset is distinguishable.
fn non_char_boundary_error() -> AstError { AstError::NonCharBoundary { offset: 6 } }

/// Run `report` on a fresh error and assert it returns the same error.
fn assert_error_report(
    report: AstReporter,
    make_error: ErrorFactory,
    expected: ExpectedReport<'_>,
) {
    let output = capture_events(|| {
        let returned = report(make_error());
        assert_eq!(
            returned,
            make_error(),
            "the helper returns its input unchanged"
        );
    });
    assert_reports(&output, expected);
}

/// Check that an out-of-bounds span reports its bounds and length.
#[rstest]
#[case(
    log_span_validation_failure,
    ExpectedReport {
        level: "ERROR",
        message: "AST span lies outside the supplied source text",
        fields: &["start=2", "end=9", "len=7"],
    }
)]
#[case(
    log_covering_failure,
    ExpectedReport {
        level: "ERROR",
        message: "no AST syntax node covers the requested span",
        fields: &["start=2", "end=9", "len=7"],
    }
)]
fn span_out_of_bounds_reports_the_offsets(
    #[case] report: AstReporter,
    #[case] expected: ExpectedReport<'static>,
) {
    assert_error_report(report, out_of_bounds_error, expected);
}

/// Check that any other failure reports the error itself.
#[rstest]
#[case(
    log_span_validation_failure,
    empty_span_error,
    ExpectedReport {
        level: "ERROR",
        message: "AST span validation failed",
        fields: &["EmptySpan", "offset: 3"],
    }
)]
#[case(
    log_covering_failure,
    non_char_boundary_error,
    ExpectedReport {
        level: "ERROR",
        message: "AST covering-node selection failed",
        fields: &["NonCharBoundary", "offset: 6"],
    }
)]
fn other_errors_are_reported_by_debug(
    #[case] report: AstReporter,
    #[case] make_error: ErrorFactory,
    #[case] expected: ExpectedReport<'static>,
) {
    assert_error_report(report, make_error, expected);
}

/// Check that a lowering failure names the parser's error elements.
#[test]
fn lowering_failure_names_parser_errors() {
    let output = capture_events(|| {
        let returned =
            log_lowering_failure(inner_span(), AstError::UnparsableSpan { start: 1, end: 5 });
        assert_eq!(
            returned,
            AstError::UnparsableSpan { start: 1, end: 5 },
            "the helper returns its input unchanged"
        );
    });

    assert_reports(
        &output,
        ExpectedReport {
            level: "ERROR",
            message: "selected AST span contains parser error elements",
            fields: &["start=1", "end=5"],
        },
    );
}

/// Check that a tolerated lowering failure reports nothing.
#[test]
fn lowering_failure_stays_silent_for_a_tolerated_error() {
    let output = capture_events(|| {
        let returned = log_lowering_failure(inner_span(), AstError::TreeTooDeep { limit: 64 });
        assert_eq!(
            returned,
            AstError::TreeTooDeep { limit: 64 },
            "the helper returns its input unchanged"
        );
    });

    assert!(
        output.is_empty(),
        "a tolerated failure must report nothing:\n{output}"
    );
}

/// Check that tolerated parser recovery errors are reported with their count.
#[test]
fn parser_recovery_reports_the_error_count() {
    let output = capture_events(|| log_parser_recovery(inner_span(), 4));

    assert_reports(
        &output,
        ExpectedReport {
            level: "WARN",
            message: "lowered AST span from source with parser recovery errors",
            fields: &["start=1", "end=5", "errors=4"],
        },
    );
}

/// Check that every exhausted budget names its usage field and the limit.
#[rstest]
#[case(BudgetCase {
    report: log_selection_depth_budget,
    message: "AST covering-node selection exceeded the depth budget",
    field: "depth",
})]
#[case(BudgetCase {
    report: log_selection_node_budget,
    message: "AST covering-node selection exceeded the node budget",
    field: "node_count",
})]
#[case(BudgetCase {
    report: log_lowering_depth_budget,
    message: "AST lowering exceeded the depth budget",
    field: "depth",
})]
#[case(BudgetCase {
    report: log_lowering_node_budget,
    message: "AST lowering exceeded the normalized-node budget",
    field: "normalized_node_count",
})]
fn exhausted_budget_reports_the_usage_and_limit(#[case] case: BudgetCase) {
    let usage = format!("{}=9", case.field);
    let output = capture_events(|| (case.report)(whole_span(), 9, 8));

    assert_reports(
        &output,
        ExpectedReport {
            level: "ERROR",
            message: case.message,
            fields: &[usage.as_str(), "limit=8"],
        },
    );
}
