//! Every snapshot failure states a remedy and refuses the public fallback.

use super::*;

/// One value of every variant.
///
/// [`is_listed`] matches exhaustively, so adding a variant without adding it
/// here fails to compile rather than escaping the contract below.
fn every_variant() -> Vec<ClippySnapshotError> {
    let text = || "detail".to_owned();
    vec![
        ClippySnapshotError::Manifest { reason: text() },
        ClippySnapshotError::BundleMissing {
            path: text(),
            asset: text(),
        },
        ClippySnapshotError::Sidecar {
            path: text(),
            reason: text(),
        },
        ClippySnapshotError::ChecksumMismatch {
            path: text(),
            expected: text(),
            actual: text(),
        },
        ClippySnapshotError::AssetNotFound { url: text() },
        ClippySnapshotError::Download {
            asset: text(),
            reason: text(),
        },
        ClippySnapshotError::NotInstalled { path: text() },
        ClippySnapshotError::Invalid {
            path: text(),
            check: text(),
        },
        ClippySnapshotError::InsufficientHistory {
            path: text(),
            toolchain: text(),
            reason: text(),
        },
        ClippySnapshotError::Git {
            operation: "clone",
            message: text(),
        },
        ClippySnapshotError::Io {
            path: text(),
            reason: text(),
        },
    ]
}

/// Compile-time guard that [`every_variant`] covers the enum.
const fn is_listed(error: &ClippySnapshotError) -> bool {
    match error {
        ClippySnapshotError::Manifest { .. }
        | ClippySnapshotError::BundleMissing { .. }
        | ClippySnapshotError::Sidecar { .. }
        | ClippySnapshotError::ChecksumMismatch { .. }
        | ClippySnapshotError::AssetNotFound { .. }
        | ClippySnapshotError::Download { .. }
        | ClippySnapshotError::NotInstalled { .. }
        | ClippySnapshotError::Invalid { .. }
        | ClippySnapshotError::InsufficientHistory { .. }
        | ClippySnapshotError::Git { .. }
        | ClippySnapshotError::Io { .. } => true,
    }
}

#[test]
fn every_failure_refuses_the_public_clippy_fallback() {
    for error in every_variant() {
        assert!(is_listed(&error));
        assert!(
            error.to_string().ends_with(NO_FALLBACK),
            "{error:?} must end with the no-fallback statement: {error}"
        );
    }
}

/// Verbs a remedy may open with.
const REMEDY_VERBS: &[&str] = &[
    "check",
    "delete",
    "download",
    "install",
    "pass",
    "publish",
    "re-run",
    "reinstall",
    "remove",
    "run",
];

/// The clause just before the no-fallback statement is an instruction, so a
/// message never stops at "X failed".
#[test]
fn every_failure_ends_with_what_to_do_next() {
    for error in every_variant() {
        let message = error.to_string();
        let body = message
            .strip_suffix(NO_FALLBACK)
            .expect("checked by the test above")
            .trim_end()
            .trim_end_matches('.');
        // Clauses end at "; " or ". "; a bare dot may be part of `.sha256`.
        let clause_start = ["; ", ". "]
            .iter()
            .filter_map(|separator| body.rfind(separator).map(|at| at + separator.len()))
            .max()
            .unwrap_or(0);
        let last_clause = &body[clause_start..];
        let first_word = last_clause
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_lowercase();
        assert!(
            REMEDY_VERBS.contains(&first_word.as_str()),
            "{error:?} ends without a remedy: {message}"
        );
    }
}
