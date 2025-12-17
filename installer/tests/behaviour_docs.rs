//! Behaviour-driven tests for documentation examples.
//!
//! These scenarios validate that documented TOML examples parse correctly
//! and produce expected configurations.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use toml::Table;

// ---------------------------------------------------------------------------
// TOML validation world
// ---------------------------------------------------------------------------

/// World fixture for TOML parsing scenarios.
#[derive(Default)]
struct TomlWorld {
    content: RefCell<String>,
    parsed: RefCell<Option<Table>>,
    error: RefCell<Option<String>>,
}

#[fixture]
fn toml_world() -> TomlWorld {
    TomlWorld::default()
}

/// Helper function to set TOML content in the world fixture.
fn set_toml_content(toml_world: &TomlWorld, content: &str) {
    toml_world.content.replace(content.to_owned());
}

// ---------------------------------------------------------------------------
// Given steps - Workspace metadata examples
// ---------------------------------------------------------------------------

#[given("a workspace metadata example for suite-only")]
fn given_suite_only_metadata(toml_world: &TomlWorld) {
    set_toml_content(
        toml_world,
        r#"
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "suite" }
]
"#,
    );
}

#[given("a workspace metadata example for individual crates")]
fn given_individual_crates_metadata(toml_world: &TomlWorld) {
    set_toml_content(
        toml_world,
        r#"
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "crates/*" }
]
"#,
    );
}

#[given("a workspace metadata example with tag pinning")]
fn given_tag_pinning_metadata(toml_world: &TomlWorld) {
    set_toml_content(
        toml_world,
        r#"
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "crates/*", tag = "v0.1.0" }
]
"#,
    );
}

#[given("a workspace metadata example with revision pinning")]
fn given_revision_pinning_metadata(toml_world: &TomlWorld) {
    set_toml_content(
        toml_world,
        r#"
[workspace.metadata.dylint]
libraries = [
  { git = "https://github.com/leynos/whitaker", pattern = "crates/*", rev = "abc123def456" }
]
"#,
    );
}

#[given("a workspace metadata example with pre-built path")]
fn given_prebuilt_path_metadata(toml_world: &TomlWorld) {
    set_toml_content(
        toml_world,
        r#"
[workspace.metadata.dylint]
libraries = [
  { path = "/home/user/.local/share/dylint/lib/nightly-2025-01-15/release" }
]
"#,
    );
}

#[given("a dylint.toml example with lint configuration")]
fn given_dylint_toml_config(toml_world: &TomlWorld) {
    set_toml_content(
        toml_world,
        r#"
locale = "cy"

[module_max_lines]
max_lines = 500

[conditional_max_n_branches]
max_branches = 3

[no_expect_outside_tests]
additional_test_attributes = ["my_framework::test", "async_std::test"]

[no_unwrap_or_else_panic]
allow_in_main = true
"#,
    );
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the TOML is parsed")]
fn when_toml_parsed(toml_world: &TomlWorld) {
    let content = toml_world.content.borrow();
    match content.parse::<Table>() {
        Ok(table) => {
            toml_world.parsed.replace(Some(table));
            toml_world.error.replace(None);
        }
        Err(e) => {
            toml_world.parsed.replace(None);
            toml_world.error.replace(Some(e.to_string()));
        }
    }
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("parsing succeeds")]
fn then_parsing_succeeds(toml_world: &TomlWorld) {
    let error = toml_world.error.borrow();
    assert!(
        error.is_none(),
        "expected TOML to parse successfully, but got error: {:?}",
        error
    );
}

#[then("the libraries pattern is {expected}")]
fn then_libraries_pattern_is(toml_world: &TomlWorld, expected: String) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let pattern = table
        .get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("dylint"))
        .and_then(|d| d.get("libraries"))
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .and_then(|lib| lib.get("pattern"))
        .and_then(|p| p.as_str())
        .expect("expected libraries[0].pattern");

    assert_eq!(pattern, expected);
}

#[then("the tag field is present")]
fn then_tag_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let tag = table
        .get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("dylint"))
        .and_then(|d| d.get("libraries"))
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .and_then(|lib| lib.get("tag"))
        .and_then(|t| t.as_str())
        .expect("expected tag field to be present");

    assert_eq!(tag, "v0.1.0", "expected tag == \"v0.1.0\"");
}

#[then("the revision field is present")]
fn then_revision_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let rev = table
        .get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("dylint"))
        .and_then(|d| d.get("libraries"))
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .and_then(|lib| lib.get("rev"))
        .and_then(|r| r.as_str())
        .expect("expected rev field to be present");

    assert_eq!(rev, "abc123def456", "expected rev == \"abc123def456\"");
}

#[then("the path field is present")]
fn then_path_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let path = table
        .get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("dylint"))
        .and_then(|d| d.get("libraries"))
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .and_then(|lib| lib.get("path"))
        .and_then(|p| p.as_str())
        .expect("expected path field to be present");

    assert_eq!(
        path, "/home/user/.local/share/dylint/lib/nightly-2025-01-15/release",
        "expected path == \"/home/user/.local/share/dylint/lib/nightly-2025-01-15/release\""
    );
}

#[then("module_max_lines configuration is present")]
fn then_module_max_lines_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let max_lines = table
        .get("module_max_lines")
        .and_then(|m| m.get("max_lines"))
        .and_then(|v| v.as_integer())
        .expect("expected module_max_lines.max_lines");

    assert_eq!(max_lines, 500);
}

#[then("conditional_max_n_branches configuration is present")]
fn then_conditional_max_branches_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let max_branches = table
        .get("conditional_max_n_branches")
        .and_then(|m| m.get("max_branches"))
        .and_then(|v| v.as_integer())
        .expect("expected conditional_max_n_branches.max_branches");

    assert_eq!(max_branches, 3);
}

#[then("no_expect_outside_tests additional_test_attributes configuration is present")]
fn then_no_expect_outside_tests_additional_test_attributes_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let attributes = table
        .get("no_expect_outside_tests")
        .and_then(|t| t.get("additional_test_attributes"))
        .and_then(|a| a.as_array())
        .expect("expected no_expect_outside_tests.additional_test_attributes array");

    let values: Vec<_> = attributes
        .iter()
        .map(|v| {
            v.as_str()
                .expect("expected additional_test_attributes entries to be strings")
        })
        .collect();

    assert_eq!(
        values,
        vec!["my_framework::test", "async_std::test"],
        "unexpected no_expect_outside_tests.additional_test_attributes"
    );
}

#[then("no_unwrap_or_else_panic allow_in_main configuration is present")]
fn then_no_unwrap_or_else_panic_allow_in_main_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let allow_in_main = table
        .get("no_unwrap_or_else_panic")
        .and_then(|t| t.get("allow_in_main"))
        .and_then(|v| v.as_bool())
        .expect("expected no_unwrap_or_else_panic.allow_in_main boolean");

    assert!(
        allow_in_main,
        "expected no_unwrap_or_else_panic.allow_in_main to be true"
    );
}

#[then("locale configuration is present")]
fn then_locale_configuration_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let locale = table
        .get("locale")
        .and_then(|v| v.as_str())
        .expect("expected locale string");

    assert_eq!(locale, "cy", "expected locale == \"cy\"");
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(
    path = "tests/features/consumer_guidance.feature",
    name = "Suite-only workspace metadata is valid TOML"
)]
fn scenario_suite_only_metadata(toml_world: TomlWorld) {
    let _ = toml_world;
}

#[scenario(
    path = "tests/features/consumer_guidance.feature",
    name = "Individual crates workspace metadata is valid TOML"
)]
fn scenario_individual_crates_metadata(toml_world: TomlWorld) {
    let _ = toml_world;
}

#[scenario(
    path = "tests/features/consumer_guidance.feature",
    name = "Version-pinned workspace metadata with tag is valid TOML"
)]
fn scenario_tag_pinning_metadata(toml_world: TomlWorld) {
    let _ = toml_world;
}

#[scenario(
    path = "tests/features/consumer_guidance.feature",
    name = "Version-pinned workspace metadata with revision is valid TOML"
)]
fn scenario_revision_pinning_metadata(toml_world: TomlWorld) {
    let _ = toml_world;
}

#[scenario(
    path = "tests/features/consumer_guidance.feature",
    name = "Pre-built library path workspace metadata is valid TOML"
)]
fn scenario_prebuilt_path_metadata(toml_world: TomlWorld) {
    let _ = toml_world;
}

#[scenario(
    path = "tests/features/consumer_guidance.feature",
    name = "dylint.toml lint configuration is valid TOML"
)]
fn scenario_dylint_toml_config(toml_world: TomlWorld) {
    let _ = toml_world;
}
