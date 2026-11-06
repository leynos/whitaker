"""Evaluate Nextest filter expressions against a known test, without Nextest.

The group contract has to ask whether an override's filter could capture a
named test, and running ``cargo nextest list`` for that would compile the
workspace. This module answers the question from the expression and whatever
the caller knows about the test. It is scoped to the workflow contracts and owns
nothing else: a contract that can afford a real listing should use one, as
``scripts/nextest_split_benchmark.py`` does, because Nextest stays the authority
on what a filter selects.

The reader is deliberately conservative. ``package()``, ``binary()`` and
``kind()`` are evaluated only when the caller supplies that field, and predicates
it has no data for, such as ``default()`` and ``deps()``, are unknown rather than
false, so a guard built on ``filter_may_select_test`` errs towards reporting a
capture. A test name alone therefore cannot rule out a bare ``package(...)``.
The same goes for matchers that need Nextest's own engines: every regular
expression and every glob with a metacharacter is unknown too. The reader's only
definite answers come from comparing exact strings, so it has no second regex or
glob engine to disagree with Nextest.
Syntax outside the supported grammar, including the ``+`` and ``-`` operators,
raises ``ValueError`` rather than guessing.

Matchers and escapes follow the Nextest filterset reference.[1]_

.. [1] https://nexte.st/docs/filtersets/reference/
"""

from __future__ import annotations

import re
import typing as typ


class FilterSubject(typ.NamedTuple):
    """What a filter can be evaluated against for one test.

    Attributes
    ----------
    name : str
        The test's name within its binary.
    package : str or None
        The package it belongs to, or None when the caller does not know.
    binary : str or None
        The name of its test binary, or None when the caller does not know.
    kind : str or None
        Its binary kind, such as ``test``, or None when the caller does not know.
    """

    name: str
    package: str | None = None
    binary: str | None = None
    kind: str | None = None


#: Nextest's default matcher for each predicate this reader can evaluate, named
#: by the prefix that would select it explicitly.
_TEST_DEFAULT_MATCHER: typ.Final[str] = "~"
_METADATA_DEFAULT_MATCHERS: typ.Final[dict[str, str]] = {
    "package": "#",
    "binary": "#",
    "kind": "=",
}

_GLOB_METACHARACTERS: typ.Final[re.Pattern[str]] = re.compile(r"[*?{}\[\]\\]")

_ESCAPES: typ.Final[dict[str, str]] = {
    "n": "\n",
    "r": "\r",
    "t": "\t",
    "\\": "\\",
    "/": "/",
    ")": ")",
    ",": ",",
}


def _resolve_escape(escape: re.Match[str]) -> str:
    """Return the character one escape stands for, or refuse an invalid one."""
    character = escape.group(1)
    if character not in _ESCAPES:
        raise ValueError(f"invalid escape \\{character} in Nextest matcher")
    return _ESCAPES[character]


def _unescape(text: str) -> str:
    """Return matcher text with its Nextest escapes resolved."""
    return re.sub(r"\\(.)", _resolve_escape, text)


def _literal_glob_matches(glob: str, value: str) -> bool | None:
    """Compare a metacharacter-free glob exactly, and report any other as unknown.

    Nextest uses ``globset``, and no Python matcher is known to agree with it:
    ``{a,b}``, ``[^a]`` and ``**/a`` each read differently under ``fnmatch``. A glob
    with no metacharacters can only match itself, which needs no glob engine.
    """
    return None if _GLOB_METACHARACTERS.search(glob) is not None else value == glob


def _selector_matches(selector: str, value: str, default_matcher: str) -> bool | None:
    """Match a Nextest name matcher, or report it unknown if it cannot be read.

    Regular expressions are always unknown. Nextest uses Rust's ``regex`` crate,
    and ``re`` differs from it on patterns both accept: ``(?i)publ\u0131c`` matches
    ``public`` only under ``re``. No repository filter uses one, so nothing is lost
    by not guessing.
    """
    if re.fullmatch(r"/.+/", selector, re.DOTALL) is not None:
        return None
    has_prefix = selector[:1] in {"=", "~", "#"}
    matcher = selector[0] if has_prefix else default_matcher
    text = _unescape(selector[1:] if has_prefix else selector)
    match matcher:
        case "=":
            return value == text
        case "~":
            return text in value
        case _:
            return _literal_glob_matches(text, value)


def _evaluate_predicate(name: str, argument: str, test: FilterSubject) -> bool | None:
    """Evaluate one predicate, or report it unknown when the data is missing."""
    if name == "test":
        return _selector_matches(argument, test.name, _TEST_DEFAULT_MATCHER)
    if name in {"all", "none"}:
        return name == "all"
    default_matcher = _METADATA_DEFAULT_MATCHERS.get(name)
    if default_matcher is None:
        return None
    value = getattr(test, name)
    return (
        None if value is None else _selector_matches(argument, value, default_matcher)
    )


_IDENTIFIER: typ.Final[re.Pattern[str]] = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*")

#: A token is ``(kind, name, argument)``; only ``atom`` tokens use the latter
#: two fields. Kinds are ``atom``, ``and``, ``or``, ``not``, ``lparen`` and
#: ``rparen``.
Token: typ.TypeAlias = tuple[str, str, str]


def _argument_end(filter_expression: str, argument_start: int) -> int | None:
    """Find the closing parenthesis for a predicate argument.

    A regular expression ends at an unescaped ``/)``, so it may hold parentheses.
    Any other matcher ends at the first unescaped ``)``: Nextest does not balance
    them, so ``test(a(b)`` is the complete matcher ``a(b``.
    """
    is_regex = filter_expression.startswith("/", argument_start)
    terminator = "/)" if is_regex else ")"
    position = argument_start + 1 if is_regex else argument_start
    while position < len(filter_expression):
        if filter_expression[position] == "\\":
            position += 2
        elif filter_expression.startswith(terminator, position):
            return position + len(terminator) - 1
        else:
            position += 1
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


_SINGLE_CHARACTER_TOKENS: typ.Final[dict[str, str]] = {
    "(": "lparen",
    ")": "rparen",
    "&": "and",
    "|": "or",
    "!": "not",
}


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
        match character:
            case _ if character.isspace():
                position += 1
            case "(" | ")" | "&" | "|" | "!":
                tokens.append((_SINGLE_CHARACTER_TOKENS[character], "", ""))
                position += 1
            case _:
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
    """Evaluate one Nextest filter expression against a test.

    Parameters
    ----------
    expression : str
        The filter expression, as an override's ``filter`` key states it.
    test : FilterSubject
        The test its predicates are compared with.

    Examples
    --------
    >>> FilterReader("test(ui) & not test(=other)", FilterSubject("ui::ui")).value()
    True
    >>> FilterReader("package(whitaker-*)", FilterSubject("ui::ui")).value() is None
    True
    >>> known = FilterSubject("ui::ui", package="whitaker-installer")
    >>> FilterReader("package(whitaker-installer)", known).value()
    True
    >>> FilterReader("package(whitaker-*)", known).value() is None
    True
    """

    def __init__(self, expression: str, test: FilterSubject) -> None:
        self._tokens = _tokenize(expression)
        self._test = test
        self._position = 0

    def value(self) -> bool | None:
        """Return the expression's truth value, or None when it is unknown.

        Returns
        -------
        bool or None
            Whether the expression selects the test, or None when that depends
            on a predicate the reader has no data for.

        Raises
        ------
        ValueError
            If the expression is malformed. Tokens left over after a complete
            expression count: reading only the first of them would let a stray
            parenthesis change a filter's meaning without any failure.

        Examples
        --------
        >>> FilterReader("test(a) | none()", FilterSubject("a")).value()
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
        return _evaluate_predicate(name, argument, self._test)

    def _accept(self, kind: str) -> bool:
        """Consume the next token when it has the given kind."""
        if (
            self._position < len(self._tokens)
            and self._tokens[self._position][0] == kind
        ):
            self._position += 1
            return True
        return False


def filter_may_select_test(filter_expression: str, test: FilterSubject) -> bool:
    """Check whether the filter may select a test.

    A filter is ruled out only when it is definitely false for what is known
    about the test. Supply the package, binary and kind where they are known: a
    name alone leaves a bare ``binary(...)`` override unknown, and so a possible
    capture of every test.

    Parameters
    ----------
    filter_expression : str
        The filter expression to evaluate.
    test : FilterSubject
        The test its predicates are compared with.

    Returns
    -------
    bool
        False only when the expression definitely does not select the test.

    Examples
    --------
    >>> filter_may_select_test("test(ui::ui)", FilterSubject("trybuild_contract"))
    False
    >>> filter_may_select_test("binary(ui)", FilterSubject("trybuild_contract"))
    True
    >>> known = FilterSubject("trybuild_contract", binary="api_contract")
    >>> filter_may_select_test("binary(ui)", known)
    False
    """
    return FilterReader(filter_expression, test).value() is not False
