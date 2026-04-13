//! Test helpers for dependency binary installation tests.

use crate::dependency_binaries::find_dependency_binary;
#[cfg(any(test, feature = "test-support"))]
use crate::dependency_binaries::{
    DependencyBinary, DependencyBinaryInstallError, DependencyBinaryInstaller,
};
#[cfg(any(test, feature = "test-support"))]
use crate::dirs::BaseDirs;
use crate::error::Result;
#[cfg(any(test, feature = "test-support"))]
use crate::installer_packaging::TargetTriple;
#[cfg(any(test, feature = "test-support"))]
use crate::test_support::env_test_guard;
use crate::test_utils::{ExpectedCall, failure_output, success_output};
#[cfg(any(test, feature = "test-support"))]
use std::fs;
#[cfg(all(any(test, feature = "test-support"), unix))]
use std::os::unix::fs::PermissionsExt;
#[cfg(any(test, feature = "test-support"))]
use std::path::{Path, PathBuf};
use std::process::Output;

/// Repository installer test double that always reports a missing archive.
#[cfg(any(test, feature = "test-support"))]
pub struct AlwaysNotFoundRepositoryInstaller;

#[cfg(any(test, feature = "test-support"))]
impl DependencyBinaryInstaller for AlwaysNotFoundRepositoryInstaller {
    fn install(
        &self,
        dependency: &DependencyBinary,
        target: &TargetTriple,
        _dirs: &dyn BaseDirs,
    ) -> std::result::Result<PathBuf, DependencyBinaryInstallError> {
        Err(DependencyBinaryInstallError::NotFound {
            url: format!(
                "https://example.test/{}-{}-v{}.tgz",
                dependency.package(),
                target,
                dependency.version()
            ),
        })
    }
}

/// Writes an empty fake binary at `path`.
#[cfg(any(test, feature = "test-support"))]
pub fn write_fake_binary(path: &Path, is_executable: bool) {
    fs::write(path, []).expect("write fake binary");
    #[cfg(unix)]
    {
        let mode = if is_executable { 0o755 } else { 0o644 };
        let mut permissions = fs::metadata(path)
            .expect("read fake binary metadata")
            .permissions();
        permissions.set_mode(mode);
        fs::set_permissions(path, permissions).expect("set fake binary permissions");
    }
    #[cfg(not(unix))]
    let _ = is_executable;
}

/// Runs a closure with `PATH` pointing at one or more fake directories.
#[cfg(any(test, feature = "test-support"))]
pub fn with_fake_path<T>(setup: impl FnOnce(&[PathBuf]), run: impl FnOnce() -> T) -> T {
    let _guard = env_test_guard();
    let temp_dirs = [
        tempfile::tempdir().expect("create temp dir"),
        tempfile::tempdir().expect("create temp dir"),
    ];
    let path_dirs = temp_dirs
        .iter()
        .map(|dir| dir.path().to_path_buf())
        .collect::<Vec<_>>();
    setup(&path_dirs);
    let path = std::env::join_paths(path_dirs.iter().map(PathBuf::as_path))
        .expect("join fake PATH directories");
    temp_env::with_var("PATH", Some(path), run)
}

/// Runs a closure with `PATH` containing a fake executable in the first entry.
#[cfg(any(test, feature = "test-support"))]
pub fn with_fake_binary_on_path<T>(binary_name: &str, run: impl FnOnce() -> T) -> T {
    with_fake_path(
        |directories| write_fake_binary(&directories[0].join(binary_name), true),
        run,
    )
}

/// Configuration for generating expected calls in dependency binary tests.
pub struct ExpectedCallConfig<'a> {
    /// Whether cargo-binstall is available.
    pub is_binstall_available: bool,
    /// Whether repository metadata is available to pin cargo fallbacks.
    pub has_repository_context: bool,
    /// Whether the repository failure is a missing asset.
    pub is_repository_asset_missing: bool,
    /// Whether to verify repository installation.
    pub should_verify_repository_install: bool,
    /// Whether repository verification should fail.
    pub is_repository_verification_failing: bool,
    /// Error message for cargo binstall failure (None if succeeds).
    pub cargo_binstall_failure: Option<&'a str>,
    /// Error message for cargo install failure (None if succeeds).
    pub cargo_install_failure: Option<&'a str>,
}

/// Creates an expected call for checking cargo-binstall availability.
pub fn binstall_version_check(is_binstall_available: bool) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["binstall", "--version"],
        result: if is_binstall_available {
            Ok(success_output())
        } else {
            Ok(failure_output("missing binstall"))
        },
    }
}

/// Creates an expected call for checking cargo-binstall with a fixed result.
pub fn binstall_version_check_with_result(result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["binstall", "--version"],
        result,
    }
}

/// Creates an expected call for installing a tool with cargo-binstall.
pub fn binstall_install(tool: &'static str, result: Result<Output>) -> ExpectedCall {
    let version = dependency_version(tool);
    ExpectedCall {
        cmd: "cargo",
        args: vec!["binstall", "-y", "--version", version, tool],
        result,
    }
}

/// Creates an expected call for installing a tool with cargo install.
pub fn cargo_install(tool: &'static str, result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["install", tool],
        result,
    }
}

fn cargo_source_install(
    tool: &'static str,
    version: &'static str,
    result: Result<Output>,
) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["install", "--locked", "--version", version, tool],
        result,
    }
}

fn dependency_version(tool: &str) -> &'static str {
    find_dependency_binary(tool)
        .expect("dependency manifest should parse")
        .map(|dependency| dependency.version())
        .unwrap_or_else(|| panic!("unexpected tool: {tool}"))
}

/// Creates an expected call for verifying repository installation.
pub fn repository_verification_call(tool: &str, verification_fails: bool) -> Option<ExpectedCall> {
    let result = if verification_fails {
        Ok(failure_output("still missing"))
    } else {
        Ok(success_output())
    };
    match tool {
        "cargo-dylint" => Some(ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result,
        }),
        "dylint-link" => None,
        other => panic!("unexpected tool: {other}"),
    }
}

/// Returns the expected verification call for a given tool.
fn tool_verification_check(tool: &str) -> Option<ExpectedCall> {
    match tool {
        "cargo-dylint" => Some(cargo_dylint_check()),
        "dylint-link" => None,
        other => panic!("unexpected tool: {other}"),
    }
}

/// Configuration for post-primary installation call sequence.
struct PostPrimaryConfig {
    /// The tool name.
    tool: String,
    /// Static tool name for cargo install args.
    tool_static: &'static str,
    /// Whether repository metadata is available to pin cargo fallbacks.
    has_repository_context: bool,
    /// Whether the primary installation succeeded.
    primary_succeeded: bool,
    /// Whether to use binstall (vs cargo install).
    use_binstall: bool,
    /// Error message for cargo install failure (None if succeeds).
    cargo_install_failure: Option<String>,
}

fn repo_aware_cargo_install(
    tool: &'static str,
    has_repository_context: bool,
    result: Result<Output>,
) -> ExpectedCall {
    if has_repository_context {
        cargo_source_install(tool, dependency_version(tool), result)
    } else {
        cargo_install(tool, result)
    }
}

/// Builds the sequence of calls that follow the primary install attempt.
fn post_primary_calls(cfg: &PostPrimaryConfig) -> Vec<ExpectedCall> {
    if cfg.primary_succeeded {
        return tool_verification_check(&cfg.tool).into_iter().collect();
    }
    if !cfg.use_binstall {
        return vec![];
    }
    // binstall failed: check if we should sequence a cargo-install attempt
    if cfg.cargo_install_failure.is_none() {
        // cargo install succeeds after binstall fails
        let cargo_call = repo_aware_cargo_install(
            cfg.tool_static,
            cfg.has_repository_context,
            Ok(success_output()),
        );
        let mut calls = vec![cargo_call];
        calls.extend(tool_verification_check(&cfg.tool));
        return calls;
    }
    // binstall failed and cargo install also fails
    if let Some(message) = cfg.cargo_install_failure.as_deref() {
        let cargo_call = repo_aware_cargo_install(
            cfg.tool_static,
            cfg.has_repository_context,
            Ok(failure_output(message)),
        );
        vec![cargo_call]
    } else {
        vec![]
    }
}

fn source_install_fallback_calls(
    tool: &str,
    tool_static: &'static str,
    config: &ExpectedCallConfig<'_>,
) -> Vec<ExpectedCall> {
    let version = dependency_version(tool);
    let result = config.cargo_install_failure.map_or_else(
        || Ok(success_output()),
        |message| Ok(failure_output(message)),
    );
    let install_call = cargo_source_install(tool_static, version, result);
    if config.cargo_install_failure.is_none() {
        let mut calls = vec![install_call];
        calls.extend(tool_verification_check(tool));
        calls
    } else {
        vec![install_call]
    }
}

fn binstall_args_for_tool(
    tool: &str,
    tool_static: &'static str,
    config: &ExpectedCallConfig<'_>,
) -> Vec<&'static str> {
    if config.has_repository_context {
        let version = dependency_version(tool);
        vec!["binstall", "-y", "--version", version, tool_static]
    } else {
        vec!["binstall", "-y", tool_static]
    }
}

/// Creates expected calls for cargo fallback installation (binstall or install).
pub fn cargo_fallback_calls(tool: &str, config: &ExpectedCallConfig<'_>) -> Vec<ExpectedCall> {
    // Intentional leak in tests to extend lifetime for static string usage;
    // acceptable here as it will not be freed.
    let tool_static: &'static str = Box::leak(tool.to_owned().into_boxed_str());

    if config.is_repository_asset_missing {
        return source_install_fallback_calls(tool, tool_static, config);
    }

    let (use_binstall, failure_message) = if config.is_binstall_available {
        (true, config.cargo_binstall_failure)
    } else {
        (false, config.cargo_install_failure)
    };

    let args = if use_binstall {
        binstall_args_for_tool(tool, tool_static, config)
    } else {
        vec!["install", tool_static]
    };

    let install_call = ExpectedCall {
        cmd: "cargo",
        args,
        result: Ok(match failure_message {
            Some(message) => failure_output(message),
            None => success_output(),
        }),
    };

    let mut calls = vec![install_call];
    let post_config = PostPrimaryConfig {
        tool: tool.to_owned(),
        tool_static,
        has_repository_context: config.has_repository_context,
        primary_succeeded: failure_message.is_none(),
        use_binstall,
        cargo_install_failure: config.cargo_install_failure.map(String::from),
    };
    calls.extend(post_primary_calls(&post_config));
    calls
}

/// Builds the complete list of expected calls for a dependency binary test scenario.
pub fn expected_calls(tool: &str, config: ExpectedCallConfig<'_>) -> Vec<ExpectedCall> {
    let mut calls = vec![binstall_version_check(config.is_binstall_available)];

    if config.should_verify_repository_install {
        calls.extend(repository_verification_call(
            tool,
            config.is_repository_verification_failing,
        ));
        if !config.is_repository_verification_failing {
            return calls;
        }
    }

    calls.extend(cargo_fallback_calls(tool, &config));
    calls
}

/// Creates an expected call for verifying cargo-dylint installation.
pub fn cargo_dylint_check() -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Ok(success_output()),
    }
}

/// Creates an expected call for verifying cargo-dylint with a fixed result.
pub fn cargo_dylint_check_with_result(result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result,
    }
}
