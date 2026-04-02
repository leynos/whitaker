//! Behaviour tests for dependency-binary installation and provenance output.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::path::PathBuf;
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
use whitaker_installer::test_utils::{
    ExpectedCall, StubDirs, StubExecutor, failure_output, success_output,
};

enum RepositoryInstallerBehaviour {
    Success,
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
        _dirs: &dyn BaseDirs,
    ) -> std::result::Result<PathBuf, DependencyBinaryInstallError> {
        match &self.behaviour {
            RepositoryInstallerBehaviour::Success => Ok(PathBuf::from(format!(
                "/tmp/bin/{}-{}",
                dependency.package(),
                target
            ))),
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
    repository_verification_fails: bool,
    binstall_available: bool,
    cargo_binstall_failure: Option<String>,
    cargo_install_failure: Option<String>,
    unsupported_target: bool,
    stderr: Vec<u8>,
    install_result: Option<std::result::Result<(), whitaker_installer::error::InstallerError>>,
    provenance: Option<String>,
    dependencies: Vec<DependencyBinary>,
}

struct ExpectedCallConfig<'a> {
    binstall_available: bool,
    verify_repository_install: bool,
    verification_fails: bool,
    cargo_binstall_failure: Option<&'a str>,
    cargo_install_failure: Option<&'a str>,
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
    world.repository_behaviour = Some(RepositoryInstallerBehaviour::Failure(message));
}

#[given("the repository installer succeeds but verification fails")]
fn given_repository_verification_failure(world: &mut DependencyBinaryWorld) {
    world.repository_behaviour = Some(RepositoryInstallerBehaviour::Success);
    world.repository_verification_fails = true;
}

#[given("cargo binstall is available")]
fn given_binstall_available(world: &mut DependencyBinaryWorld) {
    world.binstall_available = true;
}

#[given("cargo binstall is unavailable")]
fn given_binstall_unavailable(world: &mut DependencyBinaryWorld) {
    world.binstall_available = false;
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
    world.unsupported_target = true;
}

#[given("the dependency manifest is loaded")]
fn given_manifest_loaded(world: &mut DependencyBinaryWorld) {
    world.dependencies = required_dependency_binaries()
        .expect("dependency manifest should load")
        .to_vec();
}

#[when("dependency installation runs")]
fn when_dependency_installation_runs(world: &mut DependencyBinaryWorld) {
    let tool = world
        .missing_tool
        .clone()
        .expect("missing tool should be configured");
    let expect_repository_verification = matches!(
        world.repository_behaviour,
        Some(RepositoryInstallerBehaviour::Success)
    );
    let repository_installer = StubRepositoryInstaller {
        behaviour: world.repository_behaviour.take().unwrap_or(
            RepositoryInstallerBehaviour::Failure("missing repository".to_owned()),
        ),
    };
    let status = DylintToolStatus {
        cargo_dylint: tool != "cargo-dylint",
        dylint_link: tool != "dylint-link",
    };

    let executor = StubExecutor::new(expected_calls(
        &tool,
        ExpectedCallConfig {
            binstall_available: world.binstall_available,
            verify_repository_install: expect_repository_verification,
            verification_fails: world.repository_verification_fails,
            cargo_binstall_failure: world.cargo_binstall_failure.as_deref(),
            cargo_install_failure: world.cargo_install_failure.as_deref(),
        },
    ));

    let target = if world.unsupported_target {
        None
    } else {
        Some(TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target"))
    };
    let dirs = StubDirs {
        bin_dir: Some(PathBuf::from("/tmp/bin")),
    };
    world.install_result = Some(install_dylint_tools_with_options(
        &executor,
        &status,
        &mut world.stderr,
        DependencyInstallOptions {
            dirs: &dirs,
            repository_installer: &repository_installer,
            target,
            quiet: false,
        },
    ));
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

fn binstall_version_check(binstall_available: bool) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["binstall", "--version"],
        result: if binstall_available {
            Ok(success_output())
        } else {
            Ok(failure_output("missing binstall"))
        },
    }
}

fn repository_verification_call(tool: &str, verification_fails: bool) -> ExpectedCall {
    let result = if verification_fails {
        Ok(failure_output("still missing"))
    } else {
        Ok(success_output())
    };
    match tool {
        "cargo-dylint" => ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result,
        },
        "dylint-link" => ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result,
        },
        other => panic!("unexpected tool: {other}"),
    }
}

fn cargo_fallback_calls(
    tool: &str,
    binstall_available: bool,
    cargo_binstall_failure: Option<&str>,
    cargo_install_failure: Option<&str>,
) -> Vec<ExpectedCall> {
    // Intentional leak in tests to extend lifetime for static string usage;
    // acceptable here as it will not be freed.
    let tool_static: &'static str = Box::leak(tool.to_owned().into_boxed_str());

    // Determine which cargo command to run and its failure mode
    let (use_binstall, failure_message) = if binstall_available {
        (true, cargo_binstall_failure)
    } else {
        (false, cargo_install_failure)
    };

    let install_call = ExpectedCall {
        cmd: "cargo",
        args: if use_binstall {
            vec!["binstall", "-y", tool_static]
        } else {
            vec!["install", tool_static]
        },
        result: Ok(match failure_message {
            Some(message) => failure_output(message),
            None => success_output(),
        }),
    };
    let mut calls = vec![install_call];

    // If the primary install command succeeds, add verification call
    if failure_message.is_none() {
        calls.push(match tool {
            "cargo-dylint" => cargo_dylint_check(),
            "dylint-link" => dylint_link_check(),
            other => panic!("unexpected tool: {other}"),
        });
    } else if use_binstall && cargo_install_failure.is_none() {
        // binstall failed but cargo install should succeed
        calls.push(ExpectedCall {
            cmd: "cargo",
            args: vec!["install", tool_static],
            result: Ok(success_output()),
        });
        calls.push(match tool {
            "cargo-dylint" => cargo_dylint_check(),
            "dylint-link" => dylint_link_check(),
            other => panic!("unexpected tool: {other}"),
        });
    } else if use_binstall {
        // binstall failed and cargo install also fails
        if let Some(message) = cargo_install_failure {
            calls.push(ExpectedCall {
                cmd: "cargo",
                args: vec!["install", tool_static],
                result: Ok(failure_output(message)),
            });
        }
    }

    calls
}

fn expected_calls(tool: &str, config: ExpectedCallConfig<'_>) -> Vec<ExpectedCall> {
    let mut calls = vec![binstall_version_check(config.binstall_available)];

    if config.verify_repository_install {
        calls.push(repository_verification_call(
            tool,
            config.verification_fails,
        ));
        if !config.verification_fails {
            return calls;
        }
    }

    calls.extend(cargo_fallback_calls(
        tool,
        config.binstall_available,
        config.cargo_binstall_failure,
        config.cargo_install_failure,
    ));
    calls
}

fn cargo_dylint_check() -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Ok(success_output()),
    }
}

fn dylint_link_check() -> ExpectedCall {
    ExpectedCall {
        cmd: "dylint-link",
        args: vec!["--version"],
        result: Ok(success_output()),
    }
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
