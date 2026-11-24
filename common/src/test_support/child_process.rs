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

/// Which side of a [`run_in_child`] call the current process is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildRole {
    /// The original test process, after its child has passed.
    Parent,
    /// The re-executed child, after its body has run.
    Child,
}

/// Run `body` in a child process whose environment applies `variables`.
///
/// A `Some` value sets the variable and `None` removes it from the inherited
/// environment. Call this at the start of the test, passing the test's own
/// libtest name (see [`child_test_path`]). The parent re-executes the test
/// binary filtered to exactly that test; the child recognizes the marker, runs
/// `body`, and returns [`ChildRole::Child`] so the test can return normally and
/// drop what it built. The parent checks that the child passed and returns
/// [`ChildRole::Parent`], after which it reads whatever the child produced. A
/// test whose remaining code only suits the parent returns early on
/// [`ChildRole::Child`].
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
/// use whitaker_common::test_support::{ChildRole, run_in_child};
///
/// fn sees_the_variable() -> Result<ChildRole, String> {
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
) -> Result<ChildRole, String> {
    if std::env::var_os(CHILD_MARKER).is_some() {
        body();
        writeln!(std::io::stdout(), "{CHILD_RAN}")
            .map_err(|error| format!("report the child's completion: {error}"))?;
        return Ok(ChildRole::Child);
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
    Ok(ChildRole::Parent)
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

    /// Writes a marker file when dropped, if given a path.
    struct WriteOnDrop(Option<std::path::PathBuf>);

    impl Drop for WriteOnDrop {
        fn drop(&mut self) {
            if let Some(path) = &self.0 {
                // The parent asserts the file exists, so a failed write shows there.
                drop(std::fs::write(path, "dropped"));
            }
        }
    }

    /// Check that what a test built before calling [`run_in_child`] is dropped
    /// in the child, so its temporary files are cleaned up.
    ///
    /// Regression: the child once exited the process inside the helper, which
    /// skipped every destructor on the test's stack.
    #[test]
    fn the_child_drops_what_the_test_built_before_the_call() {
        const MARKER: &str = "WHITAKER_TEST_DROP_MARKER";
        let directory = tempfile::tempdir().expect("the marker directory should be created");
        let marker = directory.path().join("dropped");
        let marker_value = marker
            .to_str()
            .expect("the marker path should be valid UTF-8");
        let guard = WriteOnDrop(std::env::var_os(MARKER).map(std::path::PathBuf::from));
        let test_path = child_test_path(
            module_path!(),
            "the_child_drops_what_the_test_built_before_the_call",
        );

        let role = run_in_child(&test_path, &[(MARKER, Some(marker_value))], || {})
            .expect("the child test should pass");

        if role == ChildRole::Parent {
            drop(guard);
            assert!(marker.exists(), "the child should have run its destructors");
        }
    }
}
