//! Run a test body in a child process that carries its own environment.
//!
//! Tests must not change the process environment, because a variable set in
//! one test is visible to every test sharing the process. A test that needs an
//! ambient variable (for example `GIT_DIR`) instead re-runs itself in a child
//! process whose environment the parent sets through `Command::env`.

use std::{ffi::OsStr, io::Write, process::Command};

/// Environment variable that marks the re-executed child.
const CHILD_MARKER: &str = "WHITAKER_TEST_CHILD";

/// Line the child prints once its body has run to completion.
///
/// The parent requires it, so a `test_name` that matches no test (the child
/// then runs nothing and exits successfully) fails loudly instead of passing.
const CHILD_RAN: &str = "whitaker-test-child-body-ran";

/// Run `body` in a child process whose environment includes `variables`.
///
/// Call this from the test itself, passing the test's own libtest name (the
/// module path below the crate root plus the function name, or just the
/// function name in an integration-test binary). The parent re-executes the
/// test binary filtered to exactly that test; the child recognizes the marker
/// and runs `body`, and the parent checks that the child passed.
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
/// use whitaker_installer::test_utils::run_in_child;
///
/// fn sees_the_variable() -> Result<(), String> {
///     run_in_child("sees_the_variable", &[("EXAMPLE_FLAG", "1")], || {
///         assert_eq!(std::env::var("EXAMPLE_FLAG").as_deref(), Ok("1"));
///     })
/// }
/// # // Not run: a doctest binary has no libtest filter to re-enter.
/// # let _ = sees_the_variable;
/// ```
pub fn run_in_child(
    test_name: &str,
    variables: &[(&str, &str)],
    body: impl FnOnce(),
) -> Result<(), String> {
    if std::env::var_os(CHILD_MARKER).is_some() {
        body();
        writeln!(std::io::stdout(), "{CHILD_RAN}")
            .map_err(|error| format!("report the child's completion: {error}"))?;
        return Ok(());
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("locate the running test binary: {error}"))?;
    let mut command = Command::new(executable);
    command
        .args([test_name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_MARKER, "1");
    for (name, value) in variables {
        command.env(OsStr::new(name), OsStr::new(value));
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
