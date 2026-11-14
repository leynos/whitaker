//! Diagnostic assertions for the macro-aware lint smoke test.
//!
//! The consumer run is parsed through Cargo's JSON message stream so each
//! assertion can name the offending diagnostic rather than matching rendered
//! text. Feature-on and feature-off expectations are split because `-D
//! warnings` must deny the finding only when the lint is compiled in.

use std::io::Cursor;

use cargo_metadata::{
    Message,
    diagnostic::{Diagnostic, DiagnosticLevel},
};

use super::{CONSUMER_SOURCE, LINT_NAME, Run};

/// Parse the consumer run's compiler diagnostics.
fn diagnostics(run: &Run) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    for message in Message::parse_stream(Cursor::new(&run.stdout)) {
        match message {
            Ok(Message::CompilerMessage(compiler_message)) => {
                found.push(compiler_message.message);
            }
            Ok(_) => {}
            Err(error) => panic!("failed to parse the consumer JSON stream: {error}"),
        }
    }
    found
}

/// Return the diagnostics carrying `code`.
fn diagnostics_with_code<'a>(diagnostics: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .code
                .as_ref()
                .is_some_and(|candidate| candidate.code == code)
        })
        .collect()
}

/// Fail when the run contains an error-level diagnostic from another source.
fn assert_only_lint_errors(diagnostics: &[Diagnostic], stderr: &str) {
    for diagnostic in diagnostics {
        if diagnostic.level != DiagnosticLevel::Error {
            continue;
        }
        if let Some(code) = &diagnostic.code {
            assert_eq!(
                code.code, LINT_NAME,
                "the consumer run reported an unrelated error: {}\nstderr:\n{stderr}",
                diagnostic.message
            );
        } else {
            assert!(
                diagnostic.message.starts_with("aborting due to"),
                "the consumer run reported an uncoded error: {}\nstderr:\n{stderr}",
                diagnostic.message
            );
        }
    }
}

/// Return the expected 1-based anchor of the consumer's only function.
fn expected_anchor() -> (usize, usize) {
    let Some(index) = CONSUMER_SOURCE
        .lines()
        .position(|candidate| candidate.contains("fn authored_if"))
    else {
        panic!("the consumer source must contain the function");
    };
    let line = index + 1;
    let Some(source_line) = CONSUMER_SOURCE.lines().nth(index) else {
        panic!("the consumer function line must be reachable");
    };
    let Some(column) = source_line.find("authored_if") else {
        panic!("the consumer function line must contain its name");
    };
    (line, column + 1)
}

/// Assert the feature-on diagnostic is the only failure and sits on the function.
pub(super) fn assert_enabled_run(run: &Run) {
    assert!(
        !run.is_success,
        "`-D warnings` must deny the finding, but the run succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        run.stderr
    );
    let found = diagnostics(run);
    assert_only_lint_errors(&found, &run.stderr);

    let matching = diagnostics_with_code(&found, LINT_NAME);
    assert_eq!(
        matching.len(),
        1,
        "exactly one `{LINT_NAME}` diagnostic is expected, got {matching:#?}\nstderr:\n{}",
        run.stderr
    );
    let Some(diagnostic) = matching.first() else {
        panic!("the run carries no `{LINT_NAME}` diagnostic");
    };
    assert!(
        diagnostic.message.contains("authored_if"),
        "the diagnostic must name the function, got `{}`",
        diagnostic.message
    );
    let Some(primary) = diagnostic.spans.iter().find(|span| span.is_primary) else {
        panic!("the diagnostic must carry a primary span");
    };
    assert_eq!(
        (primary.line_start, primary.column_start),
        expected_anchor(),
        "the diagnostic must anchor on the function identifier",
    );
}

/// Assert the feature-off run is clean and never reports the lint.
pub(super) fn assert_disabled_run(run: &Run) {
    assert!(
        run.is_success,
        "the feature-off run must not deny anything\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        run.stderr
    );
    let found = diagnostics(run);
    assert!(
        diagnostics_with_code(&found, LINT_NAME).is_empty(),
        "the feature-off suite must not emit `{LINT_NAME}`",
    );
}
