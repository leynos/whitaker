//! Shared helpers for the nextest override guards.
//!
//! These load `.config/nextest.toml` and resolve which override actually
//! governs a given test. nextest applies each setting from the *first*
//! matching override that configures it, walking the selected profile and then
//! `default`, so "some override mentions this test" is not the question these
//! guards need answered. Keeping the resolution here lets every guard ask the
//! question the way nextest does.

use std::{fs, path::Path};

use rstest::fixture;
use toml::Value;

use super::filter_model::{FilterExpression, ProfileName, TestIdentity};

/// The test-group every dylint UI harness must be serialized into.
pub const SERIAL_UI_GROUP: &str = "serial-dylint-ui";

/// The clause that captures an integration-test binary named `ui`.
pub const INTEGRATION_UI_CLAUSE: &str = "(binary(ui) & test(=ui))";

/// Parses `.config/nextest.toml` into a [`Value`].
pub fn load_nextest_config() -> Value {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".config/nextest.toml");
    let contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", config_path.display()));
    toml::from_str(&contents)
        .unwrap_or_else(|err| panic!(".config/nextest.toml should parse as TOML: {err}"))
}

/// Fixture providing the parsed `nextest` config.
#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
pub fn nextest_config() -> Value { load_nextest_config() }

/// Returns the ordered override tables for `profile`.
pub fn profile_overrides<'a>(config: &'a Value, profile: ProfileName<'_>) -> &'a [Value] {
    let Some(overrides) = config
        .get("profile")
        .and_then(|p| p.get(profile.0))
        .and_then(|d| d.get("overrides"))
        .and_then(Value::as_array)
    else {
        panic!("profile.{}: overrides should be an array", profile.0);
    };

    overrides.as_slice()
}

/// Returns the ordered overrides nextest consults for `profile`: its own
/// entries, followed by the `default` profile's entries.
///
/// A custom profile inherits `default`, and nextest chooses each setting
/// independently from the first override that configures it along that chain.
/// A setting only `default` configures therefore still applies under `ci`, so
/// a guard that read the selected profile alone would report a missing
/// allowance that nextest does in fact apply.
pub fn effective_overrides(config: &Value, profile: ProfileName<'_>) -> Vec<Value> {
    let selected = profile_overrides(config, profile);
    // `default` has no profile above it to inherit from, so chaining it to
    // itself would offer every entry twice and make "first match wins" depend on
    // an accident of list position.
    let inherited: &[Value] = if profile.0 == "default" {
        &[]
    } else {
        profile_overrides(config, ProfileName("default"))
    };

    selected.iter().chain(inherited).cloned().collect()
}

/// Returns the broad `serial-dylint-ui` override for `profile`.
///
/// The clause is located by its test-group and the integration-UI disjunct it
/// carries rather than by position: the broad override is first today, and
/// reordering the file must not silently retarget this guard at a different
/// table.
pub fn broad_override<'a>(config: &'a Value, profile: ProfileName<'_>) -> &'a Value {
    profile_overrides(config, profile)
        .iter()
        .find(|over| {
            over.get("test-group").and_then(Value::as_str) == Some(SERIAL_UI_GROUP)
                && filter_of(over).is_some_and(|filter| filter.0.contains(INTEGRATION_UI_CLAUSE))
        })
        .unwrap_or_else(|| {
            panic!(
                "profile.{} should declare a broad {SERIAL_UI_GROUP} override containing \
                 `{INTEGRATION_UI_CLAUSE}`",
                profile.0
            )
        })
}

/// Reads an override's `filter` string, if it has one. An override without a
/// filter matches every test.
///
/// A `filter` key that is present but not a string is a configuration error,
/// not an absent filter. Mapping it to `None` would make such an override
/// match every test unconditionally, which is the opposite of what a filter
/// that failed to parse should do.
///
/// # Panics
///
/// Panics when `filter` is present but is not a string. The guard is a test
/// harness, so a malformed configuration should stop it at the offending
/// override rather than be threaded through every caller.
pub fn filter_of(override_table: &Value) -> Option<FilterExpression<'_>> {
    let filter = override_table.get("filter")?;
    let Some(text) = filter.as_str() else {
        panic!("an override's `filter` must be a string; found {filter:?}");
    };

    Some(FilterExpression(text))
}

/// Returns the index and override that nextest applies for `setting`, which is
/// the first override, in file order, that matches `identity` and configures
/// that setting.
///
/// # Panics
///
/// Panics when an override's filter cannot be parsed, rather than treating that
/// override as matching every test. The guard exists to detect configuration
/// drift, and a clause it cannot read is exactly such drift.
pub fn governing_override<'a>(
    overrides: &'a [Value],
    setting: &str,
    identity: TestIdentity<'_>,
) -> Option<(usize, &'a Value)> {
    overrides.iter().enumerate().find(|(_, over)| {
        over.get(setting).is_some()
            && filter_of(over).is_none_or(|filter| {
                filter
                    .selects(identity)
                    .unwrap_or_else(|error| panic!("{error}"))
            })
    })
}

/// Reads the `period` of an override's `slow-timeout`, in seconds.
pub fn slow_timeout_period_secs(overrides: &Value) -> Option<u64> {
    let period = overrides.get("slow-timeout")?.get("period")?.as_str()?;
    Some(parse_duration_secs(period))
}

/// Parses the nextest integer-with-suffix duration form, e.g. `"75m"`.
///
/// The suffixes are exactly the three below, each mapping to seconds; an
/// unknown one is a configuration bug.
pub fn parse_duration_secs(period: &str) -> u64 {
    const SUFFIXES: [(char, u64); 3] = [('s', 1), ('m', 60), ('h', 60 * 60)];

    let Some((suffix, multiplier)) = SUFFIXES
        .iter()
        .find(|(suffix, _)| period.ends_with(*suffix))
    else {
        panic!("slow-timeout period should end in s, m, or h; found {period:?}");
    };

    let digits = period
        .strip_suffix(*suffix)
        .unwrap_or_else(|| panic!("period {period:?} should end in {suffix:?}"));

    digits
        .parse::<u64>()
        .unwrap_or_else(|err| panic!("slow-timeout period {period:?} should be an integer: {err}"))
        * multiplier
}
