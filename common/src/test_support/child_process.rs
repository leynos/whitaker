//! Run a test body in a child process that carries its own environment.
//!
//! Tests must not change the process environment, because a variable set in
//! one test is visible to every test sharing the process. A test that needs an
//! ambient variable (for example `DYLINT_LOCALE`, or `GIT_DIR`) instead
//! re-runs itself in a child process whose environment the parent sets through
//! `Command::env` and `Command::env_remove`.

use std::{ffi::OsStr, io::Write, process::Command};

/// Environment variable that marks the re-executed child.
const CHILD_MARKER: &str = "WHITAKER_TEST_CHILD";

/// Line the child prints once its body has run to completion.
///
/// The parent requires it, so a `test_name` that matches no test (the child
/// then runs nothing and exits successfully) fails loudly instead of passing.
const CHILD_RAN: &str = "whitaker-test-child-body-ran";

/// Build the libtest name of a test from `module_path!()` and its function.
///
/// `module_path!()` begins with the crate name, which libtest leaves out of
/// the names it filters on, so the first segment is dropped. In an
/// integration-test binary the whole path is the binary's own name, so the
/// result is just the function name.
///
/// # Examples
///
/// ```
/// use whitaker_common::test_support::child_test_path;
///
/// assert_eq!(
///     child_test_path("my_lint::tests::ui", "ui_runs_in_welsh"),
///     "tests::ui::ui_runs_in_welsh",
/// );
/// assert_eq!(child_test_path("ui", "ui_cy"), "ui_cy");
/// ```
#[must_use]
pub fn child_test_path(module_path: &str, function: &str) -> String {
    match module_path.split_once("::") {
        Some((_, below_crate)) => format!("{below_crate}::{function}"),
        None => function.to_owned(),
    }
}

/// Run `body` in a child process whose environment applies `variables`.
///
/// A `Some` value sets the variable and `None` removes it from the inherited
/// environment. Call this at the start of the test, passing the test's own
/// libtest name (see [`child_test_path`]). The parent re-executes the test
/// binary filtered to exactly that test; the child recognizes the marker, runs
/// `body`, and exits, so nothing after this call runs in the child. The parent
/// checks that the child passed and then continues, which is where it reads
/// whatever the child produced.
///
/// # Errors
///
/// In the parent, returns a description if the child cannot be started, fails,
/// or never reaches the end of `body`. In the child, a panic in `body` is the
/// child's failure, which the parent reports.
///
/// # Examples
///
/// ```
/// use whitaker_common::test_support::run_in_child;
///
/// fn sees_the_variable() -> Result<(), String> {
///     run_in_child("sees_the_variable", &[("EXAMPLE_FLAG", Some("1"))], || {
///         assert_eq!(std::env::var("EXAMPLE_FLAG").as_deref(), Ok("1"));
///     })
/// }
/// # // Not run: a doctest binary has no libtest filter to re-enter.
/// # let _ = sees_the_variable;
/// ```
pub fn run_in_child(
    test_name: &str,
    variables: &[(&str, Option<&str>)],
    body: impl FnOnce(),
) -> Result<(), String> {
    if std::env::var_os(CHILD_MARKER).is_some() {
        body();
        writeln!(std::io::stdout(), "{CHILD_RAN}")
            .map_err(|error| format!("report the child's completion: {error}"))?;
        std::process::exit(0);
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("locate the running test binary: {error}"))?;
    let mut command = Command::new(executable);
    command
        .args([
            test_name,
            "--exact",
            "--include-ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_MARKER, "1");
    for (name, setting) in variables {
        match setting {
            Some(value) => command.env(OsStr::new(name), OsStr::new(value)),
            None => command.env_remove(OsStr::new(name)),
        };
    }
    let output = command
        .output()
        .map_err(|error| format!("start the child test process: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(format!(
            "child test {test_name} failed: {}\n{stdout}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    if !stdout.contains(CHILD_RAN) {
        return Err(format!(
            "child test {test_name} ran no body; check the test name:\n{stdout}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the child-process helpers.

    use super::*;

    /// Drop the crate segment so the name matches libtest's.
    #[test]
    fn the_crate_segment_is_dropped() {
        assert_eq!(
            child_test_path("my_lint::tests::ui", "ui_cy"),
            "tests::ui::ui_cy"
        );
    }

    /// An integration-test binary's path is its own name, so only the
    /// function remains.
    #[test]
    fn an_integration_binary_path_is_just_the_function() {
        assert_eq!(child_test_path("ui", "ui_cy"), "ui_cy");
    }
}
