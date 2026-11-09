//! Tests for the prebuilt-failure fallback and source-policy decisions.
//!
//! Every prebuilt failure has four call sites, and only the download's was
//! covered by the parent module's tests. These assert the policy decision at
//! each one, so a site that stopped consulting the policy cannot pass on the
//! strength of its neighbours.

use super::*;

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
        &PrebuiltInstallationHooks {
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
    let stderr_text = String::from_utf8(stderr).expect("stderr should be utf-8");
    assert!(
        stderr_text.contains("Prebuilt download unavailable: staging failed: forced prune failure"),
        "fallback reason should include prune error, stderr: {stderr_text}"
    );
    assert!(
        stderr_text.contains("Falling back to local compilation."),
        "fallback message should be emitted, stderr: {stderr_text}"
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
        &PrebuiltInstallationHooks {
            detect_host_target: stub_detect_host_target,
            resolve_destination_dir: stub_resolve_destination_dir,
            attempt_prebuilt: stub_attempt_prebuilt_unavailable,
            prune_prebuilt_libraries: stub_prune_prebuilt_libraries,
        },
    );
    let stderr_text = String::from_utf8(stderr).expect("stderr should be utf-8");
    (result, stderr_text)
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

    let (result, stderr_text) = run_with_unavailable_prebuilt(&args);

    let error = result.expect_err("an absent artefact must fail when a source build is forbidden");
    assert!(
        stderr_text.contains("Prebuilt download unavailable"),
        "the artefact's absence is worth reporting either way, stderr: {stderr_text}"
    );
    assert!(
        !stderr_text.contains("Falling back to local compilation."),
        "promising a fallback the policy forbids says the opposite of what happens next, stderr: \
         {stderr_text}"
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
    let result = try_prebuilt_installation_with(&context, &mut stderr, &hooks);
    let stderr_text = String::from_utf8(stderr).expect("stderr should be utf-8");
    (result, stderr_text)
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
        "promising a fallback the policy forbids says the opposite of what happens next, stderr: \
         {stderr}"
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
