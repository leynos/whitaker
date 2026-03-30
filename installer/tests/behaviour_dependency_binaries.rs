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
use whitaker_installer::test_utils::{ExpectedCall, StubExecutor, failure_output, success_output};

struct StubDirs;

impl BaseDirs for StubDirs {
    fn home_dir(&self) -> Option<PathBuf> {
        None
    }

    fn bin_dir(&self) -> Option<PathBuf> {
        Some(PathBuf::from("/tmp/bin"))
    }

    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        None
    }
}

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
    unsupported_target: bool,
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

#[given("the target is unsupported")]
fn given_unsupported_target(world: &mut DependencyBinaryWorld) {
    world.unsupported_target = true;
}

#[given("the dependency manifest is loaded")]
fn given_manifest_loaded(world: &mut DependencyBinaryWorld) {
    world.dependencies = required_dependency_binaries().to_vec();
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
        world.binstall_available,
        expect_repository_verification,
        world.repository_verification_fails,
    ));

    let target = if world.unsupported_target {
        None
    } else {
        Some(TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target"))
    };
    let dirs = StubDirs;
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

fn expected_calls(
    tool: &str,
    binstall_available: bool,
    verify_repository_install: bool,
    verification_fails: bool,
) -> Vec<ExpectedCall> {
    let mut calls = vec![ExpectedCall {
        cmd: "cargo",
        args: vec!["binstall", "--version"],
        result: if binstall_available {
            Ok(success_output())
        } else {
            Ok(failure_output("missing binstall"))
        },
    }];

    if verify_repository_install {
        calls.push(match tool {
            "cargo-dylint" => ExpectedCall {
                cmd: "cargo",
                args: vec!["dylint", "--version"],
                result: if verification_fails {
                    Ok(failure_output("still missing"))
                } else {
                    Ok(success_output())
                },
            },
            "dylint-link" => ExpectedCall {
                cmd: "dylint-link",
                args: vec!["--version"],
                result: if verification_fails {
                    Ok(failure_output("still missing"))
                } else {
                    Ok(success_output())
                },
            },
            other => panic!("unexpected tool: {other}"),
        });
        if !verification_fails {
            return calls;
        }
    }

    calls.push(ExpectedCall {
        cmd: "cargo",
        args: if binstall_available {
            vec![
                "binstall",
                "-y",
                Box::leak(tool.to_owned().into_boxed_str()),
            ]
        } else {
            vec!["install", Box::leak(tool.to_owned().into_boxed_str())]
        },
        result: Ok(success_output()),
    });
    calls
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
fn scenario_repository_and_binstall_fall_back_to_cargo_install(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 4)]
fn scenario_repository_verification_failure_uses_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 5)]
fn scenario_unsupported_target_uses_binstall(world: DependencyBinaryWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/dependency_binaries.feature", index = 6)]
fn scenario_provenance_lists_both_repositories(world: DependencyBinaryWorld) {
    let _ = world;
}
