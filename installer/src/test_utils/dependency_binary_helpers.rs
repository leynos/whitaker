//! Test helpers for dependency binary installation tests.

use crate::dependency_binaries::find_dependency_binary;
use crate::error::Result;
use crate::test_utils::{ExpectedCall, failure_output, success_output};
use std::process::Output;

/// Configuration for generating expected calls in dependency binary tests.
pub struct ExpectedCallConfig<'a> {
    /// Whether cargo-binstall is available.
    pub is_binstall_available: bool,
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
    ExpectedCall {
        cmd: "cargo",
        args: vec!["binstall", "-y", tool],
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
pub fn repository_verification_call(tool: &str, verification_fails: bool) -> ExpectedCall {
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

/// Returns the expected verification call for a given tool.
fn tool_verification_check(tool: &str) -> ExpectedCall {
    match tool {
        "cargo-dylint" => cargo_dylint_check(),
        "dylint-link" => dylint_link_check(),
        other => panic!("unexpected tool: {other}"),
    }
}

/// Configuration for post-primary installation call sequence.
struct PostPrimaryConfig {
    /// The tool name.
    tool: String,
    /// Static tool name for cargo install args.
    tool_static: &'static str,
    /// Whether the primary installation succeeded.
    primary_succeeded: bool,
    /// Whether to use binstall (vs cargo install).
    use_binstall: bool,
    /// Error message for cargo install failure (None if succeeds).
    cargo_install_failure: Option<String>,
}

/// Builds the sequence of calls that follow the primary install attempt.
fn post_primary_calls(cfg: &PostPrimaryConfig) -> Vec<ExpectedCall> {
    if cfg.primary_succeeded {
        return vec![tool_verification_check(&cfg.tool)];
    }
    if !cfg.use_binstall {
        return vec![];
    }
    // binstall failed: check if we should sequence a cargo-install attempt
    if cfg.cargo_install_failure.is_none() {
        // cargo install succeeds after binstall fails
        let cargo_call = cargo_install(cfg.tool_static, Ok(success_output()));
        return vec![cargo_call, tool_verification_check(&cfg.tool)];
    }
    // binstall failed and cargo install also fails
    if let Some(message) = cfg.cargo_install_failure.as_deref() {
        let cargo_call = cargo_install(cfg.tool_static, Ok(failure_output(message)));
        vec![cargo_call]
    } else {
        vec![]
    }
}

/// Creates expected calls for cargo fallback installation (binstall or install).
pub fn cargo_fallback_calls(tool: &str, config: &ExpectedCallConfig<'_>) -> Vec<ExpectedCall> {
    // Intentional leak in tests to extend lifetime for static string usage;
    // acceptable here as it will not be freed.
    let tool_static: &'static str = Box::leak(tool.to_owned().into_boxed_str());

    if config.is_repository_asset_missing {
        let version = dependency_version(tool);
        let install_call = cargo_source_install(
            tool_static,
            version,
            Ok(match config.cargo_install_failure {
                Some(message) => failure_output(message),
                None => success_output(),
            }),
        );
        return if config.cargo_install_failure.is_none() {
            vec![install_call, tool_verification_check(tool)]
        } else {
            vec![install_call]
        };
    }

    let (use_binstall, failure_message) = if config.is_binstall_available {
        (true, config.cargo_binstall_failure)
    } else {
        (false, config.cargo_install_failure)
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
    let post_config = PostPrimaryConfig {
        tool: tool.to_owned(),
        tool_static,
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
        calls.push(repository_verification_call(
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

/// Creates an expected call for verifying dylint-link installation.
pub fn dylint_link_check() -> ExpectedCall {
    ExpectedCall {
        cmd: "dylint-link",
        args: vec!["--version"],
        result: Ok(success_output()),
    }
}

/// Creates an expected call for verifying dylint-link with a fixed result.
pub fn dylint_link_check_with_result(result: Result<Output>) -> ExpectedCall {
    ExpectedCall {
        cmd: "dylint-link",
        args: vec!["--version"],
        result,
    }
}
