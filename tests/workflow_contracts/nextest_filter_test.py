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
from nextest_filter import FilterReader, FilterSubject, filter_may_select_test


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
        ("test(/absent/)", True),
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
    test = FilterSubject("trybuild_fixtures_compile_without_diagnostics")
    assert filter_may_select_test(selector, test) is matches


@pytest.mark.parametrize(
    "selector",
    [
        r"test(=call(site\))",
        r"test(call(site)",
        r"test(~site\))",
        r"test(#call(site\))",
    ],
)
def test_matcher_text_resolves_nextest_escapes(selector: str) -> None:
    """End a matcher at the first unescaped parenthesis."""
    assert FilterReader(selector, FilterSubject("call(site)")).value() is True


@pytest.mark.parametrize(
    ("selector", "matches"),
    [
        ("package(whitaker-installer)", True),
        ("package(installer)", False),
        ("package(~installer)", True),
        ("package(=whitaker-*)", False),
        ("binary(ui)", True),
        ("binary(other)", False),
        ("kind(test)", True),
        ("kind(te*)", False),
        ("kind(#test)", True),
    ],
)
def test_metadata_predicates_use_their_documented_default_matchers(
    selector: str, matches: bool
) -> None:
    """Glob ``package`` and ``binary`` by default, and compare ``kind`` exactly."""
    test = FilterSubject("a", package="whitaker-installer", binary="ui", kind="test")
    assert FilterReader(selector, test).value() is matches


@pytest.mark.parametrize(
    "selector",
    [
        "package(whitaker-installer)",
        "binary(ui)",
        "kind(test)",
        "binary_id(whitaker-installer::ui)",
        "platform(host)",
        "default()",
        "deps(whitaker-installer)",
        "rdeps(whitaker-installer)",
    ],
)
def test_predicates_without_data_are_unknown_rather_than_false(selector: str) -> None:
    """Never read a predicate the reader has no data for as a definite miss."""
    assert FilterReader(selector, FilterSubject("a")).value() is None


@pytest.mark.parametrize(
    "selector",
    [
        r"package({whitaker-installer\,other})",
        r"package(#{whitaker-installer\,other})",
        r"not package({other\,nope})",
        "binary([^x]i)",
        r"test(#{a\,b})",
        "binary([!x]i)",
        "package(whitaker-*)",
        "package(#whitaker-*)",
        "package(**/whitaker-installer)",
        "package(#**/whitaker-installer)",
        "not package(**/whitaker-installer)",
        "binary(u?)",
        "kind(#te*)",
        "test(#*absent*)",
        "binary([u]i)",
        "package(/^whitaker/)",
        "test(/absent/)",
        "test(/a(b)c/)",
        r"test(/a\)b/)",
        r"test(/\p{L}+/)",
        "not test(/(?i)publ\u0131c/)",
    ],
)
def test_matchers_python_cannot_evaluate_are_unknown_rather_than_false(
    selector: str,
) -> None:
    """Report matchers ``fnmatch`` or ``re`` cannot be relied on for as unknown."""
    test = FilterSubject("a", package="whitaker-installer", binary="ui", kind="test")
    assert FilterReader(selector, test).value() is None


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
        pytest.param("test(a) + test(b)", id="plus-is-outside-the-grammar"),
        pytest.param(r"test(a\()", id="opening-parenthesis-is-not-an-escape"),
        pytest.param("test(/a)", id="regex-must-close"),
    ],
)
def test_malformed_filters_raise_rather_than_read_as_no_match(expression: str) -> None:
    """Reject bad syntax, which a guard would otherwise read as no capture."""
    with pytest.raises(ValueError, match=r"\S"):
        FilterReader(expression, FilterSubject("a")).value()
