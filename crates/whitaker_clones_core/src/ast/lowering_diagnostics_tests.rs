//! Tests for the diagnostics emitted by the AST lowering adapter.
//!
//! Each case installs a formatting subscriber that writes to a temporary file,
//! invokes one reporting helper, and asserts the rendered message, level, and
//! fields. The decisions that choose a diagnostic are covered by
//! `lowering_tests`; these cases prove the report names the same facts, which
//! is what an operator reading the log actually depends on.

use std::io::{Read, Seek, SeekFrom};

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

/// Assert `output` names `message` and every one of `fields`.
fn assert_reports(output: &str, level: &str, message: &str, fields: &[&str]) {
    assert!(
        output.contains(message),
        "`{message}` missing from:\n{output}"
    );
    assert!(
        output.contains(level),
        "`{level}` level missing from:\n{output}"
    );
    for field in fields {
        assert!(output.contains(field), "`{field}` missing from:\n{output}");
    }
}

#[test]
fn span_validation_failure_reports_the_offsets() {
    let error = AstError::SpanOutOfBounds {
        start: 2,
        end: 9,
        len: 7,
    };

    let output = capture_events(|| {
        let returned = log_span_validation_failure(error);
        assert_eq!(
            returned,
            AstError::SpanOutOfBounds {
                start: 2,
                end: 9,
                len: 7,
            },
            "the helper returns its input unchanged"
        );
    });

    assert_reports(
        &output,
        "ERROR",
        "AST span lies outside the supplied source text",
        &["start=2", "end=9", "len=7"],
    );
}

#[test]
fn span_validation_failure_reports_other_errors_by_debug() {
    let error = AstError::EmptySpan { offset: 3 };

    let output = capture_events(|| {
        let returned = log_span_validation_failure(error);
        assert_eq!(
            returned,
            AstError::EmptySpan { offset: 3 },
            "the helper returns its input unchanged"
        );
    });

    assert_reports(
        &output,
        "ERROR",
        "AST span validation failed",
        &["EmptySpan", "offset: 3"],
    );
}

#[test]
fn covering_failure_reports_the_offsets() {
    let error = AstError::SpanOutOfBounds {
        start: 4,
        end: 12,
        len: 11,
    };

    let output = capture_events(|| {
        let returned = log_covering_failure(error);
        assert_eq!(
            returned,
            AstError::SpanOutOfBounds {
                start: 4,
                end: 12,
                len: 11,
            },
            "the helper returns its input unchanged"
        );
    });

    assert_reports(
        &output,
        "ERROR",
        "no AST syntax node covers the requested span",
        &["start=4", "end=12", "len=11"],
    );
}

#[test]
fn covering_failure_reports_other_errors_by_debug() {
    let error = AstError::NonCharBoundary { offset: 6 };

    let output = capture_events(|| {
        let returned = log_covering_failure(error);
        assert_eq!(
            returned,
            AstError::NonCharBoundary { offset: 6 },
            "the helper returns its input unchanged"
        );
    });

    assert_reports(
        &output,
        "ERROR",
        "AST covering-node selection failed",
        &["NonCharBoundary", "offset: 6"],
    );
}

#[test]
fn lowering_failure_names_parser_errors() {
    let span = ByteSpan::new("abcdef", 1, 5).expect("span should cover text");

    let output = capture_events(|| {
        let returned = log_lowering_failure(span, AstError::UnparsableSpan { start: 1, end: 5 });
        assert_eq!(
            returned,
            AstError::UnparsableSpan { start: 1, end: 5 },
            "the helper returns its input unchanged"
        );
    });

    assert_reports(
        &output,
        "ERROR",
        "selected AST span contains parser error elements",
        &["start=1", "end=5"],
    );
}

#[test]
fn lowering_failure_stays_silent_for_a_tolerated_error() {
    let span = ByteSpan::new("abcdef", 1, 5).expect("span should cover text");

    let output = capture_events(|| {
        let returned = log_lowering_failure(span, AstError::TreeTooDeep { limit: 64 });
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

#[test]
fn parser_recovery_reports_the_error_count() {
    let span = ByteSpan::new("abcdef", 1, 5).expect("span should cover text");

    let output = capture_events(|| log_parser_recovery(span, 4));

    assert_reports(
        &output,
        "WARN",
        "lowered AST span from source with parser recovery errors",
        &["start=1", "end=5", "errors=4"],
    );
}

#[test]
fn selection_depth_budget_reports_the_budget() {
    let span = ByteSpan::new("abcdef", 0, 6).expect("span should cover text");

    let output = capture_events(|| log_selection_depth_budget(span, 9, 8));

    assert_reports(
        &output,
        "ERROR",
        "AST covering-node selection exceeded the depth budget",
        &["depth=9", "limit=8"],
    );
}

#[test]
fn selection_node_budget_reports_the_budget() {
    let span = ByteSpan::new("abcdef", 0, 6).expect("span should cover text");

    let output = capture_events(|| log_selection_node_budget(span, 130, 128));

    assert_reports(
        &output,
        "ERROR",
        "AST covering-node selection exceeded the node budget",
        &["node_count=130", "limit=128"],
    );
}

#[test]
fn lowering_depth_budget_reports_the_budget() {
    let span = ByteSpan::new("abcdef", 0, 6).expect("span should cover text");

    let output = capture_events(|| log_lowering_depth_budget(span, 33, 32));

    assert_reports(
        &output,
        "ERROR",
        "AST lowering exceeded the depth budget",
        &["depth=33", "limit=32"],
    );
}

#[test]
fn lowering_node_budget_reports_the_budget() {
    let span = ByteSpan::new("abcdef", 0, 6).expect("span should cover text");

    let output = capture_events(|| log_lowering_node_budget(span, 2049, 2048));

    assert_reports(
        &output,
        "ERROR",
        "AST lowering exceeded the normalized-node budget",
        &["normalized_node_count=2049", "limit=2048"],
    );
}
