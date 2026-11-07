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
//! `ui`, **not** `ui::ui`) and asserts that the nextest filter contains the
//! clause needed to capture that pattern. It also protects the named nested
//! Cargo UI harnesses that share compiler resources on constrained runners.
//!
//! The group is spread over several overrides, each carrying the allowance its
//! own harnesses need, so the clauses are checked against the union of their
//! filters rather than against whichever one happens to come first.

use std::fs;

use rstest::{fixture, rstest};
use toml::Value;

/// Nested Cargo UI harnesses that must share the serial group so their
/// compiler resources are serialized on constrained runners.
const NESTED_CARGO_UI_FILTER_CLAUSES: [&str; 5] = [
    "test(example_compiles_without_diagnostics)",
    "test(example_harness_collects_call_site_evidence)",
    "test(trybuild_fixtures_compile_without_diagnostics)",
    "test(ui::hand_written_test_companion_does_not_exempt_parent_function)",
    "test(ui::aliased_test_crate_non_companion_does_not_exempt_parent_function)",
];

mod workspace_support;

use workspace_support::workspace_root;

/// Parses `.config/nextest.toml` into a [`Value`].
fn load_nextest_config() -> Value {
    let config_path = workspace_root().join(".config/nextest.toml");
    let contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", config_path.display()));
    toml::from_str(&contents)
        .unwrap_or_else(|err| panic!(".config/nextest.toml should parse as TOML: {err}"))
}

/// Returns every override that names the `serial-dylint-ui` group.
///
/// The group is declared by more than one override: the broad UI filter and
/// the narrower nested-Cargo allowances that need a longer timeout than the
/// broad filter's 20 minutes. A test is serialized when *any* override that
/// matches it names the group, so the guard must consider them together.
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
        "at least one override should name the serial-dylint-ui group"
    );
    overrides
}

/// Returns the filter expression from an override.
fn extract_filter(ui_override: &Value) -> &str {
    ui_override
        .get("filter")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("serial-dylint-ui override should have a filter string"))
}

/// Returns the broad UI override, identified by its `driver::ui` clause.
///
/// The broad override is the one that carries the retry policy and the
/// `(binary(ui) & test(=ui))` clause; the narrower allowances below it exist
/// only to extend a timeout.
fn find_broad_ui_override<'a>(overrides: &'a [&'a Value]) -> &'a Value {
    overrides
        .iter()
        .copied()
        .find(|o| extract_filter(o).contains("test(driver::ui::)"))
        .unwrap_or_else(|| panic!("the broad serial-dylint-ui override should exist"))
}

/// Discovers lint crates under `crates/` whose UI test is an integration test
/// file at `tests/ui.rs`.
///
/// These crates produce a nextest binary named `ui` with a top-level test
/// name of `ui`.  The substring match `test(ui::ui)` does **not** capture
/// them because the reported test name is plain `ui`, not `ui::ui`.
fn crates_with_integration_ui_test() -> Vec<String> {
    let crates_dir = workspace_root().join("crates");

    let entries = fs::read_dir(&crates_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", crates_dir.display()));

    let mut crate_names: Vec<String> = entries
        .filter_map(|dir_entry| {
            let path = dir_entry
                .unwrap_or_else(|err| panic!("directory entry should be readable: {err}"))
                .path();
            if path.is_dir() && path.join("tests/ui.rs").is_file() {
                Some(
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_else(|| panic!("crate directory should have a UTF-8 name"))
                        .to_owned(),
                )
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
    let broad = find_broad_ui_override(&overrides);
    let filter = extract_filter(broad);
    let crates = crates_with_integration_ui_test();

    assert!(
        !crates.is_empty(),
        "expected at least one lint crate with tests/ui.rs"
    );

    // The integration-test pattern produces binary=ui, test=ui. The filter
    // must constrain that test to the `ui` binary — historically
    // `test(ui::ui)` missed it because the test name is plain `ui`. The binary
    // clause may group this exact test with other tests from the same binary.
    let matches_ui_binary_test = filter.contains("(binary(ui) & test(=ui))")
        || filter.contains("(binary(ui) & (test(=ui) | ");

    assert!(
        matches_ui_binary_test,
        "the serial-dylint-ui filter must constrain `test(=ui)` to the `ui` binary to capture \
         integration test binaries named `ui` with a top-level `fn ui()` (e.g. {crates:?}); found \
         filter: {filter}"
    );

    for filter_clause in NESTED_CARGO_UI_FILTER_CLAUSES {
        let captured = overrides
            .iter()
            .any(|o| extract_filter(o).contains(filter_clause));
        assert!(
            captured,
            "some serial-dylint-ui override must capture nested Cargo UI test `{filter_clause}` \
             so its compiler resources are serialized on constrained runners; found filters: {:?}",
            overrides
                .iter()
                .map(|o| extract_filter(o))
                .collect::<Vec<_>>()
        );
    }
}

#[rstest]
fn serial_dylint_ui_filters_have_retry_configuration(serial_dylint_ui_overrides: Vec<Value>) {
    let overrides: Vec<&Value> = serial_dylint_ui_overrides.iter().collect();
    let broad = find_broad_ui_override(&overrides);
    // On Windows, dylint_testing::initialize() can transiently fail with
    // "Access is denied (os error 5)" during NamedTempFile::persist().
    // Retries are essential for reliable CI on Windows.
    let Some(retries) = broad.get("retries") else {
        panic!("serial-dylint-ui override must configure retries for Windows CI reliability");
    };

    let Some(count) = retries.get("count").and_then(Value::as_integer) else {
        panic!("retries.count should be a positive integer");
    };

    assert!(
        count >= 1,
        "retries.count must be at least 1 for Windows CI reliability; found {count}"
    );
}
