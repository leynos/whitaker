"""Pin the Nextest filter reader against named examples and malformed input.

Purpose
-------
State the selector forms and the rejected syntax one case at a time, so a
failure names what stopped working. ``nextest_filter_property_test`` covers the
combinations these examples cannot.

Usage
-----
Run this contract with ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest
from nextest_filter import FilterReader, filter_may_select_test


@pytest.mark.parametrize(
    ("selector", "matches"),
    [
        ("all()", True),
        ("none()", False),
        ("test(other) | all()", True),
        ("not test(=ui)", True),
        ("!test(=ui)", True),
        ("not not test(trybuild)", True),
        ("not package(no_unwrap_or_else_panic)", True),
        ("test(trybuild)", True),
        ("test(trybuild) & not package(no_unwrap_or_else_panic)", True),
        ("test(=trybuild_fixtures_compile_without_diagnostics)", True),
        ("test(/trybuild/)", True),
        ("test(#*trybuild*)", True),
        ("test(=ui)", False),
        ("test(ui::ui) & not package(no_unwrap_or_else_panic)", False),
        (
            (
                "(test(ui::ui) & not package(no_unwrap_or_else_panic)) | "
                "(binary(ui) & test(=ui))"
            ),
            False,
        ),
    ],
)
def test_filter_matching_evaluates_nextest_boolean_forms(
    selector: str, matches: bool
) -> None:
    """Evaluate test-name matchers and the boolean operators around them."""
    assert (
        filter_may_select_test(
            selector, "trybuild_fixtures_compile_without_diagnostics"
        )
        is matches
    )


@pytest.mark.parametrize(
    "selector",
    [
        r"test(=call\(site\))",
        r"test(call\(site)",
        r"test(~site\))",
        r"test(#*\(site\))",
    ],
)
def test_matcher_text_resolves_nextest_escapes(selector: str) -> None:
    """Compare escaped parentheses as the characters they stand for."""
    assert FilterReader(selector, "call(site)").value() is True


@pytest.mark.parametrize(
    "expression",
    [
        pytest.param("", id="empty-filter-has-no-predicate"),
        pytest.param("test", id="predicate-needs-an-argument"),
        pytest.param("test(unterminated", id="argument-must-close"),
        pytest.param("(test(a)", id="group-must-close"),
        pytest.param("test(a))", id="stray-closing-parenthesis-is-not-ignored"),
        pytest.param("test(a) test(b)", id="two-predicates-need-an-operator"),
        pytest.param("test(a) &", id="conjunction-needs-its-right-side"),
        pytest.param("| test(a)", id="disjunction-needs-its-left-side"),
        pytest.param("test(a) or", id="word-operator-needs-its-right-side"),
        pytest.param("not", id="negation-needs-an-operand"),
        pytest.param("test(a) - test(b)", id="difference-is-outside-the-grammar"),
    ],
)
def test_malformed_filters_raise_rather_than_read_as_no_match(expression: str) -> None:
    """Reject bad syntax, which a guard would otherwise read as no capture."""
    with pytest.raises(ValueError, match=r"\S"):
        FilterReader(expression, "a").value()
