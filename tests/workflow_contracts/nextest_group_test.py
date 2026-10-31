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
from nextest_overrides import COMPILE_CONTRACT_FILTER, REQUIRED_TEST_GROUPS


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


_IDENTIFIER: typ.Final[re.Pattern[str]] = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*")

#: A token is ``(kind, name, argument)``; only ``atom`` tokens use the latter
#: two fields. Kinds are ``atom``, ``and``, ``or``, ``not``, ``lparen`` and
#: ``rparen``.
Token: typ.TypeAlias = tuple[str, str, str]


def _advance_argument_nesting(
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


def _argument_end(filter_expression: str, argument_start: int) -> int | None:
    """Find the closing parenthesis for a predicate argument."""
    depth = 1
    escaped = False
    for position, character in enumerate(
        filter_expression[argument_start:], argument_start
    ):
        depth, escaped = _advance_argument_nesting(character, depth, escaped)
        if depth == 0:
            return position
    return None


def _read_atom(filter_expression: str, start: int, tokens: list[Token]) -> int:
    """Read one identifier atom and append its token."""
    keyword = _IDENTIFIER.match(filter_expression, start)
    if keyword is None:
        raise ValueError(
            f"unexpected character {filter_expression[start]!r} in "
            f"{filter_expression!r}"
        )
    name = keyword.group()
    if name in {"and", "or", "not"}:
        tokens.append((name, "", ""))
        return keyword.end()
    opening = keyword.end()
    while opening < len(filter_expression) and filter_expression[opening].isspace():
        opening += 1
    if opening >= len(filter_expression) or filter_expression[opening] != "(":
        raise ValueError(f"predicate {name!r} has no argument in {filter_expression!r}")
    end = _argument_end(filter_expression, opening + 1)
    if end is None:
        raise ValueError(f"predicate {name!r} is unterminated in {filter_expression!r}")
    tokens.append(("atom", name, filter_expression[opening + 1 : end]))
    return end + 1


def _tokenize(filter_expression: str) -> list[Token]:
    """Split a Nextest filter expression into operators and atoms."""
    tokens: list[Token] = []
    position = 0
    while position < len(filter_expression):
        character = filter_expression[position]
        if character.isspace():
            position += 1
        elif character in "()":
            tokens.append(("lparen" if character == "(" else "rparen", "", ""))
            position += 1
        elif character == "&":
            tokens.append(("and", "", ""))
            position += 1
        elif character == "|":
            tokens.append(("or", "", ""))
            position += 1
        elif character == "!":
            tokens.append(("not", "", ""))
            position += 1
        else:
            position = _read_atom(filter_expression, position, tokens)
    return tokens


def _conjoin(left: bool | None, right: bool | None) -> bool | None:
    """Combine two predicates with ``&`` under unknown atoms."""
    if left is False or right is False:
        return False
    if left is True and right is True:
        return True
    return None


def _disjoin(left: bool | None, right: bool | None) -> bool | None:
    """Combine two predicates with ``|`` under unknown atoms."""
    if left is True or right is True:
        return True
    if left is False and right is False:
        return False
    return None


def _negate(value: bool | None) -> bool | None:
    """Negate a predicate, leaving an unknown as unknown."""
    return None if value is None else not value


class _FilterReader:
    """Evaluate one Nextest filter expression against a test name."""

    def __init__(self, expression: str, test_name: str) -> None:
        self._tokens = _tokenize(expression)
        self._test_name = test_name
        self._position = 0

    def may_select(self) -> bool:
        """Return whether the expression may select the test name."""
        return self._parse_disjunction() is not False

    def _parse_disjunction(self) -> bool | None:
        """Parse the loosest-binding ``a | b`` form."""
        value = self._parse_conjunction()
        while self._accept("or"):
            value = _disjoin(value, self._parse_conjunction())
        return value

    def _parse_conjunction(self) -> bool | None:
        """Parse the ``a & b`` form."""
        value = self._parse_negation()
        while self._accept("and"):
            value = _conjoin(value, self._parse_negation())
        return value

    def _parse_negation(self) -> bool | None:
        """Parse the tightest-binding ``not a`` and ``!a`` forms."""
        if self._accept("not"):
            return _negate(self._parse_negation())
        return self._parse_primary()

    def _parse_primary(self) -> bool | None:
        """Parse a parenthesized expression or a bare atom."""
        if self._accept("lparen"):
            value = self._parse_disjunction()
            if not self._accept("rparen"):
                raise ValueError("unbalanced parentheses in Nextest filter")
            return value
        return self._parse_atom()

    def _parse_atom(self) -> bool | None:
        """Evaluate one predicate, or report it unknown."""
        if self._position >= len(self._tokens):
            raise ValueError("Nextest filter ended where a predicate was expected")
        kind, name, argument = self._tokens[self._position]
        if kind != "atom":
            raise ValueError(f"expected a predicate, found {kind!r}")
        self._position += 1
        if name == "test":
            return _test_name_matches(argument, self._test_name)
        if name == "all":
            return True
        if name == "none":
            return False
        if name in {"package", "binary", "kind"}:
            # These narrow a filter without selecting by test name, so they
            # cannot capture a contract on their own. Negated, the reader
            # flips this to True, which is right: `not package(...)` alone
            # selects nearly every test.
            return False
        # Anything else (``default()``, ``deps()``) may select by itself.
        return None

    def _accept(self, kind: str) -> bool:
        """Consume the next token when it has the given kind."""
        if (
            self._position < len(self._tokens)
            and self._tokens[self._position][0] == kind
        ):
            self._position += 1
            return True
        return False


def _filter_may_select_test(filter_expression: str, test_name: str) -> bool:
    """Check whether the filter may select a test name.

    Package, binary and kind predicates cannot select by test name, so a
    filter whose test-name clauses all miss is definitely false even when it
    also narrows itself with ``not package(...)``. Predicates the reader
    cannot place, such as ``default()``, stay unknown and count as possible
    matches.
    """
    return _FilterReader(filter_expression, test_name).may_select()


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
            "(test(ui::ui) & not package(no_unwrap_or_else_panic)) | "
            "(binary(ui) & test(=ui))",
            False,
        ),
    ],
)
def test_filter_matching_evaluates_nextest_boolean_forms(
    selector: str, matches: bool
) -> None:
    """Evaluate test-name matchers and the boolean operators around them."""
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
