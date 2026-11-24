//! Run a test body in a child process that carries its own environment.
//!
//! This adapts [`whitaker_common::test_support::run_in_child`], which owns the
//! mechanism, to the installer tests' habit of only ever setting variables.

use whitaker_common::test_support;

/// Run `body` in a child process whose environment includes `variables`.
///
/// Call this from the test itself, passing the test's own libtest name (the
/// module path below the crate root plus the function name, or just the
/// function name in an integration-test binary). The parent re-executes the
/// test binary filtered to exactly that test; the child recognizes the marker,
/// runs `body`, and exits, and the parent checks that the child passed.
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
    let settings: Vec<(&str, Option<&str>)> = variables
        .iter()
        .map(|&(name, value)| (name, Some(value)))
        .collect();
    test_support::run_in_child(test_name, &settings, body)
}
