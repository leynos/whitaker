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
