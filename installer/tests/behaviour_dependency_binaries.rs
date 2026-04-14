//! Behaviour tests for dependency-binary installation and provenance output.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::path::PathBuf;
use temp_env::with_var;
use whitaker_installer::dependency_binaries::{
    DependencyBinary, DependencyBinaryInstallError, DependencyBinaryInstaller,
    required_dependency_binaries,
};
use whitaker_installer::dependency_packaging::render_provenance_markdown;
use whitaker_installer::deps::{
    DependencyInstallOptions, DylintToolStatus, install_dylint_tools_with_options,
};
use whitaker_installer::dirs::BaseDirs;
use whitaker_installer::installer_packaging::TargetTriple;
use whitaker_installer::test_support::env_test_guard;
use whitaker_installer::test_utils::{
    StubDirs, StubExecutor,
    dependency_binary_helpers::{ExpectedCallConfig, expected_calls, write_fake_binary},
};

enum RepositoryInstallerBehaviour {
    Success,
    NotFound,
    Failure(String),
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
            RepositoryInstallerBehaviour::Success => dirs.bin_dir().map_or_else(
                || Err(DependencyBinaryInstallError::MissingBinDir),
                |bin_dir| Ok(bin_dir.join(format!("{}-{}", dependency.package(), target))),
            ),
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
    should_repository_verification_fail: bool,
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
    world.repository_behaviour = Some(RepositoryInstallerBehaviour::Success);
    world.should_repository_verification_fail = true;
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
fn given_manifest_loaded(world: &mut DependencyBinaryWorld) {
    world.dependencies = required_dependency_binaries()
        .expect("dependency manifest should load")
        .to_vec();
}

fn build_stub_executor(world: &DependencyBinaryWorld, tool: &str) -> StubExecutor {
    let is_repository_asset_missing = matches!(
        world.repository_behaviour,
        Some(RepositoryInstallerBehaviour::NotFound)
    );
    let expect_repository_verification = matches!(
        world.repository_behaviour,
        Some(RepositoryInstallerBehaviour::Success)
    ) && !world.is_unsupported_target;
    StubExecutor::new(expected_calls(
        tool,
        ExpectedCallConfig {
            is_binstall_available: world.is_binstall_available,
            has_repository_context: !world.is_unsupported_target,
            is_repository_asset_missing,
            should_verify_repository_install: expect_repository_verification,
            is_repository_verification_failing: world.should_repository_verification_fail,
            cargo_binstall_failure: world.cargo_binstall_failure.as_deref(),
            cargo_install_failure: world.cargo_install_failure.as_deref(),
        },
    ))
}

#[when("dependency installation runs")]
fn when_dependency_installation_runs(world: &mut DependencyBinaryWorld) {
    let tool = world
        .missing_tool
        .clone()
        .expect("missing tool should be configured");
    let executor = build_stub_executor(world, &tool);
    let repository_installer = StubRepositoryInstaller {
        behaviour: world.repository_behaviour.take().unwrap_or(
            RepositoryInstallerBehaviour::Failure("missing repository".to_owned()),
        ),
    };
    let status = DylintToolStatus {
        cargo_dylint: tool != "cargo-dylint",
        dylint_link: tool != "dylint-link",
    };

    let target = if world.is_unsupported_target {
        None
    } else {
        Some(TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target"))
    };
    let bin_dir_temp = tempfile::tempdir().expect("bin dir tempdir should be created");
    let bin_dir = bin_dir_temp.path().to_path_buf();
    let dirs = StubDirs {
        bin_dir: Some(bin_dir.clone()),
    };
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
    world.install_result = Some(
        if tool == "dylint-link" && !world.expect_missing_dylint_link {
            let _guard = env_test_guard();
            #[cfg(windows)]
            let dylint_link_path = bin_dir.join("dylint-link.cmd");
            #[cfg(not(windows))]
            let dylint_link_path = bin_dir.join("dylint-link");
            write_fake_binary(&dylint_link_path, true);
            with_var("PATH", Some(&bin_dir), run_install)
        } else {
            run_install()
        },
    );
    executor.assert_finished();
}

#[when("provenance markdown is rendered")]
fn when_provenance_markdown_rendered(world: &mut DependencyBinaryWorld) {
    world.provenance = Some(render_provenance_markdown(&world.dependencies));
}

#[then("the install succeeds")]
fn then_install_succeeds(world: &mut DependencyBinaryWorld) {
    let result = world
        .install_result
        .as_ref()
        .expect("install result should exist");
    assert!(result.is_ok(), "expected success, got {result:?}");
}

#[then("stderr contains \"{expected}\"")]
fn then_stderr_contains(world: &mut DependencyBinaryWorld, expected: String) {
    let stderr = String::from_utf8(world.stderr.clone()).expect("stderr should be UTF-8");
    assert!(
        stderr.contains(&expected),
        "expected stderr to contain {expected:?}, got {stderr:?}"
    );
}

#[then("the install fails for \"{tool}\" with message containing \"{expected}\"")]
fn then_install_fails_with_message(
    world: &mut DependencyBinaryWorld,
    tool: String,
    expected: String,
) {
    let result = world
        .install_result
        .as_ref()
        .expect("install result should exist");
    match result {
        Err(whitaker_installer::error::InstallerError::DependencyInstall {
            tool: actual_tool,
            message,
        }) => {
            assert_eq!(actual_tool, &tool);
            assert!(
                message.contains(&expected),
                "expected error message to contain {expected:?}, got {message:?}"
            );
        }
        other => panic!("expected dependency install error, got {other:?}"),
    }
}

#[then("the provenance contains \"{expected}\"")]
fn then_provenance_contains(world: &mut DependencyBinaryWorld, expected: String) {
    let provenance = world
        .provenance
        .as_ref()
        .expect("provenance should have been rendered");
    assert!(
        provenance.contains(&expected),
        "expected provenance to contain {expected:?}, got {provenance:?}"
    );
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 0)]
fn scenario_install_cargo_dylint_from_repository(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 1)]
fn scenario_install_dylint_link_from_repository(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 2)]
fn scenario_repository_falls_back_to_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 3)]
fn scenario_repository_and_binstall_and_cargo_all_fail(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 4)]
fn scenario_repository_and_binstall_fall_back_to_cargo_install(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 5)]
fn scenario_repository_and_binstall_fall_back_to_failed_cargo_install(
    world: DependencyBinaryWorld,
) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 6)]
fn scenario_repository_verification_failure_uses_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 7)]
fn scenario_unsupported_target_uses_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 8)]
fn scenario_repository_success_without_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 9)]
fn scenario_provenance_lists_both_dependencies(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 10)]
fn scenario_dylint_link_missing_after_install_fails(world: DependencyBinaryWorld) {
    let _ = world;
}
