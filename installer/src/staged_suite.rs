//! Debug-only staged-suite shortcut used by installer behavioural tests.
//!
//! The real installer should build or download a loadable suite library. This
//! helper exists only so debug-built test binaries can stage a cheap synthetic
//! artefact instead of recursively rebuilding the workspace inside nextest.

use camino::{Utf8Path, Utf8PathBuf};
use std::fs;
use whitaker_installer::crate_name::CrateName;
use whitaker_installer::error::{InstallerError, Result};
use whitaker_installer::resolution::SUITE_CRATE;
use whitaker_installer::stager::Stager;
use whitaker_installer::test_support::TEST_STAGE_SUITE_ENV;
use whitaker_installer::toolchain::Toolchain;

pub(crate) fn try_test_staged_suite_installation(
    requested_crates: &[CrateName],
    toolchain: &Toolchain,
    target_dir: &Utf8Path,
) -> Result<Option<Utf8PathBuf>> {
    if !cfg!(debug_assertions) {
        return Ok(None);
    }

    if std::env::var_os(TEST_STAGE_SUITE_ENV).is_none() || !is_suite_only_request(requested_crates)
    {
        return Ok(None);
    }

    let stager = Stager::new(target_dir.to_owned(), toolchain.channel());
    stager.prepare()?;

    let staged_path = stager
        .staging_path()
        .join(stager.staged_filename(&CrateName::from(SUITE_CRATE)));
    fs::write(staged_path.as_std_path(), b"test-only staged suite library").map_err(|error| {
        InstallerError::StagingFailed {
            reason: format!(
                "failed to write test-only staged suite library at {staged_path}: {error}"
            ),
        }
    })?;

    Ok(Some(stager.staging_path()))
}

fn is_suite_only_request(requested_crates: &[CrateName]) -> bool {
    matches!(requested_crates, [crate_name] if crate_name.as_str() == SUITE_CRATE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use temp_env::{with_var, with_var_unset};
    use tempfile::TempDir;
    use whitaker_installer::test_support::env_test_guard;

    struct StagedSuiteSetup {
        _guard: std::sync::MutexGuard<'static, ()>,
        _temp_dir: TempDir,
        toolchain: Toolchain,
        target_dir: Utf8PathBuf,
    }

    fn test_toolchain() -> Toolchain {
        Toolchain::with_override(Utf8Path::new("."), "nightly-2025-09-18")
    }

    fn utf8_temp_dir(temp_dir: &TempDir) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .expect("expected UTF-8 temp path for staged suite tests")
    }

    #[fixture]
    fn staged_suite_setup() -> StagedSuiteSetup {
        let guard = env_test_guard();
        let temp_dir = tempfile::tempdir().expect("expected temp dir for staged suite tests");
        let target_dir = utf8_temp_dir(&temp_dir);
        let toolchain = test_toolchain();

        StagedSuiteSetup {
            _guard: guard,
            _temp_dir: temp_dir,
            toolchain,
            target_dir,
        }
    }

    #[rstest]
    #[case::single_suite(vec![CrateName::from(SUITE_CRATE)], true)]
    #[case::single_non_suite(vec![CrateName::from("module_max_lines")], false)]
    #[case::suite_plus_other(
        vec![CrateName::from(SUITE_CRATE), CrateName::from("module_max_lines")],
        false
    )]
    #[case::empty(vec![], false)]
    fn suite_only_request_requires_exact_single_suite_crate(
        #[case] requested_crates: Vec<CrateName>,
        #[case] expected: bool,
    ) {
        assert_eq!(is_suite_only_request(&requested_crates), expected);
    }

    #[rstest]
    #[case::env_unset((vec![CrateName::from(SUITE_CRATE)], None))]
    #[case::non_suite((vec![CrateName::from("module_max_lines")], Some("1")))]
    fn staged_suite_installation_skips_non_staging_requests(
        staged_suite_setup: StagedSuiteSetup,
        #[case] case: (Vec<CrateName>, Option<&str>),
    ) {
        let (requested_crates, env_val) = case;
        let run = || {
            let result = try_test_staged_suite_installation(
                &requested_crates,
                &staged_suite_setup.toolchain,
                &staged_suite_setup.target_dir,
            )
            .expect("expected staged-suite installation to be skipped");
            assert!(result.is_none());
            assert!(
                !staged_suite_setup
                    .target_dir
                    .join(staged_suite_setup.toolchain.channel())
                    .join("release")
                    .exists(),
                "expected no staging directory to be created"
            );
        };

        match env_val {
            Some(val) => with_var(TEST_STAGE_SUITE_ENV, Some(val), run),
            None => with_var_unset(TEST_STAGE_SUITE_ENV, run),
        }
    }

    #[test]
    fn staged_suite_installation_writes_placeholder_library_for_suite_requests() {
        let _guard = env_test_guard();
        let temp_dir = tempfile::tempdir().expect("expected temp dir for staged suite tests");
        let target_dir = utf8_temp_dir(&temp_dir);
        let toolchain = test_toolchain();
        let requested_crates = vec![CrateName::from(SUITE_CRATE)];
        let stager = Stager::new(target_dir.clone(), toolchain.channel());

        with_var(TEST_STAGE_SUITE_ENV, Some("1"), || {
            let staging_path =
                try_test_staged_suite_installation(&requested_crates, &toolchain, &target_dir)
                    .expect("expected staged-suite installation to succeed")
                    .expect("expected suite request to stage a placeholder library");
            assert_eq!(staging_path, stager.staging_path());

            let staged_file =
                staging_path.join(stager.staged_filename(&CrateName::from(SUITE_CRATE)));
            let contents = std::fs::read(staged_file.as_std_path())
                .expect("expected staged placeholder suite library to exist");
            assert_eq!(contents, b"test-only staged suite library");
        });
    }

    #[test]
    fn staged_suite_installation_surfaces_write_failures() {
        let _guard = env_test_guard();
        let temp_dir = tempfile::tempdir().expect("expected temp dir for staged suite tests");
        let target_dir = utf8_temp_dir(&temp_dir);
        let toolchain = test_toolchain();
        let requested_crates = vec![CrateName::from(SUITE_CRATE)];
        let stager = Stager::new(target_dir.clone(), toolchain.channel());
        stager
            .prepare()
            .expect("expected staging directory to be writable for test setup");

        let blocked_path = stager
            .staging_path()
            .join(stager.staged_filename(&CrateName::from(SUITE_CRATE)));
        std::fs::create_dir_all(blocked_path.as_std_path())
            .expect("expected to pre-create staged filename as a directory");

        with_var(TEST_STAGE_SUITE_ENV, Some("1"), || {
            let err =
                try_test_staged_suite_installation(&requested_crates, &toolchain, &target_dir)
                    .expect_err("expected directory collision to fail staged-suite write");
            assert!(matches!(
                err,
                InstallerError::StagingFailed { reason }
                    if reason.contains("failed to write test-only staged suite library")
                        && reason.contains(blocked_path.as_str())
            ));
        });
    }
}
