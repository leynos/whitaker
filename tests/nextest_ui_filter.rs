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
//! *first* matching override that configures it, walking the selected profile
//! and then `default`, so an earlier broad clause silently deprives a later,
//! more specific one of its effect. The second is that the harnesses which
//! rebuild a lint library through nested Cargo keep a build-sized allowance,
//! since they queue behind shared compiler admission.
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

#[path = "nextest_ui_filter/build_allowances.rs"]
mod build_allowances;
#[path = "nextest_ui_filter/config.rs"]
mod config;
#[path = "nextest_ui_filter/errors.rs"]
mod errors;
#[path = "nextest_ui_filter/filter_model.rs"]
mod filter_model;
#[path = "nextest_ui_filter/property.rs"]
mod property;

use filter_model::{BinaryName, PackageName, ProfileName, TestIdentity, TestName};
use rstest::rstest;
use toml::Value;

use self::config::{
    INTEGRATION_UI_CLAUSE,
    SERIAL_UI_GROUP,
    broad_override,
    effective_overrides,
    filter_of,
    governing_override,
    nextest_config,
    slow_timeout_period_secs,
};

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

/// An integration-test binary named `ui` with a top-level `fn ui()`, as
/// nextest reports it: the binary and the test share the name `ui`.
const INTEGRATION_UI: TestIdentity<'static> = TestIdentity {
    package: PackageName("bumpy_road_function"),
    binary: BinaryName("ui"),
    name: TestName("ui"),
};

/// The bound the broad clause actually gives a test it captures, in seconds.
///
/// It was 20 minutes until this guard was extended. The clause also captures
/// the direct-`acquire` lock test, whose wait bound is 25 minutes, so a 20
/// minute ceiling would have killed that test mid-wait and reported a waiter's
/// timeout for a healthy owner. The clause's period is now 30 minutes, which is
/// above the wait bound and below the harnesses' own 75-minute allowance.
const BROAD_CLAUSE_SECS: u64 = 30 * 60;

/// The allowance the package overrides give a nested-Cargo harness.
const PACKAGE_OVERRIDE_SECS: u64 = 75 * 60;

/// The broad clause, not the package overrides, is what captures a plain
/// integration-test binary named `ui`: its reported test name is `ui`, which
/// `test(ui::ui)` misses.
#[rstest]
#[case::default(ProfileName("default"))]
#[case::ci(ProfileName("ci"))]
fn serial_dylint_ui_filter_captures_integration_ui_binaries(
    nextest_config: Value,
    #[case] profile: ProfileName<'_>,
) {
    let filter = filter_of(broad_override(&nextest_config, profile))
        .unwrap_or_else(|| panic!("profile.{}: broad override should filter", profile.0));

    assert!(
        filter
            .selects(INTEGRATION_UI)
            .unwrap_or_else(|error| panic!("profile.{}: {error}", profile.0)),
        "profile.{}: the {SERIAL_UI_GROUP} filter must select an integration binary named `ui` \
         whose top-level test is also named `ui`; found filter: {}",
        profile.0,
        filter.0
    );
    // Selecting the identity is necessary but not sufficient. `test(ui)` would
    // also select it, while dragging in every test whose name merely contains
    // `ui` in every package. The clause must keep the exact-name match.
    assert!(
        filter.0.contains(INTEGRATION_UI_CLAUSE),
        "profile.{}: the {SERIAL_UI_GROUP} filter must contain `{INTEGRATION_UI_CLAUSE}` so the \
         integration-binary clause matches only the test named exactly `ui`; found filter: {}",
        profile.0,
        filter.0
    );
}

/// The serialized UI override must retry, for a Windows-specific reason.
///
/// `dylint_testing::initialize()` can transiently fail with "Access is denied
/// (os error 5)" while persisting a `NamedTempFile`, so a single attempt turns
/// a host-level race into a red build. The count is asserted rather than the
/// override's text, because a retry that was written on some other override
/// would satisfy a text search and still leave these tests with none.
#[rstest]
#[case::default(ProfileName("default"))]
#[case::ci(ProfileName("ci"))]
fn serial_dylint_ui_filter_has_retry_configuration(
    nextest_config: Value,
    #[case] profile: ProfileName<'_>,
) {
    let Some(count) = broad_override(&nextest_config, profile)
        .get("retries")
        .and_then(|retries| retries.get("count"))
        .and_then(Value::as_integer)
    else {
        panic!(
            "profile.{}: the {SERIAL_UI_GROUP} override must configure retries.count for Windows \
             CI",
            profile.0
        );
    };

    assert!(
        count >= 1,
        "profile.{}: retries.count must be at least 1 for Windows CI reliability; found {count}",
        profile.0
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
    let overrides = effective_overrides(&nextest_config, profile);

    let Some((index, governing)) = governing_override(&overrides, "slow-timeout", HARNESS) else {
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

    // Evaluate the governing filter against a harness that does not exist yet.
    // A text search for `binary(no_unwrap_or_else_panic)` also passes when the
    // clause is negated, or when the package is reached only by a name list,
    // and either way the next harness added to the package falls to the base
    // bound. Only selecting an unlisted name proves the whole-binary clause.
    let unlisted = TestIdentity {
        name: TestName("ui::a_harness_added_later"),
        ..HARNESS
    };
    assert!(
        filter
            .selects(unlisted)
            .unwrap_or_else(|error| panic!("profile.{}.overrides[{index}]: {error}", profile.0)),
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
#[case::default_shared_name(ProfileName("default"), SHARED_NAME_CONTROL, BROAD_CLAUSE_SECS)]
#[case::ci_shared_name(ProfileName("ci"), SHARED_NAME_CONTROL, BROAD_CLAUSE_SECS)]
fn effective_slow_timeout_follows_override_precedence(
    nextest_config: Value,
    #[case] profile: ProfileName<'_>,
    #[case] identity: TestIdentity<'_>,
    #[case] expected_secs: u64,
) {
    let overrides = effective_overrides(&nextest_config, profile);

    let Some((index, governing)) = governing_override(&overrides, "slow-timeout", identity) else {
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

    // `test-group` is resolved independently of `slow-timeout`. Reading it from
    // the override that happened to govern the timeout would pass while a
    // different group was actually applied.
    let group = governing_override(&overrides, "test-group", identity)
        .and_then(|(_, over)| over.get("test-group"))
        .and_then(Value::as_str);
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
