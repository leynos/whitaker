//! Install-flow helpers for the installer binary.
//!
//! This module keeps prebuilt-download fallback and metrics recording logic
//! separate from CLI orchestration in `main.rs`.

use camino::Utf8Path;
use camino::Utf8PathBuf;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::io::Write;
use std::time::Duration;
use whitaker_installer::builder::{library_extension, library_prefix};
use whitaker_installer::cli::InstallArgs;
use whitaker_installer::crate_name::CrateName;
use whitaker_installer::dirs::BaseDirs;
use whitaker_installer::error::{InstallerError, Result};
use whitaker_installer::install_metrics::{InstallMode, RecordOutcome, record_install};
use whitaker_installer::output::write_stderr_line;
use whitaker_installer::prebuilt::{PrebuiltConfig, PrebuiltResult, attempt_prebuilt};
use whitaker_installer::prebuilt_path::prebuilt_library_dir;
use whitaker_installer::resolution::{EXPERIMENTAL_LINT_CRATES, LINT_CRATES, SUITE_CRATE};

/// Context needed to attempt prebuilt installation.
pub(crate) struct PrebuiltInstallationContext<'a> {
    /// CLI install arguments.
    pub(crate) args: &'a InstallArgs,
    /// Base directory provider.
    pub(crate) dirs: &'a dyn BaseDirs,
    /// Crates requested for this installation.
    pub(crate) requested_crates: &'a [CrateName],
    /// Toolchain channel resolved for this install.
    pub(crate) toolchain_channel: &'a str,
}

/// Context for recording one successful install in aggregate metrics.
pub(crate) struct MetricsWriteContext<'a> {
    /// Whether installer output is suppressed.
    pub(crate) quiet: bool,
    /// Base directory provider used to locate Whitaker data directory.
    pub(crate) dirs: &'a dyn BaseDirs,
    /// Terminal install mode (download or build).
    pub(crate) install_mode: InstallMode,
    /// Elapsed duration for this successful install run.
    pub(crate) elapsed: Duration,
}

/// Write fallback message when prebuilt installation fails.
pub(crate) fn write_prebuilt_fallback_message(
    quiet: bool,
    error: &dyn std::fmt::Display,
    stderr: &mut dyn Write,
) {
    if quiet {
        return;
    }
    write_stderr_line(stderr, format!("Prebuilt download unavailable: {error}"));
    write_stderr_line(stderr, "Falling back to local compilation.");
    write_stderr_line(stderr, "");
}

/// Attempt prebuilt installation and return staged path when successful.
pub(crate) fn try_prebuilt_installation(
    context: &PrebuiltInstallationContext<'_>,
    stderr: &mut dyn Write,
) -> Result<Option<Utf8PathBuf>> {
    try_prebuilt_installation_with(
        context,
        stderr,
        PrebuiltInstallationHooks {
            detect_host_target,
            resolve_destination_dir: prebuilt_library_dir,
            attempt_prebuilt,
            prune_prebuilt_libraries,
        },
    )
}

type DetectHostTargetFn = fn() -> Result<String>;
type ResolveDestinationDirFn = fn(&dyn BaseDirs, &str, &str) -> Result<Utf8PathBuf>;
type AttemptPrebuiltFn = fn(&PrebuiltConfig<'_>, &mut dyn Write) -> PrebuiltResult;
type PruneLibrariesFn = fn(&Utf8Path, &str, &[CrateName]) -> Result<()>;

struct PrebuiltInstallationHooks {
    detect_host_target: DetectHostTargetFn,
    resolve_destination_dir: ResolveDestinationDirFn,
    attempt_prebuilt: AttemptPrebuiltFn,
    prune_prebuilt_libraries: PruneLibrariesFn,
}

fn try_prebuilt_installation_with(
    context: &PrebuiltInstallationContext<'_>,
    stderr: &mut dyn Write,
    hooks: PrebuiltInstallationHooks,
) -> Result<Option<Utf8PathBuf>> {
    let PrebuiltInstallationHooks {
        detect_host_target,
        resolve_destination_dir,
        attempt_prebuilt,
        prune_prebuilt_libraries,
    } = hooks;

    if !context
        .args
        .should_attempt_prebuilt(context.requested_crates)
    {
        return Ok(None);
    }

    let host_target = match detect_host_target() {
        Ok(target) => target,
        Err(error) => {
            write_prebuilt_fallback_message(context.args.quiet, &error, stderr);
            return Ok(None);
        }
    };

    let destination_dir =
        match resolve_destination_dir(context.dirs, context.toolchain_channel, &host_target) {
            Ok(destination) => destination,
            Err(error) => {
                write_prebuilt_fallback_message(context.args.quiet, &error, stderr);
                return Ok(None);
            }
        };

    let prebuilt_config = PrebuiltConfig {
        target: &host_target,
        toolchain: context.toolchain_channel,
        destination_dir: &destination_dir,
        quiet: context.args.quiet,
    };

    let PrebuiltResult::Success { staging_path } = attempt_prebuilt(&prebuilt_config, stderr)
    else {
        return Ok(None);
    };
    if let Err(error) = prune_prebuilt_libraries(
        &staging_path,
        context.toolchain_channel,
        context.requested_crates,
    ) {
        write_prebuilt_fallback_message(context.args.quiet, &error, stderr);
        return Ok(None);
    }
    Ok(Some(staging_path))
}

fn requested_crate_names(requested_crates: &[CrateName]) -> HashSet<&str> {
    if requested_crates.is_empty() {
        return HashSet::from([SUITE_CRATE]);
    }
    requested_crates.iter().map(CrateName::as_str).collect()
}

fn staged_library_filename(crate_name: &str, toolchain_channel: &str) -> String {
    let normalized = crate_name.replace('-', "_");
    format!(
        "{}{}@{}{}",
        library_prefix(),
        normalized,
        toolchain_channel,
        library_extension()
    )
}

/// Remove staged prebuilt libraries that were not requested for this install.
///
/// Prebuilt archives can contain both suite and constituent crates, while local
/// builds stage only requested crates. Pruning keeps prebuilt installs aligned
/// with local-build behaviour and avoids duplicate lint registration when the
/// wrapper runs `cargo dylint --all`.
fn prune_prebuilt_libraries(
    staging_path: &Utf8Path,
    toolchain_channel: &str,
    requested_crates: &[CrateName],
) -> Result<()> {
    let requested = requested_crate_names(requested_crates);
    let known_crates = LINT_CRATES
        .iter()
        .copied()
        .chain(EXPERIMENTAL_LINT_CRATES.iter().copied())
        .chain(std::iter::once(SUITE_CRATE));

    for crate_name in known_crates {
        if requested.contains(crate_name) {
            continue;
        }

        let stale_path = staging_path.join(staged_library_filename(crate_name, toolchain_channel));
        match fs::remove_file(stale_path.as_std_path()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(InstallerError::StagingFailed {
                    reason: format!(
                        "failed to remove stale prebuilt library {stale_path}: {error}"
                    ),
                });
            }
        }
    }

    Ok(())
}

/// Detect the host target triple by parsing `rustc -vV` output.
pub(crate) fn detect_host_target() -> Result<String> {
    let output = std::process::Command::new("rustc")
        .args(["-vV"])
        .output()
        .map_err(|error| InstallerError::ToolchainDetection {
            reason: format!("failed to run `rustc -vV`: {error}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::ToolchainDetection {
            reason: format!(
                "`rustc -vV` exited with {}: {}",
                output.status,
                stderr.trim()
            ),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(host) = line.strip_prefix("host: ") {
            return Ok(host.trim().to_owned());
        }
    }
    Err(InstallerError::ToolchainDetection {
        reason: "could not determine host target from `rustc -vV`".to_owned(),
    })
}

/// Best-effort metrics recording for successful installation runs.
pub(crate) fn write_install_metrics(context: &MetricsWriteContext<'_>, stderr: &mut dyn Write) {
    match record_install(context.dirs, context.install_mode, context.elapsed) {
        Ok(record_outcome) => write_metrics_summary(context.quiet, &record_outcome, stderr),
        Err(error) => {
            if !context.quiet {
                write_stderr_line(
                    stderr,
                    format!("Warning: could not record install metrics: {error}"),
                );
            }
        }
    }
}

fn write_metrics_summary(quiet: bool, record_outcome: &RecordOutcome, stderr: &mut dyn Write) {
    if quiet {
        return;
    }

    if record_outcome.recovered_from_corrupt_file() {
        write_stderr_line(
            stderr,
            "Install metrics file was invalid and has been reset.",
        );
    }
    write_stderr_line(stderr, record_outcome.metrics().summary_line());
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use rstest::{fixture, rstest};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct StagingFixture {
        _temp_dir: tempfile::TempDir,
        staging_path: Utf8PathBuf,
        toolchain: &'static str,
    }

    struct TestBaseDirs {
        data_dir: Option<PathBuf>,
    }

    impl BaseDirs for TestBaseDirs {
        fn home_dir(&self) -> Option<PathBuf> {
            None
        }
        fn bin_dir(&self) -> Option<PathBuf> {
            None
        }
        fn whitaker_data_dir(&self) -> Option<PathBuf> {
            self.data_dir.clone()
        }
    }

    static PRUNE_HOOK_CALLED: AtomicBool = AtomicBool::new(false);

    fn stub_detect_host_target() -> Result<String> {
        Ok("x86_64-unknown-linux-gnu".to_owned())
    }

    fn stub_resolve_destination_dir(
        _dirs: &dyn BaseDirs,
        _toolchain_channel: &str,
        _host_target: &str,
    ) -> Result<Utf8PathBuf> {
        Ok(Utf8PathBuf::from("/tmp/whitaker-test-data/lints"))
    }

    fn stub_attempt_prebuilt(
        _config: &PrebuiltConfig<'_>,
        _stderr: &mut dyn Write,
    ) -> PrebuiltResult {
        PrebuiltResult::Success {
            staging_path: Utf8PathBuf::from("/tmp/whitaker-test-staging"),
        }
    }

    fn stub_prune_prebuilt_libraries(
        _staging_path: &Utf8Path,
        _toolchain_channel: &str,
        _requested_crates: &[CrateName],
    ) -> Result<()> {
        PRUNE_HOOK_CALLED.store(true, Ordering::SeqCst);
        Err(InstallerError::StagingFailed {
            reason: "forced prune failure".to_owned(),
        })
    }

    #[fixture]
    fn staging_fixture() -> StagingFixture {
        let temp_dir = tempfile::tempdir().expect("tempdir should be available");
        let staging_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .expect("tempdir path should be utf-8");
        fs::create_dir_all(staging_path.as_std_path()).expect("staging path should be creatable");
        StagingFixture {
            _temp_dir: temp_dir,
            staging_path,
            toolchain: "nightly-2025-09-18",
        }
    }

    fn create_staged_library(
        staging_path: &Utf8Path,
        crate_name: &str,
        toolchain: &str,
    ) -> Utf8PathBuf {
        let library_path = staging_path.join(staged_library_filename(crate_name, toolchain));
        fs::write(library_path.as_std_path(), b"fake prebuilt library")
            .expect("test setup should write staged library");
        library_path
    }

    #[rstest]
    #[case::suite_only(
        &[SUITE_CRATE],
        &[SUITE_CRATE],
        &["module_max_lines", "no_expect_outside_tests"]
    )]
    #[case::default_suite(
        &[],
        &[SUITE_CRATE],
        &["module_max_lines", "no_expect_outside_tests"]
    )]
    #[case::individual_only(
        &["module_max_lines"],
        &["module_max_lines"],
        &[SUITE_CRATE, "no_expect_outside_tests"]
    )]
    fn prune_prebuilt_libraries_keeps_only_requested_crates(
        staging_fixture: StagingFixture,
        #[case] requested: &[&str],
        #[case] retained: &[&str],
        #[case] removed: &[&str],
    ) {
        let StagingFixture {
            _temp_dir: _,
            staging_path,
            toolchain,
        } = staging_fixture;

        let foreign_path = staging_path.join("libforeign_lint@nightly-2025-09-18.so");
        fs::write(foreign_path.as_std_path(), b"foreign library")
            .expect("test setup should write foreign library");

        let mut staged = Vec::new();
        for crate_name in retained.iter().chain(removed.iter()) {
            let path = create_staged_library(&staging_path, crate_name, toolchain);
            staged.push(((*crate_name).to_owned(), path));
        }

        let requested_crates: Vec<CrateName> = requested
            .iter()
            .map(|name| CrateName::from(*name))
            .collect();
        if requested.is_empty() {
            let default_requested = requested_crate_names(&requested_crates);
            assert_eq!(
                default_requested,
                HashSet::from([SUITE_CRATE]),
                "empty requested-crate list should default to suite crate"
            );
        }
        prune_prebuilt_libraries(&staging_path, toolchain, &requested_crates)
            .expect("pruning should succeed");

        for crate_name in retained {
            let path = staged
                .iter()
                .find(|(name, _)| name == crate_name)
                .map(|(_, path)| path)
                .expect("retained library should have been staged");
            assert!(path.exists(), "{crate_name} should remain");
        }

        for crate_name in removed {
            let path = staged
                .iter()
                .find(|(name, _)| name == crate_name)
                .map(|(_, path)| path)
                .expect("removed library should have been staged");
            assert!(!path.exists(), "{crate_name} should be removed");
        }

        assert!(
            foreign_path.exists(),
            "non-whitaker libraries should remain untouched"
        );
    }

    #[test]
    fn try_prebuilt_installation_prune_error_falls_back_to_local_build() {
        let dirs = TestBaseDirs {
            data_dir: Some(PathBuf::from("/tmp/whitaker-test-data")),
        };

        let args = InstallArgs::default();
        let requested_crates = vec![CrateName::from(SUITE_CRATE)];
        let context = PrebuiltInstallationContext {
            args: &args,
            dirs: &dirs,
            requested_crates: &requested_crates,
            toolchain_channel: "nightly-2025-09-18",
        };

        let mut stderr = Vec::new();
        PRUNE_HOOK_CALLED.store(false, Ordering::SeqCst);
        let result = try_prebuilt_installation_with(
            &context,
            &mut stderr,
            PrebuiltInstallationHooks {
                detect_host_target: stub_detect_host_target,
                resolve_destination_dir: stub_resolve_destination_dir,
                attempt_prebuilt: stub_attempt_prebuilt,
                prune_prebuilt_libraries: stub_prune_prebuilt_libraries,
            },
        );

        assert!(
            matches!(result, Ok(None)),
            "prune failure should trigger fallback to local compilation"
        );
        assert!(
            PRUNE_HOOK_CALLED.load(Ordering::SeqCst),
            "prune hook should be invoked"
        );
        let stderr = String::from_utf8(stderr).expect("stderr should be utf-8");
        assert!(
            stderr.contains("Prebuilt download unavailable: staging failed: forced prune failure"),
            "fallback reason should include prune error, stderr: {stderr}"
        );
        assert!(
            stderr.contains("Falling back to local compilation."),
            "fallback message should be emitted, stderr: {stderr}"
        );
    }
}
