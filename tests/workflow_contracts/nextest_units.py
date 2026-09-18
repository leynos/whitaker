"""humantime's unit table, and how each unit's arithmetic lands.

Split from ``nextest_durations`` so both stay inside the 400-line limit
``AGENTS.md`` sets. The seam is data against behaviour: this module is
what humantime calls each unit and what each one is worth, and the
reader beside it is the grammar that consumes them.

Spelt out in full rather than trimmed to the plausible spellings,
because refusing a unit nextest accepts fails a configuration the runner
is happy with, which is a worse failure than the one the contract exists
to catch.
"""

from __future__ import annotations

import fractions
import typing as typ


#: Nanoseconds in a second, which is the scale the port works in.
_SECOND: typ.Final[int] = 1_000_000_000


class _Scaling(typ.NamedTuple):
    """One of humantime's products: a multiplier and where it lands.

    humantime multiplies in the unit the product lands in rather than in
    nanoseconds throughout, so a year's whole part scales by 31,557,600
    and the 64-bit check that follows is a check on seconds. Carrying
    the multiplier and its landing place together is what keeps that
    pairing from coming apart.

    Attributes
    ----------
    scale : int
        What the value is multiplied by.
    in_seconds : bool
        Whether the product is seconds rather than nanoseconds.
    """

    scale: int
    in_seconds: bool


class _Unit(typ.NamedTuple):
    """One humantime unit, as its parser treats it.

    The two scalings differ, and not only in magnitude: a second's whole
    part scales by one into seconds while its fraction scales by a
    thousand million into nanoseconds, and the landing place moves at a
    different unit for each. Whole parts land in seconds from a second
    upwards; fractions land in seconds only from an hour upwards, which
    is why ``0.123h`` is refused where ``0.123s`` is exact.

    Attributes
    ----------
    whole : _Scaling
        How the value's whole part is scaled.
    fraction : _Scaling or None
        How a fraction's numerator is scaled before the exact division
        humantime requires, or None when the unit admits no fraction at
        all. ``ns`` is that case: humantime refuses a fractional
        nanosecond outright rather than rounding it.
    """

    whole: _Scaling
    fraction: _Scaling | None


#: Every spelling humantime accepts, grouped by the unit it names, with
#: humantime's own definitions of a month and a year. Spelt out in full
#: rather than trimmed to the plausible ones, because refusing a unit
#: nextest accepts would fail a configuration the runner is happy with.
_UNIT_SPELLINGS: typ.Final[tuple[tuple[tuple[str, ...], _Unit], ...]] = (
    (("nanos", "nsec", "ns"), _Unit(_Scaling(1, False), None)),
    (("usec", "us", "µs"), _Unit(_Scaling(1_000, False), _Scaling(1_000, False))),
    (
        ("millis", "msec", "ms"),
        _Unit(_Scaling(1_000_000, False), _Scaling(1_000_000, False)),
    ),
    (
        ("seconds", "second", "secs", "sec", "s"),
        _Unit(_Scaling(1, True), _Scaling(_SECOND, False)),
    ),
    (
        ("minutes", "minute", "mins", "min", "m"),
        _Unit(_Scaling(60, True), _Scaling(60 * _SECOND, False)),
    ),
    (
        ("hours", "hour", "hrs", "hr", "h"),
        _Unit(_Scaling(3_600, True), _Scaling(3_600, True)),
    ),
    (("days", "day", "d"), _Unit(_Scaling(86_400, True), _Scaling(86_400, True))),
    (
        ("weeks", "week", "wks", "wk", "w"),
        _Unit(_Scaling(604_800, True), _Scaling(604_800, True)),
    ),
    (
        ("months", "month", "M"),
        _Unit(_Scaling(2_630_016, True), _Scaling(2_630_016, True)),
    ),
    (
        ("years", "year", "yrs", "yr", "y"),
        _Unit(_Scaling(31_557_600, True), _Scaling(31_557_600, True)),
    ),
)

_UNITS: typ.Final[dict[str, _Unit]] = {
    spelling: unit for spellings, unit in _UNIT_SPELLINGS for spelling in spellings
}

#: Each unit's length in seconds, exactly, for callers comparing
#: budgets. A ``Fraction`` for the reason ``seconds`` returns one: a
#: sub-second unit has no exact float, so a budget assembled from these
#: and one read from a duration would differ by a rounding error rather
#: than by anything anyone wrote.
UNIT_SECONDS: typ.Final[dict[str, fractions.Fraction]] = {
    spelling: fractions.Fraction(unit.whole.scale)
    if unit.whole.in_seconds
    else fractions.Fraction(unit.whole.scale, _SECOND)
    for spelling, unit in _UNITS.items()
}
