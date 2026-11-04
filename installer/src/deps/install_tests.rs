//! Tests for dependency-install status refresh behaviour.

use rstest::rstest;

use super::*;
use crate::test_utils::{
    dependency_binary_helpers::{dylint_link_install_list_check, with_fake_binary_on_path},
    failure_output,
    stdout_output,
    success_output,
};

#[rstest]
#[case(InstallOutcome::CargoBinstall)]
#[case(InstallOutcome::CargoInstall)]
fn update_status_after_install_refreshes_link_for_local_cargo_dylint_installs(
    #[case] outcome: InstallOutcome,
) {
    with_fake_binary_on_path("dylint-link", || {
        // The companion refresh confirms the dylint-link version against
        // Cargo's installed-binary registry after the PATH probe passes.
        let executor = crate::test_utils::StubExecutor::new(vec![dylint_link_install_list_check()]);
        let mut status = DylintToolStatus {
            cargo_dylint: false,
            dylint_link: false,
        };

        update_status_after_install(&mut status, &executor, &CARGO_DYLINT_TOOL, outcome);

        assert!(status.cargo_dylint);
        assert!(status.dylint_link);
        executor.assert_finished();
    })
    .expect("prepare fake PATH");
}

#[test]
fn update_status_after_install_skips_link_probe_for_repository_release() {
    let executor = crate::test_utils::StubExecutor::new(vec![]);
    let mut status = DylintToolStatus {
        cargo_dylint: false,
        dylint_link: false,
    };

    update_status_after_install(
        &mut status,
        &executor,
        &CARGO_DYLINT_TOOL,
        InstallOutcome::RepositoryRelease,
    );

    assert!(status.cargo_dylint);
    assert!(!status.dylint_link);
    executor.assert_finished();
}

#[rstest]
#[case(false, false, &CARGO_DYLINT_TOOL, true)]
#[case(true, false, &CARGO_DYLINT_TOOL, false)]
#[case(false, false, &DYLINT_LINK_TOOL, true)]
#[case(false, true, &DYLINT_LINK_TOOL, false)]
fn should_install_tool_returns_expected(
    #[case] cargo_dylint: bool,
    #[case] dylint_link: bool,
    #[case] tool: &DependencyTool,
    #[case] expected: bool,
) {
    let status = DylintToolStatus {
        cargo_dylint,
        dylint_link,
    };

    assert_eq!(should_install_tool(status, tool), expected);
}

#[rstest]
#[case::non_repo_release_and_link_missing(
    InstallOutcome::CargoInstall,
    DylintToolStatus { cargo_dylint: false, dylint_link: false },
    true,
)]
#[case::repository_release(
    InstallOutcome::RepositoryRelease,
    DylintToolStatus { cargo_dylint: false, dylint_link: false },
    false,
)]
#[case::link_already_present(
    InstallOutcome::CargoBinstall,
    DylintToolStatus { cargo_dylint: true, dylint_link: true },
    false,
)]
fn should_refresh_companions_returns_expected(
    #[case] outcome: InstallOutcome,
    #[case] status: DylintToolStatus,
    #[case] expected: bool,
) {
    assert_eq!(should_refresh_companions(outcome, status), expected);
}

/// Cargo's refusal to build a dependency whose `rust-version` exceeds the
/// active toolchain, as `cargo install --locked` reports it.
const MSRV_REFUSAL: &str = concat!(
    "error: failed to select a version for the requirement `cargo-util = \"^0.2.32\"`\n",
    "candidate versions found which didn't match: 0.2.31, 0.2.30, 0.2.29\n",
    "package `cargo-util v0.2.32` cannot be built because it requires rustc",
    " 1.97 or newer, while the currently active rustc version is 1.96.0\n",
    "Either upgrade to rustc 1.97 or newer, or use\n",
    "cargo update cargo-util@0.2.32 --precise ver\n",
    "where `ver` is the latest version of `cargo-util` supporting rustc 1.96.0",
);

/// The advice appended to a version refusal has two jobs: keep Cargo's own
/// diagnosis, and offer a command the user can act on.
///
/// Each case names one fragment the message must carry and why it matters.
/// They share a single call because the fragment is the only thing that
/// varies, and a failure then reports which promise broke rather than which
/// assertion line did.
#[rstest]
#[case::keeps_crate_and_versions(
    "cargo-util v0.2.32",
    "Cargo's original diagnostic should survive, because it names the crate that failed"
)]
#[case::keeps_required_rustc(
    "requires rustc 1.97",
    "the required rustc version should survive, because the advice rests on it"
)]
#[case::pinned_install_command(
    "cargo +stable install --locked --version 6.3.0 cargo-dylint",
    "the advice should carry the full pinned install command so it stays copy-pasteable"
)]
#[case::skip_deps_escape_hatch(
    "--skip-deps",
    "the advice should name the escape hatch for a manually installed tool"
)]
fn source_install_failure_message_advises_on_a_version_refusal(
    #[case] fragment: &str,
    #[case] why: &str,
) {
    let message = source_install_failure_message(
        &failure_output(MSRV_REFUSAL),
        &CARGO_DYLINT_TOOL,
        Some("6.3.0"),
    );

    assert!(message.contains(fragment), "{why}; got: {message}");
}

/// The advice deliberately does not offer `--toolchain`: that flag is resolved
/// after dependencies are installed, so naming it would send the user round a
/// loop that fails the same way.
///
/// This is the one promise about *absence*, which the table above cannot
/// express.
#[test]
fn source_install_failure_message_omits_the_toolchain_flag() {
    let message = source_install_failure_message(
        &failure_output(MSRV_REFUSAL),
        &CARGO_DYLINT_TOOL,
        Some("6.3.0"),
    );

    assert!(
        !message.contains("--toolchain"),
        "the installer's `--toolchain` flag does not reach the dependency step, so it must not be \
         offered; got: {message}"
    );
}

#[test]
fn source_install_failure_message_passes_unrelated_failures_through() {
    let message = source_install_failure_message(
        &failure_output("cargo install failed"),
        &CARGO_DYLINT_TOOL,
        Some("6.3.0"),
    );

    assert_eq!(message, "cargo install failed");
}

#[rstest]
#[case::requires_rustc("package `x v1.0.0` cannot be built because it requires rustc 1.97")]
#[case::ignore_flag("help: pass `--ignore-rust-version` to select a version")]
fn is_rust_version_refusal_matches_cargos_wording(#[case] message: &str) {
    assert!(is_rust_version_refusal(message));
}

#[rstest]
#[case::empty("")]
#[case::network("error: failed to download from registry")]
#[case::compile("error[E0308]: mismatched types")]
fn is_rust_version_refusal_rejects_other_failures(#[case] message: &str) {
    assert!(!is_rust_version_refusal(message));
}

/// The advice is only useful if it reaches the error the user sees, so this
/// drives the real `cargo install` path rather than the formatter alone.
#[test]
fn run_cargo_install_reports_toolchain_advice_for_a_version_refusal() {
    let executor = crate::test_utils::StubExecutor::new(vec![crate::test_utils::ExpectedCall {
        cmd: "cargo",
        args: vec!["install", "--locked", "--version", "6.3.0", "cargo-dylint"],
        result: Ok(failure_output(MSRV_REFUSAL)),
    }]);
    let plan = CargoInstallPlan::new(&CARGO_DYLINT_TOOL).with_version("6.3.0");
    let mut stderr = Vec::new();

    let error = run_cargo_install(&executor, plan, &mut stderr, true)
        .expect_err("a refused source build should fail");

    match error {
        InstallerError::DependencyInstall { tool, message } => {
            assert_eq!(tool, "cargo-dylint");
            assert!(
                message.contains("cargo +stable install"),
                "the surfaced error should carry the advice; got: {message}"
            );
        }
        other => panic!("unexpected error: {other}"),
    }
    assert!(
        stderr.is_empty(),
        "a failed install should not report success output"
    );
    executor.assert_finished();
}

/// A successful build must not gain the advice text, and must still report the
/// success message: the diagnostic wrapper only ever decorates a failure.
#[test]
fn run_cargo_install_succeeds_without_advice_when_cargo_succeeds() {
    with_fake_binary_on_path("dylint-link", || {
        let executor = crate::test_utils::StubExecutor::new(vec![
            crate::test_utils::ExpectedCall {
                cmd: "cargo",
                args: vec!["install", "--locked", "--version", "6.3.0", "dylint-link"],
                result: Ok(success_output()),
            },
            crate::test_utils::ExpectedCall {
                cmd: "cargo",
                args: vec!["install", "--list"],
                result: Ok(stdout_output("dylint-link v6.3.0:\n")),
            },
        ]);
        let plan = CargoInstallPlan::new(&DYLINT_LINK_TOOL).with_version("6.3.0");
        let mut stderr = Vec::new();

        run_cargo_install(&executor, plan, &mut stderr, false)
            .expect("the source build should succeed");

        let output = String::from_utf8(stderr).expect("stderr should be UTF-8");
        assert!(output.contains("Installed dylint-link from source with cargo install."));
        assert!(!output.contains("rustup toolchain install"));
        executor.assert_finished();
    })
    .expect("prepare fake PATH");
}
