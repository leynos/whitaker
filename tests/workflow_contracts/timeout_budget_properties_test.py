"""Invariants of the timeout arithmetic, over inputs nobody wrote down.

The bounded cases elsewhere pin the values this repository uses. These
state the properties that must hold for values it does not: a duration
is its number times the length of its unit whichever unit that is, a
composite is the sum of its parts, and a ceiling requirement never
falls when a budget it contains rises. A reading can agree with the
file in the tree and still break all three.
"""

from __future__ import annotations

import typing as typ

import pytest
from hypothesis import assume, given, settings
from hypothesis import strategies as st
from nextest_durations import UNIT_SECONDS, seconds
from timeout_budgets import Profile, profiles, required_ceiling

#: Units whose length is at least a second. Sub-second units make the
#: products too small to compare without the comparison becoming a
#: statement about floating point rather than about the reading.
_WHOLE_UNITS: typ.Final[tuple[str, ...]] = tuple(
    sorted(unit for unit, length in UNIT_SECONDS.items() if length >= 1.0)
)

_units = st.sampled_from(_WHOLE_UNITS)
_values = st.integers(min_value=0, max_value=10_000)

#: Generous because the host runs several agents at once; these are
#: pure arithmetic and the deadline is guarding against a hang, not
#: measuring speed.
_SETTINGS = settings(deadline=None, max_examples=200)


@_SETTINGS
@given(value=_values, unit=_units)
def test_a_duration_is_its_value_times_its_unit(value: int, unit: str) -> None:
    """Every unit scales, and the table is the only thing that says by how much.

    The exhaustive cases pin one of each unit; this states the relation
    the table encodes, so an entry that is right at one and wrong at
    scale is caught too.
    """
    assert seconds(f"{value}{unit}") == pytest.approx(value * UNIT_SECONDS[unit]), (
        f"{value}{unit} must scale by the length of its unit"
    )


@_SETTINGS
@given(
    first=_values,
    first_unit=_units,
    second=_values,
    second_unit=_units,
)
def test_a_composite_is_the_sum_of_its_parts(
    first: int, first_unit: str, second: int, second_unit: str
) -> None:
    """humantime sums a sequence of pairs, so the reader must sum them too.

    A reader taking one pair would refuse a composite outright, and one
    taking the last would silently discard everything before it.
    """
    composite = f"{first}{first_unit} {second}{second_unit}"
    parts = seconds(f"{first}{first_unit}") + seconds(f"{second}{second_unit}")
    assert seconds(composite) == pytest.approx(parts), (
        f"{composite!r} must read as the sum of its two pairs"
    )


def _profile(global_minutes: int, period_seconds: int) -> Profile:
    """Return a profile with that whole-run budget and per-test period."""
    return profiles(
        "[profile.example]\n"
        f'global-timeout = "{global_minutes}m"\n'
        "slow-timeout = { period = "
        f'"{period_seconds}s", terminate-after = 1, grace-period = "5s" '
        "}\n"
    )["example"]


@_SETTINGS
@given(
    lower=st.integers(min_value=1, max_value=600),
    raise_by=st.integers(min_value=0, max_value=600),
    period=st.integers(min_value=1, max_value=3600),
)
def test_raising_the_whole_run_budget_never_lowers_the_requirement(
    lower: int, raise_by: int, period: int
) -> None:
    """The required ceiling is monotonic in the budget it must cover.

    The ordering assertions compare a job's ceiling against this
    requirement. If the requirement could fall as a budget rose, a
    profile could be given a longer run and become easier to satisfy,
    which is the opposite of what the tier exists for.
    """
    smaller = required_ceiling(_profile(lower, period))
    larger = required_ceiling(_profile(lower + raise_by, period))
    assert larger >= smaller, (
        f"raising global-timeout from {lower}m to {lower + raise_by}m must not "
        f"lower the required ceiling ({larger}s against {smaller}s)"
    )


@_SETTINGS
@given(
    global_minutes=st.integers(min_value=1, max_value=600),
    period=st.integers(min_value=1, max_value=3600),
)
def test_the_requirement_always_exceeds_the_budget_it_covers(
    global_minutes: int, period: int
) -> None:
    """A ceiling equal to the run's own budget reports nothing.

    The requirement adds the termination allowance, the work outside
    the suite and a margin, so it must sit strictly above the whole-run
    budget for every profile, not merely for the one in this tree.
    """
    assume(global_minutes * 60 > period)
    required = required_ceiling(_profile(global_minutes, period))
    assert required > global_minutes * 60.0, (
        f"a {global_minutes}m run needs a ceiling above {global_minutes * 60}s; "
        f"the requirement is {required}s"
    )
