"""Check the Nextest filter reader against an independent tri-state oracle.

The named examples pin individual forms. These generate whole expression trees
with a known truth value and require the reader to recover it, across operator
spellings, nesting, and parenthesization that relies on precedence. The oracle
orders the three truth values and takes minima and maxima, so it shares no
logic with the reader's case-by-case combinators.
"""

from __future__ import annotations

import typing as typ

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st
from nextest_filter import FilterReader, filter_may_select_test

#: Contains parentheses so that generated selectors must escape them.
_TEST_NAME: typ.Final[str] = "ui::call(site)_contract"

#: Generous because the host runs several agents at once; these are pure and
#: the deadline is guarding against a hang, not measuring speed.
_SETTINGS = settings(deadline=None, max_examples=300)

_RANK: typ.Final[dict[bool | None, int]] = {False: 0, None: 1, True: 2}
_VALUES: typ.Final[tuple[bool | None, bool | None, bool | None]] = (False, None, True)

_LOOSEST: typ.Final[int] = 0
_CONJUNCTION: typ.Final[int] = 1
_TIGHTEST: typ.Final[int] = 2


class _Node(typ.NamedTuple):
    """A generated expression: an atom, or an operator over its operands."""

    operator: str
    spelling: str
    operands: tuple[_Node, ...]
    expected: bool | None


def _atom(text: str, expected: bool | None) -> _Node:
    """Build a leaf whose truth value is known from how it was written."""
    return _Node("atom", text, (), expected)


def _escape(text: str) -> str:
    """Escape matcher text the way a filter author must."""
    return text.replace("(", r"\(").replace(")", r"\)")


_FIXED_ATOMS: typ.Final[tuple[_Node, ...]] = (
    _atom(f"test(={_escape(_TEST_NAME)})", expected=True),
    _atom("test(=call)", expected=False),
    _atom("test(~absent)", expected=False),
    _atom("test(absent)", expected=False),
    _atom(r"test(/^ui::call\(site\)/)", expected=True),
    _atom("test(/^absent/)", expected=False),
    _atom(r"test(#ui::*\(site\)*)", expected=True),
    _atom("test(#absent*)", expected=False),
    _atom("all()", expected=True),
    _atom("none()", expected=False),
    _atom("package(whitaker)", expected=False),
    _atom("binary(ui)", expected=False),
    _atom("kind(test)", expected=False),
    _atom("default()", expected=None),
    _atom("deps(whitaker)", expected=None),
)


def _substring_atom(bounds: tuple[int, int]) -> _Node:
    """Build a selector for a slice of the name, which must always match."""
    start, length = bounds
    text = _escape(_TEST_NAME[start : start + length])
    return _atom(f"test({text})", expected=True)


_substrings = (
    st.tuples(
        st.integers(min_value=0, max_value=len(_TEST_NAME) - 1),
        st.integers(min_value=1, max_value=4),
    )
    .filter(lambda bounds: bounds[0] + bounds[1] <= len(_TEST_NAME))
    .map(_substring_atom)
)

_atoms = st.one_of(st.sampled_from(_FIXED_ATOMS), _substrings)


def _combine(operator: str, spelling: str, operands: tuple[_Node, ...]) -> _Node:
    """Apply the oracle to an operator and its already-valued operands."""
    ranks = [_RANK[operand.expected] for operand in operands]
    if operator == "or":
        rank = max(ranks)
    elif operator == "and":
        rank = min(ranks)
    else:
        rank = 2 - ranks[0]
    return _Node(operator, spelling, operands, _VALUES[rank])


def _extend(children: st.SearchStrategy[_Node]) -> st.SearchStrategy[_Node]:
    """Generate one more level of operators over existing expressions."""
    pairs = st.tuples(children, children)
    return st.one_of(
        st.builds(_combine, st.just("or"), st.sampled_from(("|", "or")), pairs),
        st.builds(_combine, st.just("and"), st.sampled_from(("&", "and")), pairs),
        st.builds(
            _combine,
            st.just("not"),
            st.sampled_from(("not ", "!")),
            children.map(lambda child: (child,)),
        ),
    )


_expressions = st.recursive(_atoms, _extend, max_leaves=12)


def _binding(node: _Node) -> int:
    """Return how tightly an expression binds: higher needs fewer parentheses."""
    if node.operator == "or":
        return _LOOSEST
    if node.operator == "and":
        return _CONJUNCTION
    return _TIGHTEST


def _render(node: _Node, *, always_parenthesize: bool, floor: int = _LOOSEST) -> str:
    """Write an expression, with parentheses only where its meaning needs them."""
    if node.operator == "atom":
        return node.spelling
    if node.operator == "not":
        operand = _render(
            node.operands[0], always_parenthesize=always_parenthesize, floor=_TIGHTEST
        )
        return f"{node.spelling}{operand}"
    own = _binding(node)
    left, right = (
        # One step tighter on the right keeps the tree's own shape without
        # relying on associativity, so the oracle's grouping is what is parsed.
        _render(operand, always_parenthesize=always_parenthesize, floor=own + step)
        for step, operand in enumerate(node.operands)
    )
    text = f"{left} {node.spelling} {right}"
    return f"({text})" if always_parenthesize or own < floor else text


@_SETTINGS
@given(node=_expressions, always_parenthesize=st.booleans())
def test_the_reader_recovers_the_truth_value_of_any_expression(
    node: _Node, always_parenthesize: bool
) -> None:
    """Agree with the oracle, whichever parentheses the expression relies on."""
    expression = _render(node, always_parenthesize=always_parenthesize)
    assert FilterReader(expression, _TEST_NAME).value() is node.expected, (
        f"{expression!r} must evaluate to {node.expected}"
    )


@_SETTINGS
@given(node=_expressions)
def test_only_a_definite_miss_rules_a_capture_out(node: _Node) -> None:
    """Count an unknown as a possible capture, never as a miss."""
    expression = _render(node, always_parenthesize=False)
    assert filter_may_select_test(expression, _TEST_NAME) is (
        node.expected is not False
    ), f"{expression!r} is {node.expected}"


@_SETTINGS
@given(node=_expressions)
def test_a_stray_closing_parenthesis_is_never_ignored(node: _Node) -> None:
    """Refuse trailing tokens, whatever the expression before them says."""
    expression = f"{_render(node, always_parenthesize=False)})"
    with pytest.raises(ValueError, match="after a complete expression"):
        FilterReader(expression, _TEST_NAME).value()
