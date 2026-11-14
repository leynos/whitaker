//! Configuration-boundary integration tests for `no_std_fs_operations`.
//!
//! These tests cover the forward-compatible `dylint.toml` load boundary: a key
//! that the installed suite does not understand must be reported without
//! discarding the exclusions it does understand. They share the fixture harness
//! with the exclusion tests and are `#[ignore]`d for the same reason.

use super::{test_support::create_config_fixture_project, *};

/// An unknown key must not discard the exclusions the installed lint knows.
///
/// This is the regression for issue #332: a configuration written for a newer
/// suite used to fail deserialization and silently disable every exclusion.
#[test]
#[ignore = "requires cargo-dylint and built lint library"]
#[serial]
fn unknown_key_preserves_known_exclusions() -> anyhow::Result<()> {
    let lint_library_path = lint_library_path().context("failed to build lint library")?;
    let fixture = create_config_fixture_project(
        "unknown_key_excluded_crate",
        concat!(
            "[no_std_fs_operations]\n",
            "excluded_crates = [\"unknown_key_excluded_crate\"]\n",
            "future_option = true\n",
        ),
    )
    .context("failed to create unknown-key fixture project")?;

    let result = run_cargo_dylint(fixture.root(), &lint_library_path)
        .context("failed to run cargo dylint")?;
    let count = diagnostic_count(&result.stdout)?;

    anyhow::ensure!(
        result.is_success,
        "an unknown key should warn, not fail the build\nstderr:\n{}",
        result.stderr
    );
    anyhow::ensure!(
        count == 0,
        "known exclusions should still apply, got {count} diagnostics"
    );
    assertions::assert_config_warning(
        &result,
        &["future_option", env!("CARGO_PKG_VERSION"), "revision"],
    )
}

/// An unknown key alone must not suppress diagnostics.
#[test]
#[ignore = "requires cargo-dylint and built lint library"]
#[serial]
fn unknown_key_does_not_suppress_diagnostics() -> anyhow::Result<()> {
    let lint_library_path = lint_library_path().context("failed to build lint library")?;
    let fixture = create_config_fixture_project(
        "unknown_key_included_crate",
        concat!("[no_std_fs_operations]\n", "future_option = true\n",),
    )
    .context("failed to create unknown-key fixture project")?;

    let result = run_cargo_dylint(fixture.root(), &lint_library_path)
        .context("failed to run cargo dylint")?;
    let count = diagnostic_count(&result.stdout)?;

    anyhow::ensure!(
        !result.is_success,
        "the fixture should still fail on its std::fs usage"
    );
    anyhow::ensure!(count > 0, "an unknown key must not suppress diagnostics");
    assertions::assert_config_warning(&result, &["future_option"])
}

/// A wrong-typed known value discards the section and reports the parse error.
///
/// The valid `excluded_crates` entry names the fixture's own crate, so a loader
/// that salvaged it instead of rejecting the whole section would exclude the
/// crate and leave no diagnostic for the assertions below to find.
#[test]
#[ignore = "requires cargo-dylint and built lint library"]
#[serial]
fn invalid_known_value_discards_the_section() -> anyhow::Result<()> {
    let lint_library_path = lint_library_path().context("failed to build lint library")?;
    let fixture = create_config_fixture_project(
        "invalid_value_crate",
        concat!(
            "[no_std_fs_operations]\n",
            "excluded_crates = [\"invalid_value_crate\"]\n",
            "excluded_paths = \"not-an-array\"\n",
        ),
    )
    .context("failed to create invalid-value fixture project")?;

    let result = run_cargo_dylint(fixture.root(), &lint_library_path)
        .context("failed to run cargo dylint")?;
    let count = diagnostic_count(&result.stdout)?;

    anyhow::ensure!(
        !result.is_success,
        "a discarded section should leave the std::fs diagnostic in place"
    );
    anyhow::ensure!(
        count > 0,
        "a discarded section must not suppress diagnostics"
    );
    assertions::assert_config_warning(&result, &["invalid type", "not-an-array"])
}
