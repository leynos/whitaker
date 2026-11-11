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

from nextest_filter import FilterSubject

# Parser-pin fixtures share one integration binary and Cargo package cache, so
# the allowance must cover that whole binary and serialize it.
BUILD_SCRIPT_INTEGRATION_FILTER: typ.Final[str] = "binary(build_script_integration)"
NO_DEFAULT_FEATURES_FILTER: typ.Final[str] = "binary(no_default_features_build)"
ISOLATED_CARGO_CHECKS_FILTER: typ.Final[str] = (
    "test(crate_builds_without_dylint_driver_feature) | "
    "test(fluent_bundles_are_included_in_the_package_tarball)"
)
COMPILE_CONTRACT_FILTER: typ.Final[str] = (
    "test(ortho_config_and_clap_derives_compile_together) | "
    "test(dependency_check_result_contract_is_public) | "
    "test(fixture_expansion_lint_attribute_contract) | "
    "test(public_const_api_works_in_const_context) | "
    "test(renamed_common_apis_are_public_and_legacy_names_stay_removed) | "
    "test(renamed_sarif_api_is_public_and_legacy_names_stay_removed) | "
    "test(sha2_0_11_pre_migration_patterns_fail_to_compile)"
)


class CompileContract(typ.NamedTuple):
    """One separately grouped compile contract and the crate that defines it.

    Attributes
    ----------
    test : FilterSubject
        What an override's filter is evaluated against for this contract.
    crate_directory : str
        The defining crate, relative to the repository root, so the identity
        can be checked against the source rather than trusted.
    """

    test: FilterSubject
    crate_directory: str


#: ``COMPILE_CONTRACT_FILTER`` with the identity each name has in the workspace.
#: Filters such as ``binary(integration_exclusion)`` can only be ruled out by
#: knowing which binary a contract is in, and a name alone cannot say that.
COMPILE_CONTRACTS: typ.Final[tuple[CompileContract, ...]] = (
    CompileContract(
        FilterSubject(
            "ortho_config_and_clap_derives_compile_together",
            package="whitaker",
            binary="ui",
            kind="test",
        ),
        ".",
    ),
    CompileContract(
        FilterSubject(
            "dependency_check_result_contract_is_public",
            package="whitaker-installer",
            binary="ui",
            kind="test",
        ),
        "installer",
    ),
    CompileContract(
        FilterSubject(
            "fixture_expansion_lint_attribute_contract",
            package="whitaker_test_macros",
            binary="ui",
            kind="test",
        ),
        "crates/whitaker_test_macros",
    ),
    CompileContract(
        FilterSubject(
            "public_const_api_works_in_const_context",
            package="whitaker-installer",
            binary="ui",
            kind="test",
        ),
        "installer",
    ),
    CompileContract(
        FilterSubject(
            "renamed_common_apis_are_public_and_legacy_names_stay_removed",
            package="whitaker-common",
            binary="public_api_contract",
            kind="test",
        ),
        "common",
    ),
    CompileContract(
        FilterSubject(
            "renamed_sarif_api_is_public_and_legacy_names_stay_removed",
            package="whitaker_sarif",
            binary="api_contract",
            kind="test",
        ),
        "crates/whitaker_sarif",
    ),
    CompileContract(
        FilterSubject(
            "sha2_0_11_pre_migration_patterns_fail_to_compile",
            package="whitaker-installer",
            binary="ui",
            kind="test",
        ),
        "installer",
    ),
)
NESTED_TRYBUILD_TEST_FILTER: typ.Final[str] = (
    "test(trybuild_fixtures_compile_without_diagnostics)"
)
TRYBUILD_CONTRACT_FILTER: typ.Final[str] = (
    f"{COMPILE_CONTRACT_FILTER} | {NESTED_TRYBUILD_TEST_FILTER}"
)
NESTED_TRYBUILD_FILTER: typ.Final[str] = (
    f"package(rstest_helper_should_be_fixture) & {NESTED_TRYBUILD_TEST_FILTER}"
)
#: Both of this package's harnesses rebuild the fixture project through
#: nested Cargo, so both must be named for the whole allowance to apply.
NESTED_EXAMPLE_FILTER: typ.Final[str] = (
    "package(rstest_helper_should_be_fixture) & "
    "(test(example_compiles_without_diagnostics) | "
    "test(example_harness_collects_call_site_evidence))"
)
NESTED_UI_FIXTURE_FILTER: typ.Final[str] = (
    "package(no_unwrap_or_else_panic) & "
    "test(ui::aliased_test_crate_non_companion_does_not_exempt_parent_function)"
)
#: Match the package's own binary rather than one test name. `test(ui::ui)`
#: selects only the test literally called `ui::ui`, so every other harness in
#: the package fell to the base 300 s bound and two were killed at 300.006 s.
NESTED_UI_HARNESS_FILTER: typ.Final[str] = (
    "package(no_unwrap_or_else_panic) & binary(no_unwrap_or_else_panic)"
)
#: The serialized lane for Dylint UI harnesses in packages with no override of
#: their own. The exclusion wraps the whole disjunction, not one clause: a bare
#: `test(<name>)` matches that name in every package, so guarding one clause
#: left the next free to capture `no_unwrap_or_else_panic`'s or
#: `rstest_helper_should_be_fixture`'s harness and hold it to this lane's
#: 30 minutes instead of its own 75. The lock tests join outside the guard
#: because they belong to the excluded package but still need the serial
#: group.
BROAD_UI_FILTER: typ.Final[str] = (
    "((test(driver::ui::) | test(tests::ui::) | test(ui::ui) | "
    "test(example_harness_collects_call_site_evidence) | "
    "test(ui::example_compiles_under_test_harness) | "
    "test(ui::rstest_unwrap_outside_tests_still_fails_in_non_harness_code) "
    "| test(ui::rstest_empty_companion_does_not_exempt_parent_function) "
    "| (binary(ui) & test(=ui))) & not package(no_unwrap_or_else_panic) "
    "& not package(rstest_helper_should_be_fixture)) | "
    "test(acquire_uses_the_effective_target_directory) | "
    "test(cross_process_lock_contention_and_release) | "
    "test(exported_entry_point_registers_suite_lints)"
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
#: lose the 30-minute allowance and fall back to the base 300 seconds. The
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
        ISOLATED_CARGO_CHECKS_FILTER: Allowance(
            slow_timeout={
                "period": "30m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-nested-cargo-builds",
        ),
        COMPILE_CONTRACT_FILTER: Allowance(
            slow_timeout={
                "period": "120m",
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
            test_group="serial-dylint-ui",
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
                "period": "120m",
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
        BROAD_UI_FILTER: Allowance(
            slow_timeout={
                "period": "30m",
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
        ISOLATED_CARGO_CHECKS_FILTER: Allowance(
            slow_timeout={
                "period": "30m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-nested-cargo-builds",
        ),
        COMPILE_CONTRACT_FILTER: Allowance(
            slow_timeout={
                "period": "120m",
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
            test_group="serial-dylint-ui",
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
                "period": "120m",
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
        BROAD_UI_FILTER: Allowance(
            slow_timeout={
                "period": "30m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries={"backoff": "exponential", "count": 2, "delay": "5s"},
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
