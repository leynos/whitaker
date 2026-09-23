"""Which `#[test]` functions in one Rust source drive `trybuild`.

A call is attributed by the function whose body holds it, and a helper that
drives `trybuild` passes that on to every function that calls it, wherever the
helper is declared. Splitting the source at each `#[test]` attribute instead
credited a helper's call to whichever test happened to precede it and missed a
test whose helper was declared above it.

Bodies are found by balancing braces from each `fn` signature. String and
character literals are blanked first so a brace inside one does not unbalance
the count; comments are blanked for the same reason.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ

#: The construction that makes a test pay for a nested cargo build.
TRYBUILD_CALL: typ.Final[re.Pattern[str]] = re.compile(r"trybuild::TestCases::new\(\)")

#: A function signature's name, up to the parameter list.
_SIGNATURE: typ.Final[re.Pattern[str]] = re.compile(
    r"\bfn\s+(?P<name>[A-Za-z_]\w*)\s*(?:<[^{;]*?>)?\s*\("
)

#: Comments and literals, which may hold braces that are not code.
_NOT_CODE: typ.Final[re.Pattern[str]] = re.compile(
    r"//[^\n]*|/\*.*?\*/|\"(?:\\.|[^\"\\])*\"|'(?:\\.|[^'\\])'",
    re.DOTALL,
)

#: The attribute that marks a function as a test.
_TEST_ATTRIBUTE: typ.Final[str] = "#[test]"


class Function(typ.NamedTuple):
    """One function: its name, where its signature starts and its body ends."""

    name: str
    start: int
    end: int
    body: str


def _blanked(text: str) -> str:
    """Return the text with comments and literals replaced by spaces."""
    # Same length, so every index into the blanked text is an index into the
    # original; only the braces the count must ignore disappear.
    return _NOT_CODE.sub(lambda match: " " * len(match.group(0)), text)


def _body_end(code: str, opening: int) -> int:
    """Return the index just past the brace that closes the one at `opening`."""
    depth = 0
    for index in range(opening, len(code)):
        depth += {"{": 1, "}": -1}.get(code[index], 0)
        if depth == 0:
            return index + 1
    return len(code)


def functions(text: str) -> list[Function]:
    """Return every function with a body in one source, in declaration order.

    A signature followed by `;` before any `{` is a declaration with no body,
    such as a trait method, and is skipped.

    Examples
    --------
    >>> [f.name for f in functions("fn a() { b(); }\\nfn b() {}\\n")]
    ['a', 'b']
    """
    code = _blanked(text)
    found: list[Function] = []
    for match in _SIGNATURE.finditer(code):
        opening = code.find("{", match.end())
        semicolon = code.find(";", match.end())
        if opening < 0 or 0 <= semicolon < opening:
            continue
        end = _body_end(code, opening)
        found.append(Function(match["name"], match.start(), end, code[opening:end]))
    return found


def _calls(body: str, name: str) -> bool:
    """Return whether a body calls the named function."""
    return re.search(rf"\b{re.escape(name)}\s*\(", body) is not None


def _drivers(found: list[Function]) -> set[str]:
    """Return every function that drives `trybuild`, directly or through a call."""
    drivers = {function.name for function in found if TRYBUILD_CALL.search(function.body)}
    grown = True
    while grown:
        added = {
            function.name
            for function in found
            if function.name not in drivers
            and any(_calls(function.body, driver) for driver in drivers)
        }
        drivers |= added
        grown = bool(added)
    return drivers


def _tests(text: str, found: list[Function]) -> list[str]:
    """Return the functions marked `#[test]` in blanked code before their signature."""
    # The attributes of a function sit between the end of whatever precedes it
    # and its signature. A function nested in another's body starts before
    # that end, so it reads an empty span and is never a test.
    names: list[str] = []
    previous_end = 0
    for function in found:
        if _TEST_ATTRIBUTE in text[previous_end : function.start]:
            names.append(function.name)
        previous_end = max(previous_end, function.end)
    return names


def compile_contracts_in(text: str) -> list[str]:
    """Return the name of each `#[test]` function in a source that drives `trybuild`.

    Examples
    --------
    >>> compile_contracts_in(
    ...     "fn helper() { trybuild::TestCases::new(); }\\n"
    ...     "#[test]\\nfn uses_it() { helper(); }\\n"
    ...     "#[test]\\nfn plain() {}\\n"
    ... )
    ['uses_it']
    """
    found = functions(text)
    drivers = _drivers(found)
    return [name for name in _tests(_blanked(text), found) if name in drivers]
