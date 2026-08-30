//! Behaviour tests for dependency-binary installation and provenance output.

use std::path::{Path, PathBuf};

use rstest::fixture;
use rstest_bdd_macros::{given, then, when};

#[path = "behaviour_dependency_binaries/scenarios.rs"]
mod scenarios;
use temp_env::with_var;
use whitaker_installer::{
    dependency_binaries::{
        DependencyBinary, DependencyBinaryInstallError, DependencyBinaryInstaller,
        required_dependency_binaries,
    },
    dependency_packaging::render_provenance_markdown,
    deps::{DependencyInstallOptions, DylintToolStatus, install_dylint_tools_with_options},
    dirs::BaseDirs,
    installer_packaging::TargetTriple,
    test_support::env_test_guard,
    test_utils::{
        StubDirs, StubExecutor,
        dependency_binary_helpers::{
            ExpectedCallConfig, RepositoryVerification, expected_calls, path_binary_location,
            write_fake_binary,
        },
    },
};

/// Outcome the stubbed repository installer should simulate.
enum RepositoryInstallerBehaviour {
    /// Installation succeeds and the staged binary verifies.
    Success,
    /// Installation succeeds but verification of the staged binary fails.
    SuccessWithFailedVerification,
    /// The release asset is absent from the repository.
    NotFound,
    /// Installation fails with the given message.
    Failure(String),
}

impl RepositoryInstallerBehaviour {
    /// Whether the stub stages a runnable binary for this behaviour.
    const fn installs_binary(&self) -> bool {
        matches!(*self, Self::Success | Self::SuccessWithFailedVerification)
    }
}

struct StubRepositoryInstaller {
    behaviour: RepositoryInstallerBehaviour,
}

impl DependencyBinaryInstaller for StubRepositoryInstaller {
    fn install(
        &self,
        dependency: &DependencyBinary,
        target: &TargetTriple,
        dirs: &dyn BaseDirs,
    ) -> std::result::Result<PathBuf, DependencyBinaryInstallError> {
        match &self.behaviour {
            RepositoryInstallerBehaviour::Success
            | RepositoryInstallerBehaviour::SuccessWithFailedVerification => {
                dirs.bin_dir().map_or_else(
                    || Err(DependencyBinaryInstallError::MissingBinDir),
                    |bin_dir| {
                        // Stage a runnable fake at the returned path: dylint-link
                        // verification probes the extracted binary directly. The
                        // platform suffix keeps the fake executable on Windows.
                        let installed_path = path_binary_location(
                            &bin_dir,
                            &format!("{}-{}", dependency.package(), target),
                        );
                        write_fake_binary(&installed_path, true);
                        Ok(installed_path)
                    },
                )
            }
            RepositoryInstallerBehaviour::NotFound => Err(DependencyBinaryInstallError::NotFound {
                url: format!(
                    "{}/releases/download/v{}/{}",
                    dependency.repository(),
                    dependency.version(),
                    dependency.package()
                ),
            }),
            RepositoryInstallerBehaviour::Failure(message) => {
                Err(DependencyBinaryInstallError::Install {
                    binary: dependency.binary().to_owned(),
                    reason: message.clone(),
                })
            }
        }
    }
}

#[derive(Default)]
struct DependencyBinaryWorld {
    missing_tool: Option<String>,
    repository_behaviour: Option<RepositoryInstallerBehaviour>,
    expect_missing_dylint_link: bool,
    is_binstall_available: bool,
    cargo_binstall_failure: Option<String>,
    cargo_install_failure: Option<String>,
    is_unsupported_target: bool,
    stderr: Vec<u8>,
    install_result: Option<std::result::Result<(), whitaker_installer::error::InstallerError>>,
    provenance: Option<String>,
    dependencies: Vec<DependencyBinary>,
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> DependencyBinaryWorld {
    DependencyBinaryWorld::default()
}

#[given("the missing tool is \"{tool}\"")]
fn given_missing_tool(world: &mut DependencyBinaryWorld, tool: String) {
    world.missing_tool = Some(tool);
}

#[given("the repository installer succeeds")]
fn given_repository_success(world: &mut DependencyBinaryWorld) {
    world.repository_behaviour = Some(RepositoryInstallerBehaviour::Success);
}

#[given("the repository installer fails with \"{message}\"")]
fn given_repository_failure(world: &mut DependencyBinaryWorld, message: String) {
    world.repository_behaviour = Some(if message == "not found" {
        RepositoryInstallerBehaviour::NotFound
    } else {
        RepositoryInstallerBehaviour::Failure(message)
    });
}

#[given("the repository installer succeeds but verification fails")]
fn given_repository_verification_failure(world: &mut DependencyBinaryWorld) {
    world.repository_behaviour = Some(RepositoryInstallerBehaviour::SuccessWithFailedVerification);
}

#[given("dylint-link is missing from PATH after installation")]
fn given_missing_dylint_link_on_path(world: &mut DependencyBinaryWorld) {
    world.expect_missing_dylint_link = true;
}

#[given("cargo binstall is available")]
fn given_binstall_available(world: &mut DependencyBinaryWorld) {
    world.is_binstall_available = true;
}

#[given("cargo binstall is unavailable")]
fn given_binstall_unavailable(world: &mut DependencyBinaryWorld) {
    world.is_binstall_available = false;
}

#[given("cargo binstall fails with \"{message}\"")]
fn given_cargo_binstall_failure(world: &mut DependencyBinaryWorld, message: String) {
    world.cargo_binstall_failure = Some(message);
}

#[given("cargo install fails with \"{message}\"")]
fn given_cargo_install_failure(world: &mut DependencyBinaryWorld, message: String) {
    world.cargo_install_failure = Some(message);
}

#[given("the target is unsupported")]
fn given_unsupported_target(world: &mut DependencyBinaryWorld) {
    world.is_unsupported_target = true;
}

#[given("the dependency manifest is loaded")]
fn given_manifest_loaded(world: &mut DependencyBinaryWorld) -> Result<(), String> {
    world.dependencies = required_dependency_binaries()
        .map_err(|error| format!("dependency manifest should load: {error}"))?
        .to_vec();
    Ok(())
}

fn build_stub_executor(world: &DependencyBinaryWorld, tool: &str) -> Result<StubExecutor, String> {
    let is_repository_asset_missing = matches!(
        world.repository_behaviour,
        Some(RepositoryInstallerBehaviour::NotFound)
    );
    let expect_repository_verification = world
        .repository_behaviour
        .as_ref()
        .is_some_and(RepositoryInstallerBehaviour::installs_binary)
        && !world.is_unsupported_target;
    let should_verification_fail = matches!(
        world.repository_behaviour,
        Some(RepositoryInstallerBehaviour::SuccessWithFailedVerification)
    );
    let repository_verification = match (expect_repository_verification, should_verification_fail) {
        (false, _) => RepositoryVerification::Skip,
        (true, true) => RepositoryVerification::Fails,
        (true, false) => RepositoryVerification::Succeeds,
    };
    Ok(StubExecutor::new(expected_calls(
        tool,
        &ExpectedCallConfig {
            is_binstall_available: world.is_binstall_available,
            has_repository_context: !world.is_unsupported_target,
            is_repository_asset_missing,
            repository_verification,
            cargo_binstall_failure: world.cargo_binstall_failure.as_deref(),
            cargo_install_failure: world.cargo_install_failure.as_deref(),
        },
    )?))
}

/// Stages a runnable `dylint-link` fake in the executables directory so PATH
/// lookups succeed.
fn stage_dylint_link_on_path(bin_dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let dylint_link_path = bin_dir.join("dylint-link.cmd");
    #[cfg(not(windows))]
    let dylint_link_path = bin_dir.join("dylint-link");
    write_fake_binary(&dylint_link_path, true);
    Ok(())
}

fn run_install_with_dylint_link_on_path(
    bin_dir: &Path,
    run_install: impl FnOnce() -> std::result::Result<(), whitaker_installer::error::InstallerError>,
) -> std::result::Result<(), whitaker_installer::error::InstallerError> {
    let _guard = env_test_guard();
    with_var("PATH", Some(bin_dir), run_install)
}

#[when("dependency installation runs")]
fn when_dependency_installation_runs(world: &mut DependencyBinaryWorld) -> Result<(), String> {
    let tool = world
        .missing_tool
        .clone()
        .ok_or_else(|| String::from("missing tool should be configured"))?;
    let executor = build_stub_executor(world, &tool)?;
    let repository_installer = StubRepositoryInstaller {
        behaviour: world.repository_behaviour.take().unwrap_or_else(|| {
            RepositoryInstallerBehaviour::Failure("missing repository".to_owned())
        }),
    };
    let status = DylintToolStatus {
        cargo_dylint: tool != "cargo-dylint",
        dylint_link: tool != "dylint-link",
    };

    let target = if world.is_unsupported_target {
        None
    } else {
        Some(
            TargetTriple::try_from("x86_64-unknown-linux-gnu")
                .map_err(|error| format!("valid target: {error}"))?,
        )
    };
    let bin_dir_temp = tempfile::tempdir()
        .map_err(|error| format!("bin dir tempdir should be created: {error}"))?;
    let bin_dir = bin_dir_temp.path().to_path_buf();
    let dirs = StubDirs {
        bin_dir: Some(bin_dir.clone()),
    };
    let is_dylint_link = tool == "dylint-link";
    if is_dylint_link && !world.expect_missing_dylint_link {
        stage_dylint_link_on_path(&bin_dir)?;
    }
    let run_install = || {
        install_dylint_tools_with_options(
            &executor,
            &status,
            &mut world.stderr,
            DependencyInstallOptions {
                dirs: &dirs,
                repository_installer: &repository_installer,
                target,
                quiet: false,
            },
        )
    };
    world.install_result = Some(if is_dylint_link {
        run_install_with_dylint_link_on_path(&bin_dir, run_install)
    } else {
        run_install()
    });
    executor.assert_finished();
    Ok(())
}

#[when("provenance markdown is rendered")]
fn when_provenance_markdown_rendered(world: &mut DependencyBinaryWorld) {
    world.provenance = Some(render_provenance_markdown(&world.dependencies));
}

/// Borrows the recorded install outcome, failing when the When step has not run.
fn install_result(
    world: &DependencyBinaryWorld,
) -> Result<&std::result::Result<(), whitaker_installer::error::InstallerError>, String> {
    world
        .install_result
        .as_ref()
        .ok_or_else(|| String::from("install result should exist"))
}

#[then("the install succeeds")]
fn then_install_succeeds(world: &mut DependencyBinaryWorld) -> Result<(), String> {
    let result = install_result(world)?;
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(format!("expected success, got {error:?}")),
    }
}

#[then("stderr contains \"{expected}\"")]
fn then_stderr_contains(world: &mut DependencyBinaryWorld, expected: String) -> Result<(), String> {
    let stderr = String::from_utf8(world.stderr.clone())
        .map_err(|error| format!("stderr should be UTF-8: {error}"))?;
    if stderr.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "expected stderr to contain {expected:?}, got {stderr:?}"
        ))
    }
}

#[then("the install fails for \"{tool}\" with message containing \"{expected}\"")]
fn then_install_fails_with_message(
    world: &mut DependencyBinaryWorld,
    tool: String,
    expected: String,
) -> Result<(), String> {
    let result = install_result(world)?;
    let Err(whitaker_installer::error::InstallerError::DependencyInstall {
        tool: actual_tool,
        message,
    }) = result
    else {
        return Err(format!("expected dependency install error, got {result:?}"));
    };
    if actual_tool != &tool {
        return Err(format!(
            "expected failure for {tool:?}, got {actual_tool:?}"
        ));
    }
    if message.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "expected error message to contain {expected:?}, got {message:?}"
        ))
    }
}

#[then("the provenance contains \"{expected}\"")]
fn then_provenance_contains(
    world: &mut DependencyBinaryWorld,
    expected: String,
) -> Result<(), String> {
    let provenance = world
        .provenance
        .as_ref()
        .ok_or_else(|| String::from("provenance should have been rendered"))?;
    if provenance.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "expected provenance to contain {expected:?}, got {provenance:?}"
        ))
    }
}
