//! Canonical destination paths for prebuilt lint libraries.
//!
//! ADR-001 requires prebuilt artefacts to extract into:
//! `<whitaker_data_dir>/lints/<toolchain>/<target>/lib`.
//! This module centralizes that path derivation so callers do not duplicate
//! directory layout logic.

use camino::Utf8PathBuf;

use crate::dirs::BaseDirs;
use crate::error::{InstallerError, Result};

/// Build the canonical prebuilt library destination directory.
///
/// The returned path is:
/// `<whitaker_data_dir>/lints/<toolchain>/<target>/lib`.
///
/// # Errors
///
/// Returns an error when the platform data directory cannot be determined or
/// cannot be represented as UTF-8.
pub fn prebuilt_library_dir(
    dirs: &dyn BaseDirs,
    toolchain: &str,
    target: &str,
) -> Result<Utf8PathBuf> {
    let base_dir = dirs
        .whitaker_data_dir()
        .ok_or_else(|| InstallerError::StagingFailed {
            reason: "could not determine Whitaker data directory".to_owned(),
        })?;
    let base_dir =
        Utf8PathBuf::from_path_buf(base_dir).map_err(|path| InstallerError::StagingFailed {
            reason: format!(
                "Whitaker data directory is not valid UTF-8: {}",
                path.display()
            ),
        })?;
    Ok(base_dir
        .join("lints")
        .join(toolchain)
        .join(target)
        .join("lib"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dirs::MockBaseDirs;
    use rstest::rstest;
    use std::path::PathBuf;

    #[test]
    fn prebuilt_library_dir_builds_expected_path() {
        let mut dirs = MockBaseDirs::new();
        dirs.expect_whitaker_data_dir()
            .returning(|| Some(PathBuf::from("/home/test/.local/share/whitaker")));

        let result = prebuilt_library_dir(&dirs, "nightly-2025-09-18", "x86_64-unknown-linux-gnu")
            .expect("expected path construction to succeed");
        let expected = Utf8PathBuf::from("/home/test/.local/share/whitaker")
            .join("lints")
            .join("nightly-2025-09-18")
            .join("x86_64-unknown-linux-gnu")
            .join("lib");

        assert_eq!(result, expected);
    }

    #[rstest]
    #[case::missing_data_dir(None, "could not determine Whitaker data directory")]
    fn prebuilt_library_dir_returns_error_on_missing_data_dir(
        #[case] data_dir: Option<PathBuf>,
        #[case] expected_reason: &str,
    ) {
        let mut dirs = MockBaseDirs::new();
        dirs.expect_whitaker_data_dir()
            .return_once(move || data_dir.clone());

        let err = prebuilt_library_dir(&dirs, "nightly-2025-09-18", "x86_64-unknown-linux-gnu")
            .expect_err("expected error");
        assert!(
            matches!(err, InstallerError::StagingFailed { ref reason } if reason.contains(expected_reason)),
            "unexpected error: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn prebuilt_library_dir_rejects_non_utf8_data_dir() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let mut dirs = MockBaseDirs::new();
        dirs.expect_whitaker_data_dir().return_once(|| {
            Some(PathBuf::from(OsString::from_vec(vec![
                b'/', b't', b'm', b'p', b'/', 0xff,
            ])))
        });

        let err = prebuilt_library_dir(&dirs, "nightly-2025-09-18", "x86_64-unknown-linux-gnu")
            .expect_err("expected UTF-8 conversion error");
        assert!(
            matches!(err, InstallerError::StagingFailed { ref reason } if reason.contains("not valid UTF-8")),
            "unexpected error: {err}"
        );
    }
}
