"""Pin nextest test-group declarations and override references.

Purpose
-------
Keep every override group declared at capacity one, keep the six independent
trybuild contracts out of the serialized Dylint UI group, and keep the
lock-sharing trybuild fixture inside it.

Usage
-----
Run this contract with ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest
import tomllib
from nextest_config import load_nextest_config
from nextest_filter import filter_may_select_test
from nextest_overrides import (
    COMPILE_CONTRACT_FILTER,
    COMPILE_CONTRACTS,
    NESTED_TRYBUILD_FILTER,
    REQUIRED_TEST_GROUPS,
    CompileContract,
)


@pytest.fixture(scope="module")
def parsed_nextest() -> dict[str, typ.Any]:
    """Return the parsed nextest configuration."""
    return load_nextest_config()


@pytest.mark.parametrize(
    ("group", "declaration"),
    sorted(REQUIRED_TEST_GROUPS.items()),
    ids=sorted(REQUIRED_TEST_GROUPS),
)
def test_each_group_the_overrides_name_is_declared_serial(
    parsed_nextest: dict[str, typ.Any], group: str, declaration: dict[str, object]
) -> None:
    """Require each override group to be declared with capacity one.

    A group declared with any other thread count parses, reads as
    deliberate, and serializes nothing, so the UI harnesses still race on
    the shared target directory. Compare the whole table so added keys fail.
    """
    declared = parsed_nextest.get("test-groups", {})
    assert declared.get(group) == declaration, (
        f"[test-groups] must declare {group} as {declaration}; got "
        f"{declared.get(group)}"
    )


def test_no_override_names_a_group_the_configuration_lacks(
    parsed_nextest: dict[str, typ.Any],
) -> None:
    """Require every group named by an override to be declared."""
    declared = set(parsed_nextest.get("test-groups", {}))
    named = {
        group
        for section in parsed_nextest["profile"].values()
        for override in section.get("overrides") or []
        if (group := override.get("test-group")) is not None
    }
    assert named <= declared, (
        f"every test-group an override names must be declared in "
        f"[test-groups]; {sorted(named - declared)} are not"
    )


def _contracts_captured_by(filters: typ.Iterable[str]) -> list[str]:
    """Name the separately grouped contracts any of these filters may select."""
    expressions = tuple(filters)
    return [
        contract.test.name
        for contract in COMPILE_CONTRACTS
        if any(
            filter_may_select_test(expression, contract.test)
            for expression in expressions
        )
    ]


@pytest.mark.parametrize("profile_name", ("default", "ci"))
def test_separately_grouped_trybuild_contracts_do_not_match_dylint_ui_overrides(
    parsed_nextest: dict[str, typ.Any], profile_name: str
) -> None:
    """Keep the six separately grouped contracts out of Dylint overrides."""
    captured = _contracts_captured_by(
        override.get("filter") or ""
        for override in parsed_nextest["profile"][profile_name].get("overrides") or []
        if override.get("test-group") == "serial-dylint-ui"
    )

    assert not captured, (
        "the six separately grouped trybuild contracts must not match "
        f"serial-dylint-ui overrides in {profile_name}; found {captured}"
    )


@pytest.mark.parametrize(
    "conflicting_filter",
    [
        "package(whitaker-installer)",
        "package(whitaker-*)",
        "package(=whitaker-installer) & test(dependency_check_result_contract)",
        "binary(ui)",
        "binary(public_api_contract)",
        "binary(ui) & test(contract)",
        "kind(test)",
        "kind(test) & test(sha2_0_11)",
        "not package(no_unwrap_or_else_panic)",
        "default()",
        (
            r"package({whitaker-installer\,other}) & "
            "test(dependency_check_result_contract_is_public)"
        ),
        r"package(#{whitaker-installer\,other})",
        r"not package({other\,nope})",
        "not test(/(?i)publ\u0131c/) & test(dependency_check_result_contract_is_public)",
        "test(/dependency_check/)",
        "package([w]hitaker-installer)",
    ],
)
def test_guard_reports_a_filter_that_selects_by_test_metadata(
    conflicting_filter: str,
) -> None:
    """Report captures that no ``test()`` clause on its own would reveal."""
    assert _contracts_captured_by([conflicting_filter]), (
        f"{conflicting_filter!r} selects a separately grouped contract"
    )


@pytest.mark.parametrize(
    "unrelated_filter",
    [
        "binary(integration_exclusion)",
        "package(no_unwrap_or_else_panic)",
        "kind(example)",
        "package(whitaker-installer) & test(=ui)",
    ],
)
def test_guard_lets_a_filter_for_other_tests_through(unrelated_filter: str) -> None:
    """Stay quiet about filters whose metadata rules every contract out."""
    assert not _contracts_captured_by([unrelated_filter]), (
        f"{unrelated_filter!r} selects no separately grouped contract"
    )


def test_contract_identities_name_the_filter_the_group_uses() -> None:
    """Keep the identity table and the group's filter on the same six tests."""
    filter_names = {
        clause.removeprefix("test(").removesuffix(")")
        for clause in COMPILE_CONTRACT_FILTER.split(" | ")
    }
    assert {contract.test.name for contract in COMPILE_CONTRACTS} == filter_names, (
        "COMPILE_CONTRACTS must name exactly the tests COMPILE_CONTRACT_FILTER does"
    )


@pytest.mark.parametrize(
    "contract", COMPILE_CONTRACTS, ids=lambda contract: contract.test.name
)
def test_contract_identities_match_the_source_tree(contract: CompileContract) -> None:
    """Check each stated package and binary, because the guard trusts them."""
    crate = pathlib.Path(__file__).resolve().parents[2] / contract.crate_directory
    manifest = tomllib.loads((crate / "Cargo.toml").read_text(encoding="utf-8"))
    assert manifest["package"]["name"] == contract.test.package
    source = crate / "tests" / f"{contract.test.binary}.rs"
    assert f"fn {contract.test.name}(" in source.read_text(encoding="utf-8"), (
        f"{source} must define {contract.test.name}"
    )


@pytest.mark.parametrize("profile_name", ("default", "ci"))
def test_lock_sharing_trybuild_fixture_stays_in_dylint_ui_group(
    parsed_nextest: dict[str, typ.Any], profile_name: str
) -> None:
    """Keep the nested fixture with Dylint UI to share ``ExampleHarnessLock``."""
    nested_override_exists = any(
        override.get("filter") == NESTED_TRYBUILD_FILTER
        and override.get("test-group") == "serial-dylint-ui"
        for override in parsed_nextest["profile"][profile_name].get("overrides") or []
    )

    assert nested_override_exists, (
        "the lock-sharing trybuild fixture must stay in serial-dylint-ui "
        f"for {profile_name}"
    )
