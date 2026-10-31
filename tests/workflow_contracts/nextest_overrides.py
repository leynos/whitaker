"""The nextest overrides and test groups this repository requires, by value.

Split from ``nextest_profile_test`` so no module outgrows the 400-line
limit ``AGENTS.md`` sets: the tables below are data and read as data,
while the assertions over them are short. Both the profile contract and
the compile-contract discovery import these tables rather than restating
them, so neither reader can be satisfied by narrowing what the other
sees.
"""

from __future__ import annotations

import typing as typ


# Parser-pin fixtures share one integration binary and Cargo package cache, so
# the allowance must cover that whole binary and serialize it.
BUILD_SCRIPT_INTEGRATION_FILTER: typ.Final[str] = "binary(build_script_integration)"
NO_DEFAULT_FEATURES_FILTER: typ.Final[str] = "binary(no_default_features_build)"
COMPILE_CONTRACT_FILTER: typ.Final[str] = (
    "test(dependency_check_result_contract_is_public) | "
    "test(fixture_expansion_lint_attribute_contract) | "
    "test(normalize_settings_is_public_and_the_legacy_name_stays_removed) | "
    "test(renamed_common_apis_are_public_and_legacy_names_stay_removed) | "
    "test(renamed_sarif_api_is_public_and_legacy_names_stay_removed) | "
    "test(sha2_0_11_pre_migration_patterns_fail_to_compile)"
)
NESTED_TRYBUILD_TEST_FILTER: typ.Final[str] = (
    "test(trybuild_fixtures_compile_without_diagnostics)"
)
TRYBUILD_CONTRACT_FILTER: typ.Final[str] = (
    f"{COMPILE_CONTRACT_FILTER} | {NESTED_TRYBUILD_TEST_FILTER}"
)
NESTED_TRYBUILD_FILTER: typ.Final[str] = (
    "package(rstest_helper_should_be_fixture) & "
    f"{NESTED_TRYBUILD_TEST_FILTER}"
)
NESTED_EXAMPLE_FILTER: typ.Final[str] = (
    "package(rstest_helper_should_be_fixture) & "
    "test(example_compiles_without_diagnostics)"
)
NESTED_UI_FIXTURE_FILTER: typ.Final[str] = (
    "package(no_unwrap_or_else_panic) & "
    "test(ui::aliased_test_crate_non_companion_does_not_exempt_parent_function)"
)
NESTED_UI_HARNESS_FILTER: typ.Final[str] = (
    "package(no_unwrap_or_else_panic) & test(ui::ui)"
)


class Allowance(typ.NamedTuple):
    """One override's whole contribution to a profile.

    Attributes
    ----------
    slow_timeout : dict[str, object]
        The whole ``slow-timeout`` table the override must declare.
    test_group : str or None
        The group it must name, or None when it names none. The budget
        and group are one claim: nested Cargo builds with long allowances
        share one serial group so cold fixtures do not compete for admission.
    retries : integer, dict[str, object], or None
        The whole ``retries`` setting, or None when the override declares
        none. The Dylint UI harnesses retry transient Windows rename failures;
        the large nested UI harnesses and compile contracts disable retries
        because each retry repeats a costly Cargo build.
    """

    slow_timeout: dict[str, object]
    test_group: str | None
    retries: int | dict[str, object] | None = None


#: The overrides both profiles carry, keyed by the binary or filter they
#: exist for, with the whole ``slow-timeout`` each must declare. Both
#: are named so one cannot be dropped while the other keeps the profile
#: looking deliberate, and the table is compared whole rather than field
#: by field, so an added or removed key fails too. The grace period
#: matters as much as the period: it is what nextest waits before
#: killing a test it has signalled, and the watchdog above is sized to
#: cover it.
#:
#: The filters are stated whole and matched exactly, per profile. A
#: substring would accept a narrowed one: dropping a clause from a `ui`
#: disjunction leaves each selected needle intact while the affected tests
#: lose the 20-minute allowance and fall back to the base 300 seconds. The
#: default profile includes platform-specific example tests that `ci` omits,
#: but both profiles give expensive API contracts and nested Cargo fixtures
#: their own explicit allowances.
REQUIRED_OVERRIDES: typ.Final[dict[str, dict[str, Allowance]]] = {
    "default": {
        BUILD_SCRIPT_INTEGRATION_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-nested-cargo-builds",
        ),
        NO_DEFAULT_FEATURES_FILTER: Allowance(
            slow_timeout={
                "period": "20m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-nested-cargo-builds",
        ),
        COMPILE_CONTRACT_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-trybuild-contracts",
            retries=0,
        ),
        NESTED_TRYBUILD_FILTER: Allowance(
            slow_timeout={
                "period": "120m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-trybuild-contracts",
            retries=0,
        ),
        NESTED_EXAMPLE_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries={"backoff": "exponential", "count": 2, "delay": "5s"},
        ),
        NESTED_UI_FIXTURE_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries=0,
        ),
        NESTED_UI_HARNESS_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries=0,
        ),
        ("binary(behaviour_toolchain)"): Allowance(
            slow_timeout={
                "period": "30m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            # The default profile serializes these in a second override
            # matching three named scenarios, so this one names none.
            test_group=None,
        ),
        (
            "test(driver::ui::) | "
            "test(tests::ui::) | "
            "(test(ui::ui) & not package(no_unwrap_or_else_panic)) | "
            "test(acquire_uses_the_effective_target_directory) | "
            "test(cross_process_lock_contention_and_release) | "
            "test(example_harness_collects_call_site_evidence) | "
            "test(ui::example_compiles_under_test_harness) | "
            "test(ui::hand_written_test_companion_does_not_exempt_parent_"
            "function) | "
            "test(ui::rstest_unwrap_outside_tests_still_fails_in_non_harness_"
            "code) | "
            "test(ui::rstest_empty_companion_does_not_exempt_parent_function)"
            " | "
            "(binary(ui) & test(=ui))"
        ): Allowance(
            slow_timeout={
                "period": "20m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries={"backoff": "exponential", "count": 2, "delay": "5s"},
        ),
    },
    "ci": {
        BUILD_SCRIPT_INTEGRATION_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-nested-cargo-builds",
        ),
        NO_DEFAULT_FEATURES_FILTER: Allowance(
            slow_timeout={
                "period": "20m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-nested-cargo-builds",
        ),
        COMPILE_CONTRACT_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-trybuild-contracts",
            retries=0,
        ),
        NESTED_TRYBUILD_FILTER: Allowance(
            slow_timeout={
                "period": "120m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-trybuild-contracts",
            retries=0,
        ),
        NESTED_EXAMPLE_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries={"backoff": "exponential", "count": 2, "delay": "5s"},
        ),
        NESTED_UI_FIXTURE_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries=0,
        ),
        NESTED_UI_HARNESS_FILTER: Allowance(
            slow_timeout={
                "period": "75m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries=0,
        ),
        ("binary(behaviour_toolchain)"): Allowance(
            slow_timeout={
                "period": "30m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-toolchain-installs",
        ),
        (
            "test(driver::ui::) | "
            "test(tests::ui::) | "
            "(test(ui::ui) & not package(no_unwrap_or_else_panic)) | "
            "test(acquire_uses_the_effective_target_directory) | "
            "test(cross_process_lock_contention_and_release) | "
            "(binary(ui) & test(=ui))"
        ): Allowance(
            slow_timeout={
                "period": "20m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
        ),
    },
}

#: Every group an override may name, with the whole table declaring it.
#: A group a profile names and ``[test-groups]`` does not declare is a
#: configuration nextest refuses, and one declared without
#: ``max-threads = 1`` serializes nothing: the UI harnesses would then
#: build lint libraries concurrently against the shared target directory,
#: while nested Cargo fixtures would compete for compiler admission.
REQUIRED_TEST_GROUPS: typ.Final[dict[str, dict[str, object]]] = {
    "serial-dylint-ui": {"max-threads": 1},
    "serial-trybuild-contracts": {"max-threads": 1},
    "serial-toolchain-installs": {"max-threads": 1},
    "serial-nested-cargo-builds": {"max-threads": 1},
}
