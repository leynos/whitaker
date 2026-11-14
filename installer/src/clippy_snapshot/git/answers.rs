//! Interpret Git's scalar answers and failures as adapter results.
//!
//! Git states some facts as a bare word and reports failures only as text on
//! standard error. Both need translating at the adapter boundary: a word that
//! is neither `true` nor `false` is not evidence, and an empty diagnostic still
//! has to name the operation that failed.

use std::process::Output;

use super::ClippySnapshotError;

/// Interpret one of Git's `true`/`false` answers, refusing anything else.
///
/// Git documents these questions as printing `true` or `false`. Treating any
/// other successful output as "not shallow" would accept history whose depth
/// the adapter never established, so an unrecognized answer is reported as an
/// infrastructure fault instead of becoming evidence.
pub(super) fn boolean_answer(
    operation: &'static str,
    question: &str,
    answer: &str,
) -> Result<bool, ClippySnapshotError> {
    match answer {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(ClippySnapshotError::Git {
            operation,
            message: format!(
                "expected `true` or `false` for the {question} question, got `{other}`"
            ),
        }),
    }
}

/// Describe a failed Git command from its standard error.
pub(super) fn git_failure(operation: &'static str, output: &Output) -> ClippySnapshotError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = match stderr.trim() {
        "" => format!("exited with {}", output.status),
        trimmed => trimmed.to_owned(),
    };
    ClippySnapshotError::Git { operation, message }
}

/// Describe what an integrity check reported.
///
/// The command-specific wording belongs to the adapter, so policy receives a
/// finished diagnostic and only decides whether the recorded result is
/// acceptable. Taking the exit result and the raw standard error, rather than
/// a process handle, keeps every branch testable without running Git.
pub(super) fn integrity_diagnostic(succeeded: bool, stderr: &[u8]) -> String {
    if succeeded {
        return String::new();
    }
    let message = String::from_utf8_lossy(stderr);
    match message.trim() {
        "" => "git fsck --full failed without a diagnostic".to_owned(),
        trimmed => format!("git fsck --full failed: {trimmed}"),
    }
}

#[cfg(test)]
#[path = "answers_tests.rs"]
mod tests;
