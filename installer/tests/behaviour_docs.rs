//! Behaviour-driven tests for documentation examples.
//!
//! These scenarios validate that documented TOML examples parse correctly
//! and produce expected configurations. Examples are loaded directly from
//! the user guide to prevent drift between documentation and tests.

mod doc_extraction;

use std::cell::{Ref, RefCell};

use doc_extraction::extraction::{DOC_TOML_BLOCKS, find_block_containing};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
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

#[whitaker_test_macros::allow_fixture_expansion_lints]
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
    let block = find_block_containing(r#"pattern = "whitaker_suite""#);
    set_toml_content(toml_world, &block);
}

#[given("a workspace metadata example for individual crates")]
fn given_individual_crates_metadata(toml_world: &TomlWorld) -> Result<(), String> {
    // Matches the individual crates example showing explicit lint patterns
    let block = DOC_TOML_BLOCKS
        .iter()
        .find(|b| {
            b.contains(r#"pattern = "crates/module_max_lines""#)
                && !b.contains("tag =")
                && !b.contains("rev =")
        })
        .ok_or_else(|| String::from("no individual crates TOML block found"))?
        .clone();
    set_toml_content(toml_world, &block);
    Ok(())
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
    let block = find_block_containing("/whitaker/lints/");
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

/// Borrows the parsed TOML table, failing when the When step has not run.
fn parsed_table(toml_world: &TomlWorld) -> Result<Ref<'_, Table>, String> {
    Ref::filter_map(toml_world.parsed.borrow(), Option::as_ref)
        .map_err(|_| String::from("expected parsed TOML"))
}

/// Get a reference to the first library entry in `workspace.metadata.dylint.libraries`.
fn get_first_library(table: &Table) -> Result<&toml::Value, String> {
    table
        .get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("dylint"))
        .and_then(|d| d.get("libraries"))
        .and_then(|l| l.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| String::from("expected workspace.metadata.dylint.libraries[0]"))
}

/// Get a string field from the first library entry.
fn get_library_string_field<'a>(table: &'a Table, field: &str) -> Result<&'a str, String> {
    get_first_library(table)?
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("expected libraries[0].{field} to be a string"))
}

/// Get an integer configuration value from a nested table.
fn get_config_integer(table: &Table, section: &str, key: &str) -> Result<i64, String> {
    table
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("expected {section}.{key} to be an integer"))
}

/// Compare two values for equality, reporting a mismatch as an error.
fn ensure_eq<T, U>(actual: &T, expected: &U, context: &str) -> Result<(), String>
where
    T: PartialEq<U> + std::fmt::Debug + ?Sized,
    U: std::fmt::Debug + ?Sized,
{
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{context}: expected {expected:?}, got {actual:?}"))
    }
}

// ---------------------------------------------------------------------------
// Assertion steps
// ---------------------------------------------------------------------------

#[then("parsing succeeds")]
fn then_parsing_succeeds(toml_world: &TomlWorld) -> Result<(), String> {
    let error = toml_world.error.borrow();
    error.as_ref().map_or(Ok(()), |message| {
        Err(format!(
            "expected TOML to parse successfully, but got error: {message}"
        ))
    })
}

#[then("the libraries pattern is {expected}")]
fn then_libraries_pattern_is(toml_world: &TomlWorld, expected: String) -> Result<(), String> {
    let table = parsed_table(toml_world)?;
    let pattern = get_library_string_field(&table, "pattern")?;
    ensure_eq(pattern, expected.as_str(), "libraries[0].pattern")
}

#[then("the libraries pattern starts with {prefix}")]
fn then_libraries_pattern_starts_with(
    toml_world: &TomlWorld,
    prefix: String,
) -> Result<(), String> {
    let table = parsed_table(toml_world)?;
    let pattern = get_library_string_field(&table, "pattern")?;
    if pattern.starts_with(&prefix) {
        Ok(())
    } else {
        Err(format!(
            "expected pattern to start with '{prefix}', got '{pattern}'"
        ))
    }
}

#[then("the tag field is present")]
fn then_tag_present(toml_world: &TomlWorld) -> Result<(), String> {
    let table = parsed_table(toml_world)?;
    let tag = get_library_string_field(&table, "tag")?;
    ensure_eq(tag, "v0.1.0", "libraries[0].tag")
}

#[then("the revision field is present")]
fn then_revision_present(toml_world: &TomlWorld) -> Result<(), String> {
    let table = parsed_table(toml_world)?;
    let rev = get_library_string_field(&table, "rev")?;
    ensure_eq(rev, "abc123def456", "libraries[0].rev")
}

#[then("the path field is present")]
fn then_path_present(toml_world: &TomlWorld) -> Result<(), String> {
    let table = parsed_table(toml_world)?;
    let path = get_library_string_field(&table, "path")?;
    let has_prebuilt_layout = path.contains("/whitaker/lints/")
        && path.contains("/nightly-")
        && path.contains("/x86_64-unknown-linux-gnu/lib");
    if has_prebuilt_layout {
        Ok(())
    } else {
        Err(format!(
            "expected path to contain prebuilt lints layout, got: {path}"
        ))
    }
}

#[then("module_max_lines configuration is present")]
fn then_module_max_lines_present(toml_world: &TomlWorld) -> Result<(), String> {
    let table = parsed_table(toml_world)?;
    let max_lines = get_config_integer(&table, "module_max_lines", "max_lines")?;
    ensure_eq(&max_lines, &500, "module_max_lines.max_lines")
}

#[then("conditional_max_n_branches configuration is present")]
fn then_conditional_max_branches_present(toml_world: &TomlWorld) -> Result<(), String> {
    let table = parsed_table(toml_world)?;
    let max_branches = get_config_integer(&table, "conditional_max_n_branches", "max_branches")?;
    ensure_eq(&max_branches, &3, "conditional_max_n_branches.max_branches")
}

#[then("no_expect_outside_tests additional_test_attributes configuration is present")]
fn then_no_expect_outside_tests_additional_test_attributes_present(
    toml_world: &TomlWorld,
) -> Result<(), String> {
    let table = parsed_table(toml_world)?;

    let attributes = table
        .get("no_expect_outside_tests")
        .and_then(|t| t.get("additional_test_attributes"))
        .and_then(|a| a.as_array())
        .ok_or_else(|| {
            String::from("expected no_expect_outside_tests.additional_test_attributes array")
        })?;

    let values = attributes
        .iter()
        .map(|v| {
            v.as_str().ok_or_else(|| {
                String::from("expected additional_test_attributes entries to be strings")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    ensure_eq(
        values.as_slice(),
        ["my_framework::test", "wasm_bindgen_test"].as_slice(),
        "no_expect_outside_tests.additional_test_attributes",
    )
}

#[then("no_unwrap_or_else_panic allow_in_main configuration is present")]
fn then_no_unwrap_or_else_panic_allow_in_main_present(
    toml_world: &TomlWorld,
) -> Result<(), String> {
    let table = parsed_table(toml_world)?;

    let allow_in_main = table
        .get("no_unwrap_or_else_panic")
        .and_then(|t| t.get("allow_in_main"))
        .and_then(toml::Value::as_bool)
        .ok_or_else(|| String::from("expected no_unwrap_or_else_panic.allow_in_main boolean"))?;

    if allow_in_main {
        Ok(())
    } else {
        Err(String::from(
            "expected no_unwrap_or_else_panic.allow_in_main to be true",
        ))
    }
}

#[then("locale configuration is present")]
fn then_locale_configuration_present(toml_world: &TomlWorld) -> Result<(), String> {
    let table = parsed_table(toml_world)?;

    let locale = table
        .get("locale")
        .and_then(|v| v.as_str())
        .ok_or_else(|| String::from("expected locale string"))?;

    ensure_eq(locale, "cy", "locale")
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
