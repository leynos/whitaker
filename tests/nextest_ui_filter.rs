//! Guards against the nextest filter for `serial-dylint-ui` silently missing
//! new dylint UI test binaries.
//!
//! The `serial-dylint-ui` test-group in `.config/nextest.toml` serializes all
//! dylint UI tests so they do not race on the shared target directory or on
//! Windows temporary-file handles.  If a lint crate adds a UI test whose
//! fully-qualified name does not match the filter, the test will run without
//! serialization or retries, causing flaky Windows CI failures.
//!
//! This test scans for all lint crates that declare a `tests/ui.rs` integration
//! test (which produces a binary named `ui` with a top-level test name of
//! `ui`, **not** `ui::ui`) and asserts that the nextest filters contain the
//! clause needed to capture that pattern. It also protects the named nested
//! Cargo UI harnesses that share compiler resources on constrained runners.
//!
//! The group is spread over several overrides, each carrying the allowance its
//! own harnesses need, so the clauses are checked against the union of their
//! filters rather than against whichever one happens to come first.

use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use toml::Value;

const NESTED_CARGO_UI_FILTER_CLAUSES: [&str; 4] = [
    "test(example_compiles_without_diagnostics)",
    "test(example_harness_collects_call_site_evidence)",
    "test(ui::hand_written_test_companion_does_not_exempt_parent_function)",
    "test(ui::aliased_test_crate_non_companion_does_not_exempt_parent_function)",
];

/// Parses `.config/nextest.toml` into a [`Value`].
fn load_nextest_config() -> Value {
    let repository_root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .unwrap_or_else(|err| panic!("failed to open repository root: {err}"));
    let contents = repository_root
        .read_to_string(".config/nextest.toml")
        .unwrap_or_else(|err| panic!("failed to read .config/nextest.toml: {err}"));
    toml::from_str(&contents)
        .unwrap_or_else(|err| panic!(".config/nextest.toml should parse as TOML: {err}"))
}

/// Returns every `serial-dylint-ui` override table from the config.
fn find_serial_dylint_ui_overrides(config: &Value) -> Vec<&Value> {
    let Some(all_overrides) = config
        .get("profile")
        .and_then(|p| p.get("default"))
        .and_then(|d| d.get("overrides"))
        .and_then(Value::as_array)
    else {
        panic!("profile.default.overrides should be an array");
    };

    let overrides: Vec<&Value> = all_overrides
        .iter()
        .filter(|o| {
            o.get("test-group")
                .and_then(Value::as_str)
                .is_some_and(|group| group == "serial-dylint-ui")
        })
        .collect();
    assert!(
        !overrides.is_empty(),
        "at least one serial-dylint-ui override should exist"
    );
    overrides
}

/// Returns the filter expression from each `serial-dylint-ui` override.
fn extract_filters<'a>(ui_overrides: &[&'a Value]) -> Vec<&'a str> {
    ui_overrides
        .iter()
        .map(|ui_override| {
            ui_override
                .get("filter")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("serial-dylint-ui override should have a filter string"))
        })
        .collect()
}

/// Discovers lint crates under `crates/` whose UI test is an integration test
/// file at `tests/ui.rs`.
///
/// These crates produce a nextest binary named `ui` with a top-level test
/// name of `ui`.  The substring match `test(ui::ui)` does **not** capture
/// them because the reported test name is plain `ui`, not `ui::ui`.
fn crates_with_integration_ui_test() -> Vec<String> {
    let repository_root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .unwrap_or_else(|err| panic!("failed to open repository root: {err}"));
    let crates_dir = repository_root
        .open_dir("crates")
        .unwrap_or_else(|err| panic!("failed to open crates directory: {err}"));

    let entries = crates_dir
        .entries()
        .unwrap_or_else(|err| panic!("failed to read crates directory: {err}"));

    let mut crate_names: Vec<String> = entries
        .filter_map(|dir_entry| {
            let entry =
                dir_entry.unwrap_or_else(|err| panic!("directory entry should be readable: {err}"));
            let crate_directory = entry.open_dir().ok()?;
            if crate_directory.is_file("tests/ui.rs") {
                Some(entry.file_name().unwrap_or_else(|err| {
                    panic!("crate directory should have a UTF-8 name: {err}")
                }))
            } else {
                None
            }
        })
        .collect();

    crate_names.sort();
    crate_names
}

/// Fixture providing every `serial-dylint-ui` override from nextest config.
#[fixture]
fn serial_dylint_ui_overrides() -> Vec<Value> {
    let config = load_nextest_config();
    find_serial_dylint_ui_overrides(&config)
        .into_iter()
        .cloned()
        .collect()
}

#[rstest]
fn serial_dylint_ui_filters_capture_integration_and_nested_cargo_ui_tests(
    serial_dylint_ui_overrides: Vec<Value>,
) {
    let overrides: Vec<&Value> = serial_dylint_ui_overrides.iter().collect();
    let filter = extract_filters(&overrides).join(" | ");
    let crates = crates_with_integration_ui_test();

    assert!(
        !crates.is_empty(),
        "expected at least one lint crate with tests/ui.rs"
    );

    // The integration-test pattern produces binary=ui, test=ui.  The filter
    // must contain a clause that matches this pattern — historically
    // `test(ui::ui)` missed it because the test name is plain `ui`.
    assert!(
        filter.contains("(binary(ui) & test(=ui))"),
        "the serial-dylint-ui filters must contain `(binary(ui) & test(=ui))` \
         to capture integration test binaries named `ui` with a top-level \
         `fn ui()` (e.g. {crates:?}); found filters: {filter}"
    );

    for filter_clause in NESTED_CARGO_UI_FILTER_CLAUSES {
        assert!(
            filter.contains(filter_clause),
            "the serial-dylint-ui filters must capture nested Cargo UI test \
             `{filter_clause}` so its compiler resources are serialized on \
             constrained runners; found filters: {filter}"
        );
    }
}

#[rstest]
fn serial_dylint_ui_filters_have_retry_configuration(serial_dylint_ui_overrides: Vec<Value>) {
    // On Windows, dylint_testing::initialize() can transiently fail with
    // "Access is denied (os error 5)" during NamedTempFile::persist().
    // Retries are essential for reliable CI on Windows.
    let retry_counts: Vec<i64> = serial_dylint_ui_overrides
        .iter()
        .filter_map(|override_table| override_table.get("retries"))
        .filter_map(|retries| retries.get("count").and_then(Value::as_integer))
        .collect();

    assert!(
        retry_counts.iter().copied().any(|count| count >= 1),
        "at least one serial-dylint-ui override must configure retries for \
         Windows CI reliability; found counts: {retry_counts:?}"
    );
}
