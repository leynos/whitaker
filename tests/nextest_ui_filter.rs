//! Guards against the nextest filter for `serial-dylint-ui` silently missing
//! new dylint UI test binaries.
//!
//! The `serial-dylint-ui` test-group in `.config/nextest.toml` serialises all
//! dylint UI tests so they do not race on the shared target directory or on
//! Windows temporary-file handles.  If a lint crate adds a UI test whose
//! fully-qualified name does not match the filter, the test will run without
//! serialisation or retries, causing flaky Windows CI failures.
//!
//! This test scans for all lint crates that declare a `tests/ui.rs` integration
//! test (which produces a binary named `ui` with a top-level test name of
//! `ui`, **not** `ui::ui`) and asserts that the nextest filter contains the
//! clause needed to capture that pattern.

use std::fs;
use std::path::Path;

use toml::Value;

/// Parses `.config/nextest.toml` into a [`Value`].
fn load_nextest_config() -> Value {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".config/nextest.toml");
    let contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", config_path.display()));
    toml::from_str(&contents)
        .unwrap_or_else(|err| panic!(".config/nextest.toml should parse as TOML: {err}"))
}

/// Returns the `serial-dylint-ui` override table from the config.
fn find_serial_dylint_ui_override(config: &Value) -> &Value {
    let Some(all_overrides) = config
        .get("profile")
        .and_then(|p| p.get("default"))
        .and_then(|d| d.get("overrides"))
        .and_then(Value::as_array)
    else {
        panic!("profile.default.overrides should be an array");
    };

    all_overrides
        .iter()
        .find(|o| {
            o.get("test-group")
                .and_then(Value::as_str)
                .is_some_and(|group| group == "serial-dylint-ui")
        })
        .unwrap_or_else(|| panic!("serial-dylint-ui override should exist"))
}

/// Returns the filter expression from the `serial-dylint-ui` override.
fn extract_filter(ui_override: &Value) -> &str {
    ui_override
        .get("filter")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("serial-dylint-ui override should have a filter string"))
}

/// Discovers lint crates under `crates/` whose UI test is an integration test
/// file at `tests/ui.rs`.
///
/// These crates produce a nextest binary named `ui` with a top-level test
/// name of `ui`.  The substring match `test(ui::ui)` does **not** capture
/// them because the reported test name is plain `ui`, not `ui::ui`.
fn crates_with_integration_ui_test() -> Vec<String> {
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates");

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

#[test]
fn serial_dylint_ui_filter_captures_integration_ui_binaries() {
    let config = load_nextest_config();
    let ui_override = find_serial_dylint_ui_override(&config);
    let filter = extract_filter(ui_override);
    let crates = crates_with_integration_ui_test();

    assert!(
        !crates.is_empty(),
        "expected at least one lint crate with tests/ui.rs"
    );

    // The integration-test pattern produces binary=ui, test=ui.  The filter
    // must contain a clause that matches this pattern — historically
    // `test(ui::ui)` missed it because the test name is plain `ui`.
    assert!(
        filter.contains("binary(ui)") && filter.contains("test(=ui)"),
        "the serial-dylint-ui filter must contain `(binary(ui) & test(=ui))` \
         to capture integration test binaries named `ui` with a top-level \
         `fn ui()` (e.g. {crates:?}); found filter: {filter}"
    );
}

#[test]
fn serial_dylint_ui_filter_has_retry_configuration() {
    let config = load_nextest_config();
    let ui_override = find_serial_dylint_ui_override(&config);

    // On Windows, dylint_testing::initialize() can transiently fail with
    // "Access is denied (os error 5)" during NamedTempFile::persist().
    // Retries are essential for reliable CI on Windows.
    let Some(retries) = ui_override.get("retries") else {
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
