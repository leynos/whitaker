//! Test helpers for dependency binary installation behaviour tests.

use crate::test_utils::{ExpectedCall, failure_output, success_output};

/// Configuration for generating expected calls in dependency binary tests.
pub struct ExpectedCallConfig<'a> {
    /// Whether cargo-binstall is available.
    pub binstall_available: bool,
    /// Whether to verify repository installation.
    pub verify_repository_install: bool,
    /// Whether repository verification should fail.
    pub verification_fails: bool,
    /// Error message for cargo binstall failure (None if succeeds).
    pub cargo_binstall_failure: Option<&'a str>,
    /// Error message for cargo install failure (None if succeeds).
    pub cargo_install_failure: Option<&'a str>,
}

/// Creates an expected call for checking cargo-binstall availability.
pub fn binstall_version_check(binstall_available: bool) -> ExpectedCall {
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

/// Builds the sequence of calls that follow the primary install attempt.
#[allow(clippy::too_many_arguments)]
fn post_primary_calls(
    tool: &str,
    tool_static: &'static str,
    primary_succeeded: bool,
    use_binstall: bool,
    cargo_install_failure: Option<&str>,
) -> Vec<ExpectedCall> {
    if primary_succeeded {
        return vec![tool_verification_check(tool)];
    }
    if !use_binstall {
        return vec![];
    }
    // binstall failed: check if we should sequence a cargo-install attempt
    if cargo_install_failure.is_none() {
        // cargo install succeeds after binstall fails
        let cargo_call = ExpectedCall {
            cmd: "cargo",
            args: vec!["install", tool_static],
            result: Ok(success_output()),
        };
        return vec![cargo_call, tool_verification_check(tool)];
    }
    // binstall failed and cargo install also fails
    let cargo_call = ExpectedCall {
        cmd: "cargo",
        args: vec!["install", tool_static],
        result: Ok(failure_output(cargo_install_failure.unwrap())),
    };
    vec![cargo_call]
}

/// Creates expected calls for cargo fallback installation (binstall or install).
pub fn cargo_fallback_calls(
    tool: &str,
    binstall_available: bool,
    cargo_binstall_failure: Option<&str>,
    cargo_install_failure: Option<&str>,
) -> Vec<ExpectedCall> {
    // Intentional leak in tests to extend lifetime for static string usage;
    // acceptable here as it will not be freed.
    let tool_static: &'static str = Box::leak(tool.to_owned().into_boxed_str());

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
    calls.extend(post_primary_calls(
        tool,
        tool_static,
        failure_message.is_none(),
        use_binstall,
        cargo_install_failure,
    ));
    calls
}

/// Builds the complete list of expected calls for a dependency binary test scenario.
pub fn expected_calls(tool: &str, config: ExpectedCallConfig<'_>) -> Vec<ExpectedCall> {
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

/// Creates an expected call for verifying cargo-dylint installation.
pub fn cargo_dylint_check() -> ExpectedCall {
    ExpectedCall {
        cmd: "cargo",
        args: vec!["dylint", "--version"],
        result: Ok(success_output()),
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
