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

import typing as typ

import pytest
from nextest_config import load_nextest_config
from nextest_filter import filter_may_select_test
from nextest_overrides import (
    NESTED_TRYBUILD_FILTER,
    NESTED_TRYBUILD_TEST_FILTER,
    REQUIRED_TEST_GROUPS,
    TRYBUILD_CONTRACT_FILTER,
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


@pytest.mark.parametrize("profile_name", ("default", "ci"))
def test_separately_grouped_trybuild_contracts_do_not_match_dylint_ui_overrides(
    parsed_nextest: dict[str, typ.Any], profile_name: str
) -> None:
    """Keep the six separately grouped contracts out of Dylint overrides."""
    trybuild_filters = TRYBUILD_CONTRACT_FILTER.split(" | ")
    assert NESTED_TRYBUILD_TEST_FILTER in trybuild_filters
    trybuild_names = tuple(
        clause.removeprefix("test(").removesuffix(")")
        for clause in trybuild_filters
        if clause != NESTED_TRYBUILD_TEST_FILTER
    )
    serial_dylint_filters = [
        override.get("filter") or ""
        for override in parsed_nextest["profile"][profile_name].get("overrides") or []
        if override.get("test-group") == "serial-dylint-ui"
    ]
    matching_names = [
        test_name
        for test_name in trybuild_names
        if any(
            filter_may_select_test(filter_expression, test_name)
            for filter_expression in serial_dylint_filters
        )
    ]

    assert not matching_names, (
        "the six separately grouped trybuild contracts must not match "
        f"serial-dylint-ui overrides in {profile_name}; "
        f"found {matching_names}"
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
