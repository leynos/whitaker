//! Guards against the nextest overrides for dylint UI harnesses silently
//! missing or shadowing a harness.
//!
//! The `serial-dylint-ui` test-group in `.config/nextest.toml` serializes all
//! dylint UI tests so they do not race on the shared target directory or on
//! Windows temporary-file handles. If a lint crate adds a UI test whose
//! fully-qualified name does not match the filter, the test runs without
//! serialization or retries, causing flaky Windows CI failures.
//!
//! Two properties are pinned here, because a broken one is invisible by
//! inspection. The first is precedence: nextest resolves each setting from the
//! *first* matching override that configures it, so an earlier broad clause
//! silently deprives a later, more specific one of its effect. The second is
//! that the harnesses which rebuild a lint library through nested Cargo keep a
//! build-sized allowance, since they queue behind shared compiler admission.
//!
//! `no_unwrap_or_else_panic` re-exports its UI module as `mod ui`
//! (`src/lib.rs`), so nextest reports its tests as `ui::<name>`; its filter
//! therefore has to select the *package's own* binary rather than a
//! `tests/ui.rs` integration binary named `ui`.
//!
//! The filter grammar is evaluated by [`filter_model`] against concrete
//! identities rather than searched as text, because a text search cannot tell
//! "this clause matches the test" from "this clause matches a different test
//! with the same name in another package" — which is exactly how the shadowing
//! went unnoticed.

#[path = "nextest_ui_filter/filter_model.rs"]
mod filter_model;

use std::{fs, path::Path};

use filter_model::{
    BinaryName,
    FilterExpression,
    PackageName,
    ProfileName,
    TestIdentity,
    TestName,
};
use rstest::{fixture, rstest};
use toml::Value;

/// The crate whose nested-Cargo harnesses timed out.
const HARNESS_PACKAGE: PackageName<'static> = PackageName("no_unwrap_or_else_panic");

/// A harness in [`HARNESS_PACKAGE`], named as nextest reports it.
const HARNESS: TestIdentity<'static> = TestIdentity {
    package: HARNESS_PACKAGE,
    binary: BinaryName("no_unwrap_or_else_panic"),
    name: TestName("ui::example_compiles_under_test_harness"),
};

/// The harness in `rstest_helper_should_be_fixture` that the broad clause used
/// to capture: the crate's integration binary is `ui`, but this test rebuilds
/// its lint library through nested Cargo like the harnesses above.
const RSTEST_HELPER_HARNESS: TestIdentity<'static> = TestIdentity {
    package: PackageName("rstest_helper_should_be_fixture"),
    binary: BinaryName("ui"),
    name: TestName("example_harness_collects_call_site_evidence"),
};

/// A crate that declares a test with the *same name* as [`HARNESS`] but has no
/// override of its own. It is the negative control: an earlier broad clause
/// must still reach it, and a filter test that cannot see that is not testing
/// precedence at all.
const SHARED_NAME_CONTROL: TestIdentity<'static> = TestIdentity {
    package: PackageName("no_expect_outside_tests"),
    binary: BinaryName("no_expect_outside_tests"),
    name: TestName("ui::example_compiles_under_test_harness"),
};

/// The test-group every dylint UI harness must be serialized into.
const SERIAL_UI_GROUP: &str = "serial-dylint-ui";

/// The minimum period that counts as a build-sized allowance, in seconds.
///
/// Nested Cargo builds queue behind shared compiler admission and package
/// caches; twenty minutes is the smallest budget the repository has needed.
/// Anything shorter is a hang detector, not an allowance.
const BUILD_SLOW_TIMEOUT_SECS: u64 = 20 * 60;

/// The allowance the package overrides give a nested-Cargo harness.
const PACKAGE_OVERRIDE_SECS: u64 = 75 * 60;

/// Parses `.config/nextest.toml` into a [`Value`].
fn load_nextest_config() -> Value {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".config/nextest.toml");
    let contents = fs::read_to_string(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", config_path.display()));
    toml::from_str(&contents)
        .unwrap_or_else(|err| panic!(".config/nextest.toml should parse as TOML: {err}"))
}

/// Fixture providing the parsed `nextest` config.
#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn nextest_config() -> Value { load_nextest_config() }

/// Returns the ordered override tables for `profile`.
fn profile_overrides<'a>(config: &'a Value, profile: ProfileName<'_>) -> &'a [Value] {
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

/// Returns the broad `serial-dylint-ui` override, which is first in the
/// default profile because every clause below it is more specific.
fn broad_override(config: &Value) -> &Value {
    profile_overrides(config, ProfileName("default"))
        .first()
        .unwrap_or_else(|| panic!("the default profile should declare a broad overrides entry"))
}

/// Reads an override's `filter` string, if it has one. An override without a
/// filter matches every test.
fn filter_of(override_table: &Value) -> Option<FilterExpression<'_>> {
    override_table
        .get("filter")
        .and_then(Value::as_str)
        .map(FilterExpression)
}

/// Returns the index and override that nextest applies for `setting`, which is
/// the first override, in file order, that matches `identity` and configures
/// that setting.
fn governing_override<'a>(
    overrides: &'a [Value],
    setting: &str,
    identity: TestIdentity<'_>,
) -> Option<(usize, &'a Value)> {
    overrides.iter().enumerate().find(|(_, over)| {
        over.get(setting).is_some() && filter_of(over).is_none_or(|filter| filter.selects(identity))
    })
}

/// Reads the `period` of an override's `slow-timeout`, in seconds.
fn slow_timeout_period_secs(overrides: &Value) -> Option<u64> {
    let period = overrides.get("slow-timeout")?.get("period")?.as_str()?;
    Some(parse_duration_secs(period))
}

/// Parses the nextest integer-with-suffix duration form, e.g. `"75m"`.
///
/// The suffixes are exactly the three below, each mapping to seconds; an
/// unknown one is a configuration bug.
fn parse_duration_secs(period: &str) -> u64 {
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

#[rstest]
fn serial_dylint_ui_filter_captures_integration_ui_binaries(nextest_config: Value) {
    let filter = filter_of(broad_override(&nextest_config))
        .unwrap_or_else(|| panic!("broad override should filter"));

    // The integration-test pattern produces binary=ui, test=ui, so the filter
    // must match it; `test(ui::ui)` misses because the reported name is `ui`.
    assert!(
        filter.0.contains("(binary(ui) & test(=ui))"),
        "the serial-dylint-ui filter must contain `(binary(ui) & test(=ui))` to capture \
         integration test binaries named `ui` with a top-level `fn ui()`; found filter: {}",
        filter.0
    );
}

#[rstest]
fn serial_dylint_ui_filter_has_retry_configuration(nextest_config: Value) {
    // On Windows, dylint_testing::initialize() can transiently fail with
    // "Access is denied (os error 5)" during NamedTempFile::persist().
    // Retries are essential for reliable CI on Windows.
    let Some(count) = broad_override(&nextest_config)
        .get("retries")
        .and_then(|retries| retries.get("count"))
        .and_then(Value::as_integer)
    else {
        panic!("serial-dylint-ui override must configure retries.count for Windows CI");
    };

    assert!(
        count >= 1,
        "retries.count must be at least 1 for Windows CI reliability; found {count}"
    );
}

/// A name list is what rotted: it silently stopped reaching harnesses that were
/// added later. Selecting the package's own binary covers every test in it
/// without an enumerable list, and this pins that the *governing* override, not
/// merely some override, is the one that does so.
#[rstest]
#[case::default(ProfileName("default"))]
#[case::ci(ProfileName("ci"))]
fn nested_cargo_harnesses_are_governed_by_the_whole_package_override(
    nextest_config: Value,
    #[case] profile: ProfileName<'_>,
) {
    let overrides = profile_overrides(&nextest_config, profile);

    let Some((index, governing)) = governing_override(overrides, "slow-timeout", HARNESS) else {
        panic!(
            "profile.{}: no override with a slow-timeout selects {} {}, so its nested-Cargo \
             harnesses fall to the base bound",
            profile.0, HARNESS.binary.0, HARNESS.name.0
        );
    };

    let Some(filter) = filter_of(governing) else {
        panic!(
            "profile.{}.overrides[{index}] should have a filter",
            profile.0
        );
    };

    assert!(
        filter.0.contains(&format!("binary({HARNESS_PACKAGE})")),
        "profile.{}.overrides[{index}] governs {} by name list rather than the whole package \
         binary, so a new harness would silently fall to the base bound; found filter: {}",
        profile.0,
        HARNESS.name.0,
        filter.0
    );
}

/// Pins the *effective* allowance for each nested-Cargo harness, which is what
/// nextest actually applies. Checking that some override mentions a package is
/// not enough: an earlier broad clause that also matches the test wins the
/// setting, and the specific override below never takes effect.
///
/// [`SHARED_NAME_CONTROL`] is the negative control. It has the same test name
/// as [`HARNESS`] in a package with no override of its own, so it must still be
/// governed by the broad clause. If the matcher were written to ignore
/// precedence, or if the broad clause were weakened until it matched nothing,
/// this case fails.
#[rstest]
#[case::default_package_override(ProfileName("default"), HARNESS, PACKAGE_OVERRIDE_SECS)]
#[case::ci_package_override(ProfileName("ci"), HARNESS, PACKAGE_OVERRIDE_SECS)]
#[case::default_rstest_helper(ProfileName("default"), RSTEST_HELPER_HARNESS, PACKAGE_OVERRIDE_SECS)]
#[case::ci_rstest_helper(ProfileName("ci"), RSTEST_HELPER_HARNESS, PACKAGE_OVERRIDE_SECS)]
#[case::default_shared_name(ProfileName("default"), SHARED_NAME_CONTROL, BUILD_SLOW_TIMEOUT_SECS)]
#[case::ci_shared_name(ProfileName("ci"), SHARED_NAME_CONTROL, BUILD_SLOW_TIMEOUT_SECS)]
fn effective_slow_timeout_follows_override_precedence(
    nextest_config: Value,
    #[case] profile: ProfileName<'_>,
    #[case] identity: TestIdentity<'_>,
    #[case] expected_secs: u64,
) {
    let overrides = profile_overrides(&nextest_config, profile);

    let Some((index, governing)) = governing_override(overrides, "slow-timeout", identity) else {
        panic!(
            "profile.{}: no override with a slow-timeout selects {} {}, so it falls to the base \
             bound instead of the {expected_secs}s allowance",
            profile.0, identity.binary.0, identity.name.0
        );
    };

    let period = slow_timeout_period_secs(governing).unwrap_or_else(|| {
        panic!(
            "profile.{}.overrides[{index}] should set slow-timeout.period",
            profile.0
        )
    });
    assert_eq!(
        period, expected_secs,
        "profile.{}.overrides[{index}] gives {} {} a {period}s slow-timeout, not the expected \
         {expected_secs}s. nextest resolves a setting from the *first* matching override that \
         configures it, so a broad clause listed above a package override shadows it.",
        profile.0, identity.binary.0, identity.name.0
    );

    let group = governing.get("test-group").and_then(Value::as_str);
    assert_eq!(
        group,
        Some(SERIAL_UI_GROUP),
        "profile.{}.overrides[{index}] governs {} {} but its test-group is {group:?}; \
         nested-Cargo UI harnesses must be serialized in {SERIAL_UI_GROUP}",
        profile.0,
        identity.binary.0,
        identity.name.0
    );
}
