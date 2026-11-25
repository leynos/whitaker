//! Debug-only staged-suite shortcut used by installer behavioural tests.
//!
//! The real installer should build or download a loadable suite library. This
//! helper exists only so debug-built test binaries can stage a cheap synthetic
//! artefact instead of recursively rebuilding the workspace inside nextest.

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use crate::{
    crate_name::CrateName,
    error::{InstallerError, Result},
    resolution::SUITE_CRATE,
    stager::Stager,
    test_support::TEST_STAGE_SUITE_ENV,
    toolchain::Toolchain,
};

pub(crate) fn try_test_staged_suite_installation(
    requested_crates: &[CrateName],
    toolchain: &Toolchain,
    target_dir: &Utf8Path,
) -> Result<Option<Utf8PathBuf>> {
    stage_suite_when(
        std::env::var(TEST_STAGE_SUITE_ENV).ok().as_deref(),
        requested_crates,
        toolchain,
        target_dir,
    )
}

/// Stage the placeholder suite when `switch` is `"1"`, the request is for the
/// suite alone, and this is a debug build.
///
/// `switch` is the value of [`TEST_STAGE_SUITE_ENV`], passed in so tests can
/// supply it without changing the process environment.
fn stage_suite_when(
    switch: Option<&str>,
    requested_crates: &[CrateName],
    toolchain: &Toolchain,
    target_dir: &Utf8Path,
) -> Result<Option<Utf8PathBuf>> {
    if !cfg!(debug_assertions) {
        return Ok(None);
    }

    if switch != Some("1") || !is_suite_only_request(requested_crates) {
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
    //! Tests for staged-suite gating, placeholder staging, and write-failure
    //! handling.
    //!
    //! This `mod tests` module uses `rstest` fixtures and passes the switch
    //! value in directly to exercise the debug-only staged suite shortcuts
    //! without touching the process environment.

    use rstest::{fixture, rstest};
    use tempfile::TempDir;

    use super::*;

    struct StagedSuiteSetup {
        _temp_dir: TempDir,
        toolchain: Toolchain,
        target_dir: Utf8PathBuf,
    }

    impl StagedSuiteSetup {
        fn requested_suite_crates() -> Vec<CrateName> { vec![CrateName::from(SUITE_CRATE)] }

        fn stager(&self) -> Stager {
            Stager::new(self.target_dir.clone(), self.toolchain.channel())
        }

        fn create_blocked_suite_output(&self) -> Result<Utf8PathBuf> {
            let stager = self.stager();
            stager.prepare()?;

            let blocked_path = stager
                .staging_path()
                .join(stager.staged_filename(&CrateName::from(SUITE_CRATE)));
            std::fs::create_dir_all(blocked_path.as_std_path())?;
            Ok(blocked_path)
        }
    }

    fn test_toolchain() -> Toolchain {
        Toolchain::with_override(Utf8Path::new("."), "nightly-2026-05-28")
    }

    fn utf8_temp_dir(temp_dir: &TempDir) -> std::io::Result<Utf8PathBuf> {
        Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| std::io::Error::other("temporary directory path must be UTF-8"))
    }

    #[fixture]
    fn staged_suite_setup() -> std::io::Result<StagedSuiteSetup> {
        let temp_dir = tempfile::tempdir()?;
        let target_dir = utf8_temp_dir(&temp_dir)?;
        let toolchain = test_toolchain();

        Ok(StagedSuiteSetup {
            _temp_dir: temp_dir,
            toolchain,
            target_dir,
        })
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
    #[case::env_disabled((vec![CrateName::from(SUITE_CRATE)], Some("0")))]
    #[case::non_suite((vec![CrateName::from("module_max_lines")], Some("1")))]
    fn staged_suite_installation_skips_non_staging_requests(
        #[from(staged_suite_setup)] setup_res: std::io::Result<StagedSuiteSetup>,
        #[case] case: (Vec<CrateName>, Option<&str>),
    ) {
        let staged_suite_setup = setup_res.expect("staged suite setup should be created");

        let (requested_crates, env_val) = case;
        let result = stage_suite_when(
            env_val,
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
    }

    #[cfg(debug_assertions)]
    #[rstest]
    fn staged_suite_installation_writes_placeholder_library_for_suite_requests(
        #[from(staged_suite_setup)] setup_res: std::io::Result<StagedSuiteSetup>,
    ) {
        let staged_suite_setup = setup_res.expect("staged suite setup should be created");

        let requested_crates = StagedSuiteSetup::requested_suite_crates();
        let stager = staged_suite_setup.stager();

        let staging_path = stage_suite_when(
            Some("1"),
            &requested_crates,
            &staged_suite_setup.toolchain,
            &staged_suite_setup.target_dir,
        )
        .expect("expected staged-suite installation to succeed")
        .expect("expected suite request to stage a placeholder library");
        assert_eq!(staging_path, stager.staging_path());

        let staged_file = staging_path.join(stager.staged_filename(&CrateName::from(SUITE_CRATE)));
        let contents = std::fs::read(staged_file.as_std_path())
            .expect("expected staged placeholder suite library to exist");
        assert_eq!(contents, b"test-only staged suite library");
    }

    #[cfg(debug_assertions)]
    #[rstest]
    fn staged_suite_installation_surfaces_write_failures(
        #[from(staged_suite_setup)] setup_res: std::io::Result<StagedSuiteSetup>,
    ) {
        let staged_suite_setup = setup_res.expect("staged suite setup should be created");

        let requested_crates = StagedSuiteSetup::requested_suite_crates();
        let blocked_path = staged_suite_setup
            .create_blocked_suite_output()
            .expect("blocked staged output should be pre-created");

        let err = stage_suite_when(
            Some("1"),
            &requested_crates,
            &staged_suite_setup.toolchain,
            &staged_suite_setup.target_dir,
        )
        .expect_err("expected directory collision to fail staged-suite write");
        assert!(
            matches!(
                &err,
                InstallerError::StagingFailed { reason }
                    if reason.contains("failed to write test-only staged suite library")
                        && reason.contains(blocked_path.as_str())
            ),
            "expected a staging failure naming {blocked_path}, got {err:?}"
        );
    }

    #[cfg(not(debug_assertions))]
    #[rstest]
    fn staged_suite_installation_is_disabled_in_release_builds(
        #[from(staged_suite_setup)] setup_res: std::io::Result<StagedSuiteSetup>,
    ) {
        let staged_suite_setup = setup_res.expect("staged suite setup should be created");

        let requested_crates = StagedSuiteSetup::requested_suite_crates();
        let staging_dir = staged_suite_setup
            .target_dir
            .join(staged_suite_setup.toolchain.channel())
            .join("release");

        let result = stage_suite_when(
            Some("1"),
            &requested_crates,
            &staged_suite_setup.toolchain,
            &staged_suite_setup.target_dir,
        )
        .expect("expected release builds to skip staged-suite installation");
        assert!(result.is_none());
        assert!(
            !staging_dir.exists(),
            "expected release builds to skip creating a staging directory"
        );
    }
}
