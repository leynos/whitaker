"""Evaluate Nextest filter expressions against a test name, without Nextest.

The group contract has to ask whether an override's filter could capture a
named test, and running ``cargo nextest list`` for that would compile the
workspace. This module answers the question from the expression alone. It is
scoped to the workflow contracts and owns nothing else: a contract that can
afford a real listing should use one, as ``scripts/nextest_split_benchmark.py``
does, because Nextest stays the authority on what a filter selects.

The reader is deliberately conservative. A predicate that cannot be evaluated
from a name is unknown rather than false, so a guard built on
``filter_may_select_test`` errs towards reporting a capture. Syntax outside the
supported grammar raises ``ValueError`` rather than guessing.
"""

from __future__ import annotations

import fnmatch
import re
import typing as typ

#: Escapes Nextest requires inside a matcher so its text cannot end the
#: predicate early. Regular expressions keep theirs, because there the
#: backslash already means the same thing to ``re``.
_MATCHER_ESCAPE: typ.Final[re.Pattern[str]] = re.compile(r"\\([()\\,])")


def _unescape(text: str) -> str:
    """Return matcher text with its Nextest escapes resolved."""
    return _MATCHER_ESCAPE.sub(r"\1", text)


def _test_name_matches(selector: str, test_name: str) -> bool:
    """Match a Nextest test-name selector against a test name."""
    if selector.startswith("/") and selector.endswith("/"):
        return re.search(selector[1:-1], test_name) is not None
    if selector.startswith("="):
        return test_name == _unescape(selector[1:])
    if selector.startswith("~"):
        return _unescape(selector[1:]) in test_name
    if selector.startswith("#"):
        return fnmatch.fnmatchcase(test_name, _unescape(selector[1:]))
    return _unescape(selector) in test_name


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


def _skip_whitespace(text: str, position: int) -> int:
    """Return the first position at or after ``position`` that is not space."""
    while position < len(text) and text[position].isspace():
        position += 1
    return position


def _read_argument(
    filter_expression: str, name: str, after_name: int
) -> tuple[str, int]:
    """Read a predicate's parenthesized argument and the position after it."""
    opening = _skip_whitespace(filter_expression, after_name)
    if opening >= len(filter_expression) or filter_expression[opening] != "(":
        raise ValueError(f"predicate {name!r} has no argument in {filter_expression!r}")
    end = _argument_end(filter_expression, opening + 1)
    if end is None:
        raise ValueError(f"predicate {name!r} is unterminated in {filter_expression!r}")
    return filter_expression[opening + 1 : end], end + 1


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
    argument, position = _read_argument(filter_expression, name, keyword.end())
    tokens.append(("atom", name, argument))
    return position


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


class FilterReader:
    """Evaluate one Nextest filter expression against a test name.

    Parameters
    ----------
    expression : str
        The filter expression, as an override's ``filter`` key states it.
    test_name : str
        The name its ``test()`` predicates are compared with.

    Examples
    --------
    >>> FilterReader("test(ui) & not test(=other)", "ui::ui").value()
    True
    >>> FilterReader("default()", "ui::ui").value() is None
    True
    """

    def __init__(self, expression: str, test_name: str) -> None:
        self._tokens = _tokenize(expression)
        self._test_name = test_name
        self._position = 0

    def value(self) -> bool | None:
        """Return the expression's truth value, or None when it is unknown.

        Returns
        -------
        bool or None
            Whether the expression selects the test name, or None when that
            depends on a predicate the reader cannot evaluate from a name.

        Raises
        ------
        ValueError
            If the expression is malformed. Tokens left over after a complete
            expression count: reading only the first of them would let a stray
            parenthesis change a filter's meaning without any failure.

        Examples
        --------
        >>> FilterReader("test(a) | none()", "a").value()
        True
        """
        self._position = 0
        value = self._parse_disjunction()
        if self._position != len(self._tokens):
            raise ValueError("Nextest filter has tokens after a complete expression")
        return value

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
        if name in {"all", "none"}:
            return name == "all"
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


def filter_may_select_test(filter_expression: str, test_name: str) -> bool:
    """Check whether the filter may select a test name.

    Package, binary and kind predicates cannot select by test name, so a
    filter whose test-name clauses all miss is definitely false even when it
    also narrows itself with ``not package(...)``. Predicates the reader
    cannot place, such as ``default()``, stay unknown and count as possible
    matches.

    Parameters
    ----------
    filter_expression : str
        The filter expression to evaluate.
    test_name : str
        The name its ``test()`` predicates are compared with.

    Returns
    -------
    bool
        False only when the expression definitely does not select the name.

    Examples
    --------
    >>> filter_may_select_test("test(ui::ui)", "trybuild_contract")
    False
    >>> filter_may_select_test("default()", "trybuild_contract")
    True
    """
    return FilterReader(filter_expression, test_name).value() is not False
