//! Guards for the build-sized overrides that size a nested-Cargo build.
//!
//! A nested-Cargo harness that falls to the 300 s base bound is killed
//! mid-build, and the failure it reports is a timeout rather than the compile
//! error it was waiting on. Two things therefore have to hold, and neither is
//! visible by reading the filter text: the *effective* allowance must be
//! build-sized, and the test must be serialized into the group that owns its
//! target directory.
//!
//! Checking that an override *exists* is not enough, and neither is matching on
//! its text. nextest resolves each setting from the first override that
//! configures it, so a broad clause listed above a package override shadows it
//! whatever the text says. These guards ask for the resolved value instead.

use rstest::rstest;
use toml::Value;

use super::{
    config::{effective_overrides, governing_override, nextest_config, slow_timeout_period_secs},
    filter_model::{BinaryName, PackageName, ProfileName, TestIdentity, TestName},
};

/// The `trybuild` compile contract in `installer/tests/ui.rs`.
///
/// This is a top-level `fn` in the file, so nextest reports it without the
/// `ui::` module prefix that the package's own harnesses produce. That
/// difference matters: the broad clause's `test(ui::ui)` term is a *contains*
/// match and does not select this name, which is why its own override is
/// reachable.
const SHA2_CONTRACT: TestIdentity<'static> = TestIdentity {
    package: PackageName("whitaker-installer"),
    binary: BinaryName("ui"),
    name: TestName("sha2_0_11_pre_migration_patterns_fail_to_compile"),
};

/// The package-isolated packaging check in `common/tests/i18n_packaging.rs`,
/// which `#[cfg(unix)] mod unix` nests, so nextest reports the `unix::` prefix.
///
/// Like [`SHA2_CONTRACT`] this name has no `ui::` prefix, so the broad clause's
/// `ui::` terms do not reach it.
const PACKAGING_CHECK: TestIdentity<'static> = TestIdentity {
    package: PackageName("whitaker-common"),
    binary: BinaryName("i18n_packaging"),
    name: TestName("unix::fluent_bundles_are_included_in_the_package_tarball"),
};

/// The macro-free score-parity check in `macro_aware_cognitive_complexity`.
///
/// Its `tests/parity.rs` integration binary builds the lint library, then runs
/// a nested `cargo clippy` and `cargo dylint` over a generated fixture, so it
/// needs a build-sized allowance and the nested-build lane.
const PARITY_CHECK: TestIdentity<'static> = TestIdentity {
    package: PackageName("macro_aware_cognitive_complexity"),
    binary: BinaryName("parity"),
    name: TestName("macro_free_scores_match_pinned_clippy"),
};

/// The per-test bound a profile applies when no override selects a test, read
/// from each profile's `slow-timeout.period` in `.config/nextest.toml`.
const BASE_SLOW_TIMEOUT_SECS: u64 = 300;

/// The allowance the `trybuild` compile contracts need for a cold fixture
/// build through nested Cargo.
///
/// 120 minutes, not the 75 this guard was first written against. Mainline
/// raised the `.config/nextest.toml` period after a cold build was terminated
/// 47 seconds into the first case, and the comment above that clause records
/// why: a cold build and the cases after it must both fit. This guard pins the
/// value the config actually sets, so the two move together — it exists to
/// catch a shadowing override, not to second-guess the budget.
const SHA2_ALLOWANCE_SECS: u64 = 120 * 60;

/// The allowance the packaging check needs, because `cargo package` builds into
/// its own fresh target directory.
const PACKAGING_ALLOWANCE_SECS: u64 = 30 * 60;

/// The allowance the parity check needs for three nested Cargo builds.
const PARITY_ALLOWANCE_SECS: u64 = 30 * 60;

/// The group the `trybuild` compile contracts serialize in.
///
/// Mainline moved the six compile contracts out of `serial-dylint-ui` into this
/// group so they run beside the Dylint UI harnesses instead of queueing behind
/// them. `sha2_0_11_pre_migration_patterns_fail_to_compile` is one of the six,
/// and `serial-trybuild-contracts` has capacity one, which is what keeps its
/// nested Cargo build from racing another fixture build over one target
/// directory.
const TRYBUILD_CONTRACT_GROUP: &str = "serial-trybuild-contracts";

/// The group the package-isolated and feature-disabled builds serialize in.
///
/// Separate from the Dylint UI group because these tests share no target
/// directory with the Dylint UI harnesses; putting them in one group would
/// serialize two independent build queues against each other.
const NESTED_BUILD_GROUP: &str = "serial-nested-cargo-builds";

/// The allowance each build-sized override must actually give its test.
///
/// One struct rather than a parameter per `#[case]` column: the columns only
/// mean anything together, and `rstest` expands every case parameter into a
/// function argument.
struct Allowance {
    /// The test the override is written for.
    identity: TestIdentity<'static>,
    /// The `slow-timeout.period` nextest must resolve, in seconds.
    secs: u64,
    /// The test-group that serializes the build's target directory.
    group: &'static str,
    /// The `retries.count` nextest must resolve, when the override pins one.
    retries: Option<i64>,
}

/// Pins the build-sized allowances these tests need, in both profiles.
///
/// The `ci` profile budgeted 75 minutes for the `trybuild` compile contracts
/// and 30 for the packaging check, but `default` — which `make test` and
/// `make coverage` run — had no entry for either, so both were held to the
/// 300 s base and killed mid-build. The parity check's override exists in both
/// profiles; it is pinned here so a shadowing clause cannot silently retarget
/// it.
///
/// These tests are named without a `ui::` module prefix, so the broad clause's
/// `test(ui::ui)` term does not select them today; but that is a property of
/// the *names*, and an earlier override that did select them would silently win
/// the setting. Assert the effective allowance, which is what nextest applies.
///
/// The `sha2` contract has since moved out of `serial-dylint-ui` into
/// `serial-trybuild-contracts`, beside the other five compile contracts. Its
/// allowance is unchanged by that move; its group is not, and the group is
/// asserted here because it is what stops two fixture builds sharing a target
/// directory.
#[rstest]
#[case::default_sha2(
    ProfileName("default"),
    Allowance { identity: SHA2_CONTRACT, secs: SHA2_ALLOWANCE_SECS, group: TRYBUILD_CONTRACT_GROUP, retries: Some(0) }
)]
#[case::ci_sha2(
    ProfileName("ci"),
    Allowance { identity: SHA2_CONTRACT, secs: SHA2_ALLOWANCE_SECS, group: TRYBUILD_CONTRACT_GROUP, retries: Some(0) }
)]
#[case::default_packaging(
    ProfileName("default"),
    Allowance { identity: PACKAGING_CHECK, secs: PACKAGING_ALLOWANCE_SECS, group: NESTED_BUILD_GROUP, retries: None }
)]
#[case::ci_packaging(
    ProfileName("ci"),
    Allowance { identity: PACKAGING_CHECK, secs: PACKAGING_ALLOWANCE_SECS, group: NESTED_BUILD_GROUP, retries: None }
)]
#[case::default_parity(
    ProfileName("default"),
    Allowance { identity: PARITY_CHECK, secs: PARITY_ALLOWANCE_SECS, group: NESTED_BUILD_GROUP, retries: Some(0) }
)]
#[case::ci_parity(
    ProfileName("ci"),
    Allowance { identity: PARITY_CHECK, secs: PARITY_ALLOWANCE_SECS, group: NESTED_BUILD_GROUP, retries: Some(0) }
)]
fn build_sized_allowances_govern_their_tests(
    nextest_config: Value,
    #[case] profile: ProfileName<'_>,
    #[case] allowance: Allowance,
) {
    let Allowance {
        identity,
        secs: expected_secs,
        group: expected_group,
        retries: expected_retries,
    } = allowance;
    let overrides = effective_overrides(&nextest_config, profile);

    let Some((index, governing)) = governing_override(&overrides, "slow-timeout", identity) else {
        panic!(
            "profile.{}: no override with a slow-timeout selects {}, so it falls to the {}s base \
             bound instead of the {expected_secs}s allowance",
            profile.0, identity.name.0, BASE_SLOW_TIMEOUT_SECS
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
        "profile.{}.overrides[{index}] gives {} a {period}s slow-timeout, not the expected \
         {expected_secs}s. nextest resolves a setting from the *first* matching override that \
         configures it, so a broad clause listed above it shadows the allowance.",
        profile.0, identity.name.0
    );

    // Each setting is resolved on its own. Reading the group from the override
    // that governed the timeout, or the retries from the one that governs the
    // group, would pass while nextest applied a different value: a later
    // override can be the first to configure one setting without being first
    // for another.
    let group = governing_override(&overrides, "test-group", identity)
        .and_then(|(_, over)| over.get("test-group"))
        .and_then(Value::as_str);
    assert_eq!(
        group,
        Some(expected_group),
        "profile.{}: {} must be serialized in {expected_group}; the governing test-group is \
         {group:?}. A missing group lets concurrent builds race one target directory, and a \
         different group serializes it against unrelated work.",
        profile.0,
        identity.name.0
    );

    if let Some(pinned) = expected_retries {
        let retries = governing_override(&overrides, "retries", identity)
            .and_then(|(_, over)| over.get("retries"))
            .and_then(Value::as_integer);
        assert_eq!(
            retries,
            Some(pinned),
            "profile.{}: {} must not retry, because repeating a 75-minute fixture build only \
             reproduces the same compile error and lengthens the queue for every other agent; the \
             governing retries setting is {retries:?}",
            profile.0,
            identity.name.0
        );
    }
}
