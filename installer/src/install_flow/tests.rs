//! Unit tests for install-flow prebuilt staging and fallback behaviour.

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

/// The policy these tests assume, stated rather than inherited.
///
/// `InstallArgs::source_policy` reads `WHITAKER_NO_SOURCE_FALLBACK` from the
/// process environment, so a developer or a runner that exported it would turn
/// every fallback here into a refusal and the failures would look like defects
/// in the flow. The policy under test is a parameter, so these tests state it.
fn permissive_policy() -> SourcePolicy {
    SourcePolicy {
        quiet: false,
        no_source_fallback: false,
    }
}

/// The policy a test's own arguments ask for, without reading the environment.
///
/// This reads the `no_source_fallback` field rather than calling
/// `InstallArgs::forbids_source_fallback`, which consults
/// `WHITAKER_NO_SOURCE_FALLBACK`. A test that sets the flag gets the strict
/// policy; a test that does not gets the permissive one, whatever the
/// environment holds.
fn policy_from_flags(args: &InstallArgs) -> SourcePolicy {
    SourcePolicy {
        quiet: args.quiet,
        no_source_fallback: args.no_source_fallback,
    }
}

/// The same, with the refusal enabled.
fn strict_policy() -> SourcePolicy {
    SourcePolicy {
        quiet: false,
        no_source_fallback: true,
    }
}

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

fn stub_attempt_prebuilt(_config: &PrebuiltConfig<'_>, _stderr: &mut dyn Write) -> PrebuiltResult {
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
        toolchain: "nightly-2026-05-28",
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

    let foreign_path = staging_path.join("libforeign_lint@nightly-2026-05-28.so");
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
        policy: permissive_policy(),
        dirs: &dirs,
        requested_crates: &requested_crates,
        toolchain_channel: "nightly-2026-05-28",
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

/// Reports the artefact as unavailable, the shape a republish window produces.
///
/// It writes the same two lines the production `attempt_prebuilt` writes
/// before returning `Fallback`. The caller deliberately does not repeat them,
/// so a stub that stayed silent would make the default path look as though
/// nothing had been reported at all.
fn stub_attempt_prebuilt_unavailable(
    config: &PrebuiltConfig<'_>,
    stderr: &mut dyn Write,
) -> PrebuiltResult {
    let reason = "repository asset not found";
    write_stderr_line(stderr, format!("Prebuilt download unavailable: {reason}"));
    if config.allow_source_fallback {
        write_stderr_line(stderr, "Falling back to local compilation.");
    }
    PrebuiltResult::Fallback {
        reason: reason.to_owned(),
    }
}

fn prebuilt_context_for<'a>(
    args: &'a InstallArgs,
    dirs: &'a TestBaseDirs,
    requested_crates: &'a [CrateName],
) -> PrebuiltInstallationContext<'a> {
    PrebuiltInstallationContext {
        args,
        policy: policy_from_flags(args),
        dirs,
        requested_crates,
        toolchain_channel: "nightly-2026-05-28",
    }
}

fn run_with_unavailable_prebuilt(args: &InstallArgs) -> (Result<Option<Utf8PathBuf>>, String) {
    let dirs = TestBaseDirs {
        data_dir: Some(PathBuf::from("/tmp/whitaker-test-data")),
    };
    let requested_crates = vec![CrateName::from(SUITE_CRATE)];
    let context = prebuilt_context_for(args, &dirs, &requested_crates);
    let mut stderr = Vec::new();
    let result = try_prebuilt_installation_with(
        &context,
        &mut stderr,
        PrebuiltInstallationHooks {
            detect_host_target: stub_detect_host_target,
            resolve_destination_dir: stub_resolve_destination_dir,
            attempt_prebuilt: stub_attempt_prebuilt_unavailable,
            prune_prebuilt_libraries: stub_prune_prebuilt_libraries,
        },
    );
    let stderr = String::from_utf8(stderr).expect("stderr should be utf-8");
    (result, stderr)
}

#[test]
fn a_missing_suite_artefact_falls_back_without_the_flag() {
    // The behaviour the flag exists to change, asserted here so the pair reads
    // as a choice rather than the flag looking like the only path.
    let (result, stderr) = run_with_unavailable_prebuilt(&InstallArgs::default());

    assert!(
        matches!(result, Ok(None)),
        "an absent artefact should fall back to a local build by default"
    );
    assert!(
        stderr.contains("Falling back to local compilation."),
        "the fallback should be announced, stderr: {stderr}"
    );
}

#[test]
fn a_missing_suite_artefact_fails_under_the_flag() {
    // A local compilation succeeds, which is why this has to be an error
    // rather than a warning: the run would otherwise report success having
    // built a suite nobody pinned.
    let args = InstallArgs {
        no_source_fallback: true,
        ..InstallArgs::default()
    };

    let (result, _stderr) = run_with_unavailable_prebuilt(&args);

    let error = result.expect_err("an absent artefact must fail when a source build is forbidden");
    assert!(
        _stderr.contains("Prebuilt download unavailable"),
        "the artefact's absence is worth reporting either way, stderr: {_stderr}"
    );
    assert!(
        !_stderr.contains("Falling back to local compilation."),
        "promising a fallback the policy forbids says the opposite of what \
         happens next, stderr: {_stderr}"
    );
    assert!(
        matches!(error, InstallerError::SourceFallbackForbidden { .. }),
        "expected a source-fallback refusal, got: {error}"
    );
    let message = error.to_string();
    assert!(
        message.contains("repository asset not found"),
        "the refusal must carry the download's own reason, got: {message}"
    );
    assert!(
        message.contains("prebuilt lint library"),
        "the refusal must name what was missing, got: {message}"
    );
}

fn failing_detect_host_target() -> Result<String> {
    Err(InstallerError::WorkspaceNotFound {
        reason: "forced host-target failure".to_owned(),
    })
}

fn failing_resolve_destination_dir(
    _dirs: &dyn BaseDirs,
    _toolchain_channel: &str,
    _host_target: &str,
) -> Result<Utf8PathBuf> {
    Err(InstallerError::WorkspaceNotFound {
        reason: "forced destination failure".to_owned(),
    })
}

/// Run the prebuilt flow under a stated policy with one hook forced to fail.
fn run_prebuilt_flow(
    policy: SourcePolicy,
    hooks: PrebuiltInstallationHooks,
) -> (Result<Option<Utf8PathBuf>>, String) {
    let dirs = TestBaseDirs {
        data_dir: Some(PathBuf::from("/tmp/whitaker-test-data")),
    };
    let args = InstallArgs::default();
    let requested_crates = vec![CrateName::from(SUITE_CRATE)];
    let context = PrebuiltInstallationContext {
        args: &args,
        policy,
        dirs: &dirs,
        requested_crates: &requested_crates,
        toolchain_channel: "nightly-2026-05-28",
    };
    let mut stderr = Vec::new();
    let result = try_prebuilt_installation_with(&context, &mut stderr, hooks);
    let stderr = String::from_utf8(stderr).expect("stderr should be utf-8");
    (result, stderr)
}

fn hooks_with_failing_host_target() -> PrebuiltInstallationHooks {
    PrebuiltInstallationHooks {
        detect_host_target: failing_detect_host_target,
        resolve_destination_dir: stub_resolve_destination_dir,
        attempt_prebuilt: stub_attempt_prebuilt,
        prune_prebuilt_libraries: stub_prune_prebuilt_libraries,
    }
}

fn hooks_with_failing_destination() -> PrebuiltInstallationHooks {
    PrebuiltInstallationHooks {
        detect_host_target: stub_detect_host_target,
        resolve_destination_dir: failing_resolve_destination_dir,
        attempt_prebuilt: stub_attempt_prebuilt,
        prune_prebuilt_libraries: stub_prune_prebuilt_libraries,
    }
}

/// The default stubs already fail at pruning, so this names that intent.
fn hooks_with_failing_prune() -> PrebuiltInstallationHooks {
    PrebuiltInstallationHooks {
        detect_host_target: stub_detect_host_target,
        resolve_destination_dir: stub_resolve_destination_dir,
        attempt_prebuilt: stub_attempt_prebuilt,
        prune_prebuilt_libraries: stub_prune_prebuilt_libraries,
    }
}

/// Every prebuilt failure refuses under the strict policy, not just the download.
///
/// `prebuilt_unavailable` has four call sites, and only the download's was
/// covered. The other three end in a local compilation by default, which is the
/// same silent success the flag exists to stop, so each has to refuse and each
/// has to name what was missing. A test per site, because a site that forgot to
/// consult the policy would still pass the others.
#[rstest]
#[case::host_target(hooks_with_failing_host_target(), "the host target")]
#[case::destination(hooks_with_failing_destination(), "the prebuilt library directory")]
#[case::pruning(hooks_with_failing_prune(), "the pruned prebuilt libraries")]
fn every_prebuilt_failure_refuses_under_the_strict_policy(
    #[case] hooks: PrebuiltInstallationHooks,
    #[case] artefact: &str,
) {
    PRUNE_HOOK_CALLED.store(false, Ordering::SeqCst);

    let (result, stderr) = run_prebuilt_flow(strict_policy(), hooks);

    let error = result.expect_err("a strict policy must refuse every prebuilt failure");
    assert!(
        matches!(error, InstallerError::SourceFallbackForbidden { .. }),
        "expected a source-fallback refusal, got: {error}"
    );
    let message = error.to_string();
    assert!(
        message.contains(artefact),
        "the refusal must name {artefact:?}, got: {message}"
    );
    assert!(
        !stderr.contains("Falling back to local compilation."),
        "promising a fallback the policy forbids says the opposite of what \
         happens next, stderr: {stderr}"
    );
}

/// The same three sites fall back by default, so the refusal is a choice.
#[rstest]
#[case::host_target(hooks_with_failing_host_target())]
#[case::destination(hooks_with_failing_destination())]
#[case::pruning(hooks_with_failing_prune())]
fn every_prebuilt_failure_falls_back_by_default(#[case] hooks: PrebuiltInstallationHooks) {
    PRUNE_HOOK_CALLED.store(false, Ordering::SeqCst);

    let (result, _stderr) = run_prebuilt_flow(permissive_policy(), hooks);

    assert!(
        matches!(result, Ok(None)),
        "the default policy should fall back to a local build"
    );
}
