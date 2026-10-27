//! Translate injected environment variables into OrthoConfig merge layers.

use std::ffi::OsStr;

use ortho_config::{OrthoResult, ScanEnvSource};
use serde_json::{json, Map, Value};

use crate::{commands::environment_key, error::invalid, source_fallback_override};

/// Collect legacy and canonical environment layers without reading process state.
///
/// Legacy locale is kept in a lower-priority layer so that the canonical name
/// wins independently of the order in which the environment is enumerated.
///
/// # Errors
///
/// Returns an error if a recognized value is non-Unicode or cannot be parsed.
pub(crate) fn collect(source: &dyn ScanEnvSource) -> OrthoResult<(Value, Value, bool)> {
    let mut layers = EnvironmentLayers::default();
    for (key, value) in source.scan() {
        if let Some(key) = key.to_str() {
            layers.insert_variable(key, &value)?;
        }
    }
    Ok((
        Value::Object(layers.legacy),
        Value::Object(layers.canonical),
        layers.has_legacy_locale,
    ))
}

/// Accumulate separate precedence layers while classifying environment values.
#[derive(Default)]
struct EnvironmentLayers {
    legacy: Map<String, Value>,
    canonical: Map<String, Value>,
    has_legacy_locale: bool,
}

impl EnvironmentLayers {
    /// Map one recognized variable into its corresponding source layer.
    fn insert_variable(&mut self, key: &str, value: &OsStr) -> OrthoResult<()> {
        match key {
            "DYLINT_LOCALE" => {
                let text = text_value(key, value)?;
                insert(
                    &mut self.legacy,
                    "diagnostics",
                    "locale",
                    Value::String(text.to_owned()),
                );
                self.has_legacy_locale = true;
            }
            "WHITAKER_NO_SOURCE_FALLBACK" => {
                if let Some(allow) = source_fallback_override(Some(value)) {
                    insert(
                        &mut self.legacy,
                        "install",
                        "allow_build_from_source",
                        json!(allow),
                    );
                }
            }
            _ => self.insert_canonical(key, value)?,
        }
        Ok(())
    }

    /// Map a canonical global or command-specific variable when recognized.
    fn insert_canonical(&mut self, key: &str, value: &OsStr) -> OrthoResult<()> {
        if let Some((command, setting)) = environment_key(key) {
            let canonical_key = format!("WHITAKER_{setting}");
            if let Some((section, field, kind)) = known_key(&canonical_key) {
                let parsed = parse_value(key, text_value(key, value)?, kind)?;
                insert_command(&mut self.canonical, command, (section, field), parsed);
            }
        } else if let Some((section, field, kind)) = known_key(key) {
            let parsed = parse_value(key, text_value(key, value)?, kind)?;
            insert(&mut self.canonical, section, field, parsed);
        }
        Ok(())
    }
}

/// Identify the currently specified global environment names.
fn known_key(key: &str) -> Option<(&'static str, &'static str, Kind)> {
    global_key(key)
        .or_else(|| lint_key(key))
        .or_else(|| tuning_key(key))
}

/// Map global application settings.
fn global_key(key: &str) -> Option<(&'static str, &'static str, Kind)> {
    match key {
        "WHITAKER_LINT_SELECT" => Some(("lint", "select", Kind::List)),
        "WHITAKER_LINT_IGNORE" => Some(("lint", "ignore", Kind::List)),
        "WHITAKER_LINT_EXPERIMENTAL" => Some(("lint", "experimental", Kind::Bool)),
        "WHITAKER_INSTALL_LAZY" => Some(("install", "lazy", Kind::Bool)),
        "WHITAKER_INSTALL_PREFER_PREBUILT" => Some(("install", "prefer_prebuilt", Kind::Bool)),
        "WHITAKER_INSTALL_ALLOW_BUILD_FROM_SOURCE" => {
            Some(("install", "allow_build_from_source", Kind::Bool))
        }
        "WHITAKER_DIAGNOSTICS_LOCALE" => Some(("diagnostics", "locale", Kind::Text)),
        "WHITAKER_OUTPUT_COLOUR" => Some(("output", "colour", Kind::Text)),
        "WHITAKER_OUTPUT_PROGRESS" => Some(("output", "progress", Kind::Text)),
        _ => None,
    }
}

/// Map line, branch, and test-attribute lint settings.
fn lint_key(key: &str) -> Option<(&'static str, &'static str, Kind)> {
    match key {
        "WHITAKER_MODULE_MAX_LINES_MAX_LINES" => {
            Some(("module_max_lines", "max_lines", Kind::Integer))
        }
        "WHITAKER_CONDITIONAL_MAX_N_BRANCHES_MAX_BRANCHES" => {
            Some(("conditional_max_n_branches", "max_branches", Kind::Integer))
        }
        "WHITAKER_NO_EXPECT_OUTSIDE_TESTS_ADDITIONAL_TEST_ATTRIBUTES" => Some((
            "no_expect_outside_tests",
            "additional_test_attributes",
            Kind::List,
        )),
        "WHITAKER_TEST_MUST_NOT_HAVE_EXAMPLE_ADDITIONAL_TEST_ATTRIBUTES" => Some((
            "test_must_not_have_example",
            "additional_test_attributes",
            Kind::List,
        )),
        "WHITAKER_NO_UNWRAP_OR_ELSE_PANIC_ALLOW_IN_MAIN" => {
            Some(("no_unwrap_or_else_panic", "allow_in_main", Kind::Bool))
        }
        _ => None,
    }
}

/// Map fixture, filesystem, and complexity tuning settings.
fn tuning_key(key: &str) -> Option<(&'static str, &'static str, Kind)> {
    match key {
        "WHITAKER_RSTEST_HELPER_SHOULD_BE_FIXTURE_MIN_CALLS" => Some((
            "rstest_helper_should_be_fixture",
            "min_calls",
            Kind::Integer,
        )),
        "WHITAKER_RSTEST_HELPER_SHOULD_BE_FIXTURE_MIN_DISTINCT_TESTS" => Some((
            "rstest_helper_should_be_fixture",
            "min_distinct_tests",
            Kind::Integer,
        )),
        "WHITAKER_RSTEST_HELPER_SHOULD_BE_FIXTURE_REQUIRE_IDENTICAL_FIXTURE_ARG_NAMES" => Some((
            "rstest_helper_should_be_fixture",
            "require_identical_fixture_arg_names",
            Kind::Bool,
        )),
        "WHITAKER_RSTEST_HELPER_SHOULD_BE_FIXTURE_PROVIDER_PARAM_ATTRIBUTES" => Some((
            "rstest_helper_should_be_fixture",
            "provider_param_attributes",
            Kind::List,
        )),
        "WHITAKER_RSTEST_HELPER_SHOULD_BE_FIXTURE_USE_SOURCE_CALLEE_FALLBACK" => Some((
            "rstest_helper_should_be_fixture",
            "use_source_callee_fallback",
            Kind::Bool,
        )),
        "WHITAKER_NO_STD_FS_OPERATIONS_EXCLUDED_CRATES" => {
            Some(("no_std_fs_operations", "excluded_crates", Kind::List))
        }
        "WHITAKER_NO_STD_FS_OPERATIONS_EXCLUDED_PATHS" => {
            Some(("no_std_fs_operations", "excluded_paths", Kind::List))
        }
        "WHITAKER_BUMPY_ROAD_FUNCTION_THRESHOLD" => {
            Some(("bumpy_road_function", "threshold", Kind::Float))
        }
        "WHITAKER_BUMPY_ROAD_FUNCTION_WINDOW" => {
            Some(("bumpy_road_function", "window", Kind::Integer))
        }
        "WHITAKER_BUMPY_ROAD_FUNCTION_MIN_BUMP_LINES" => {
            Some(("bumpy_road_function", "min_bump_lines", Kind::Integer))
        }
        "WHITAKER_BUMPY_ROAD_FUNCTION_INCLUDE_CLOSURES" => {
            Some(("bumpy_road_function", "include_closures", Kind::Bool))
        }
        "WHITAKER_BUMPY_ROAD_FUNCTION_WEIGHTS_DEPTH" => {
            Some(("bumpy_road_function.weights", "depth", Kind::Float))
        }
        "WHITAKER_BUMPY_ROAD_FUNCTION_WEIGHTS_PREDICATE" => {
            Some(("bumpy_road_function.weights", "predicate", Kind::Float))
        }
        "WHITAKER_BUMPY_ROAD_FUNCTION_WEIGHTS_FLOW" => {
            Some(("bumpy_road_function.weights", "flow", Kind::Float))
        }
        _ => None,
    }
}

/// Describe the representation expected for a known environment value.
#[derive(Clone, Copy)]
enum Kind {
    Text,
    Bool,
    List,
    Integer,
    Float,
}

/// Convert an operating-system value without silently losing bytes.
fn text_value<'a>(key: &str, value: &'a OsStr) -> OrthoResult<&'a str> {
    value
        .to_str()
        .ok_or_else(|| invalid(key, "expected Unicode text"))
}

/// Parse one environment value using the corresponding setting's type.
fn parse_value(key: &str, text: &str, kind: Kind) -> OrthoResult<Value> {
    match kind {
        Kind::Text => Ok(json!(text)),
        Kind::Bool => text
            .parse::<bool>()
            .map(|value| json!(value))
            .map_err(|_| invalid(key, "expected true or false")),
        Kind::Integer => text
            .parse::<u64>()
            .map(|value| json!(value))
            .map_err(|_| invalid(key, "expected a non-negative integer")),
        Kind::Float => text
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| json!(value))
            .ok_or_else(|| invalid(key, "expected a finite number")),
        Kind::List => Ok(json!(text
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>())),
    }
}

/// Insert a setting into a nested configuration object.
fn insert(root: &mut Map<String, Value>, section: &str, field: &str, value: Value) {
    if let Some((first, rest)) = section.split_once('.') {
        let child = root.entry(first).or_insert_with(|| json!({}));
        if let Value::Object(fields) = child {
            insert(fields, rest, field, value);
        }
    } else {
        let object = root.entry(section).or_insert_with(|| json!({}));
        if let Value::Object(fields) = object {
            fields.insert(field.to_owned(), value);
        }
    }
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;

/// Insert a value under its command and settings section.
fn insert_command(
    root: &mut Map<String, Value>,
    command: &str,
    setting: (&str, &str),
    value: Value,
) {
    let commands = root.entry("cmds").or_insert_with(|| json!({}));
    if let Value::Object(commands) = commands {
        let command = commands.entry(command).or_insert_with(|| json!({}));
        if let Value::Object(command) = command {
            insert(command, setting.0, setting.1, value);
        }
    }
}
