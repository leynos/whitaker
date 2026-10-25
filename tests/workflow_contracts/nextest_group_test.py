"""Pin nextest test-group declarations and override references.

Purpose
-------
Keep every override group declared at capacity one and keep the trybuild
contract tests out of the serialized Dylint UI group.

Usage
-----
Run this contract with ``make test-workflow-contracts``.
"""

from __future__ import annotations

import fnmatch
import re
import typing as typ

import pytest
from nextest_config import load_nextest_config
from nextest_profile_test import COMPILE_CONTRACT_FILTER, REQUIRED_TEST_GROUPS


def _test_name_matches(selector: str, test_name: str) -> bool:
    """Match a Nextest test-name selector against a test name."""
    if selector.startswith("="):
        return test_name == selector[1:]
    if selector.startswith("~"):
        return selector[1:] in test_name
    if selector.startswith("/") and selector.endswith("/"):
        return re.search(selector[1:-1], test_name) is not None
    if selector.startswith("#"):
        return fnmatch.fnmatchcase(test_name, selector[1:])
    return selector in test_name


def _advance_selector_nesting(
    character: str, depth: int, escaped: bool
) -> tuple[int, bool]:
    """Track balanced parentheses while respecting Nextest escapes."""
    if escaped:
        return depth, False
    if character == "\\":
        return depth, True
    if character == "(":
        return depth + 1, False
    if character == ")":
        return depth - 1, False
    return depth, False


def _test_selector_end(filter_expression: str, selector_start: int) -> int | None:
    """Find the closing parenthesis for a test selector."""
    depth = 1
    escaped = False
    for position, character in enumerate(filter_expression[selector_start:], selector_start):
        depth, escaped = _advance_selector_nesting(character, depth, escaped)
        if depth == 0:
            return position
    return None


def _test_name_selectors(filter_expression: str) -> list[str]:
    """Extract test-name selectors from a Nextest filter expression."""
    selectors = []
    offset = 0
    while (start := filter_expression.find("test(", offset)) != -1:
        selector_start = start + len("test(")
        selector_end = _test_selector_end(filter_expression, selector_start)
        if selector_end is None:
            break
        selectors.append(filter_expression[selector_start:selector_end])
        offset = selector_end + 1
    return selectors


def _filter_may_select_test(filter_expression: str, test_name: str) -> bool:
    """Check whether a test predicate can match a test name."""
    return any(
        _test_name_matches(selector, test_name)
        for selector in _test_name_selectors(filter_expression)
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


@pytest.mark.parametrize(
    ("selector", "matches"),
    [
        ("test(trybuild)", True),
        ("test(=trybuild_fixtures_compile_without_diagnostics)", True),
        ("test(/trybuild/)", True),
        ("test(#*trybuild*)", True),
        ("test(=ui)", False),
    ],
)
def test_name_selector_matching_covers_nextest_matcher_forms(
    selector: str, matches: bool
) -> None:
    """Recognize the test-name matchers Nextest uses in configuration."""
    assert _filter_may_select_test(
        selector, "trybuild_fixtures_compile_without_diagnostics"
    ) is matches


def test_trybuild_contracts_do_not_match_dylint_ui_overrides(
    parsed_nextest: dict[str, typ.Any],
) -> None:
    """Keep trybuild tests out of serial Dylint UI overrides."""
    trybuild_names = tuple(
        clause.removeprefix("test(").removesuffix(")")
        for clause in COMPILE_CONTRACT_FILTER.split(" | ")
    )
    matching_filters = [
        (profile_name, override.get("filter"))
        for profile_name in ("default", "ci")
        for override in parsed_nextest["profile"][profile_name].get("overrides") or []
        if override.get("test-group") == "serial-dylint-ui"
        and any(
            _filter_may_select_test(override.get("filter") or "", test_name)
            for test_name in trybuild_names
        )
    ]

    assert not matching_filters, (
        "trybuild contract names must not match serial-dylint-ui overrides "
        f"in either profile; found {matching_filters}"
    )
