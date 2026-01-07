//! Behaviour-driven tests for documentation examples.
//!
//! These scenarios validate that documented TOML examples parse correctly
//! and produce expected configurations. Examples are loaded directly from
//! the user guide to prevent drift between documentation and tests.

mod doc_extraction;

use doc_extraction::extraction::{DOC_TOML_BLOCKS, find_block_containing};
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
// Given steps - Workspace metadata examples (loaded from documentation)
// ---------------------------------------------------------------------------

#[given("a workspace metadata example for suite-only")]
fn given_suite_only_metadata(toml_world: &TomlWorld) {
    // Matches the "aggregated suite provides the simplest setup" example
    let block = find_block_containing(r#"pattern = "suite""#);
    set_toml_content(toml_world, &block);
}

#[given("a workspace metadata example for individual crates")]
fn given_individual_crates_metadata(toml_world: &TomlWorld) {
    // Matches the individual crates example showing explicit lint patterns
    let block = DOC_TOML_BLOCKS
        .iter()
        .find(|b| {
            b.contains(r#"pattern = "crates/module_max_lines""#)
                && !b.contains("tag =")
                && !b.contains("rev =")
        })
        .expect("no individual crates TOML block found")
        .clone();
    set_toml_content(toml_world, &block);
}

#[given("a workspace metadata example with tag pinning")]
fn given_tag_pinning_metadata(toml_world: &TomlWorld) {
    let block = find_block_containing(r#"tag = "v0.1.0""#);
    set_toml_content(toml_world, &block);
}

#[given("a workspace metadata example with revision pinning")]
fn given_revision_pinning_metadata(toml_world: &TomlWorld) {
    let block = find_block_containing(r#"rev = "abc123def456""#);
    set_toml_content(toml_world, &block);
}

#[given("a workspace metadata example with pre-built path")]
fn given_prebuilt_path_metadata(toml_world: &TomlWorld) {
    let block = find_block_containing("path = ");
    set_toml_content(toml_world, &block);
}

#[given("a dylint.toml example with lint configuration")]
fn given_dylint_toml_config(toml_world: &TomlWorld) {
    // The lint configuration block contains module_max_lines and other settings
    let block = find_block_containing("[module_max_lines]");
    set_toml_content(toml_world, &block);
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

// Helper functions for TOML navigation
// ---------------------------------------------------------------------------

/// Get a reference to the first library entry in workspace.metadata.dylint.libraries.
fn get_first_library(table: &Table) -> &toml::Value {
    table
        .get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("dylint"))
        .and_then(|d| d.get("libraries"))
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .expect("expected workspace.metadata.dylint.libraries[0]")
}

/// Get a string field from the first library entry.
fn get_library_string_field<'a>(table: &'a Table, field: &str) -> &'a str {
    get_first_library(table)
        .get(field)
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("expected libraries[0].{field} to be a string"))
}

/// Get an integer configuration value from a nested table.
fn get_config_integer(table: &Table, section: &str, key: &str) -> i64 {
    table
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_integer())
        .unwrap_or_else(|| panic!("expected {section}.{key} to be an integer"))
}

// ---------------------------------------------------------------------------
// Assertion steps
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

    let pattern = get_library_string_field(table, "pattern");

    assert_eq!(pattern, expected);
}

#[then("the libraries pattern starts with {prefix}")]
fn then_libraries_pattern_starts_with(toml_world: &TomlWorld, prefix: String) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let pattern = get_library_string_field(table, "pattern");

    assert!(
        pattern.starts_with(&prefix),
        "expected pattern to start with '{prefix}', got '{pattern}'"
    );
}

#[then("the tag field is present")]
fn then_tag_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let tag = get_library_string_field(table, "tag");

    assert_eq!(tag, "v0.1.0", "expected tag == \"v0.1.0\"");
}

#[then("the revision field is present")]
fn then_revision_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let rev = get_library_string_field(table, "rev");

    assert_eq!(rev, "abc123def456", "expected rev == \"abc123def456\"");
}

#[then("the path field is present")]
fn then_path_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let path = get_library_string_field(table, "path");

    assert!(
        path.contains("dylint/lib") && path.contains("/release"),
        "expected path to contain toolchain/release structure, got: {path}"
    );
}

#[then("module_max_lines configuration is present")]
fn then_module_max_lines_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let max_lines = get_config_integer(table, "module_max_lines", "max_lines");

    assert_eq!(max_lines, 500);
}

#[then("conditional_max_n_branches configuration is present")]
fn then_conditional_max_branches_present(toml_world: &TomlWorld) {
    let parsed = toml_world.parsed.borrow();
    let table = parsed.as_ref().expect("expected parsed TOML");

    let max_branches = get_config_integer(table, "conditional_max_n_branches", "max_branches");

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
