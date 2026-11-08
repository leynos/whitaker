"""Every unit spelling and its worth, written out independently.

`nextest_units` is a table, and a test that derived its expectations from
that table would assert the table equals itself. A spelling dropped from
`_UNIT_SPELLINGS`, or a multiplier mistyped, would move both sides at once
and nothing would fail. The cost of that is asymmetric: refusing a unit
nextest accepts fails a configuration the runner is happy with, and accepting
one at the wrong scale sizes a budget wrongly without anything saying so.

So the expectations here are written from humantime's documented definitions
rather than read from the module under test. The two compound units are
written as their definitions rather than as their products, because the
product is exactly what a typo would preserve: humantime's month is 30.44
days and its year is 365.25 days, neither of which is the other divided or
multiplied by twelve.

Run via ``make test-workflow-contracts``.
"""

import fractions

import pytest
from nextest_durations import seconds
from nextest_units import UNIT_SECONDS

_MINUTE = 60
_HOUR = 60 * _MINUTE
_DAY = 24 * _HOUR
_WEEK = 7 * _DAY
#: humantime's month is 30.44 days, not a twelfth of its year: a twelfth of
#: 365.25 days is 30.4375 days, which is 216 seconds short.
_MONTH = fractions.Fraction(3044, 100) * _DAY
#: humantime's year is the Julian year, 365.25 days.
_YEAR = fractions.Fraction(36525, 100) * _DAY

#: Spelling to exact length in seconds. Every spelling humantime accepts,
#: listed here rather than generated, so a spelling that disappears from the
#: reader fails this file.
EXPECTED_SECONDS: dict[str, fractions.Fraction] = {
    "nanos": fractions.Fraction(1, 1_000_000_000),
    "nsec": fractions.Fraction(1, 1_000_000_000),
    "ns": fractions.Fraction(1, 1_000_000_000),
    "usec": fractions.Fraction(1, 1_000_000),
    "us": fractions.Fraction(1, 1_000_000),
    "µs": fractions.Fraction(1, 1_000_000),
    "millis": fractions.Fraction(1, 1_000),
    "msec": fractions.Fraction(1, 1_000),
    "ms": fractions.Fraction(1, 1_000),
    "seconds": fractions.Fraction(1),
    "second": fractions.Fraction(1),
    "secs": fractions.Fraction(1),
    "sec": fractions.Fraction(1),
    "s": fractions.Fraction(1),
    "minutes": fractions.Fraction(_MINUTE),
    "minute": fractions.Fraction(_MINUTE),
    "mins": fractions.Fraction(_MINUTE),
    "min": fractions.Fraction(_MINUTE),
    "m": fractions.Fraction(_MINUTE),
    "hours": fractions.Fraction(_HOUR),
    "hour": fractions.Fraction(_HOUR),
    "hrs": fractions.Fraction(_HOUR),
    "hr": fractions.Fraction(_HOUR),
    "h": fractions.Fraction(_HOUR),
    "days": fractions.Fraction(_DAY),
    "day": fractions.Fraction(_DAY),
    "d": fractions.Fraction(_DAY),
    "weeks": fractions.Fraction(_WEEK),
    "week": fractions.Fraction(_WEEK),
    "wks": fractions.Fraction(_WEEK),
    "wk": fractions.Fraction(_WEEK),
    "w": fractions.Fraction(_WEEK),
    "months": _MONTH,
    "month": _MONTH,
    "M": _MONTH,
    "years": _YEAR,
    "year": _YEAR,
    "yrs": _YEAR,
    "yr": _YEAR,
    "y": _YEAR,
}


def test_the_reader_accepts_exactly_these_spellings() -> None:
    """The set, both ways, because each direction fails differently.

    A spelling the reader lacks refuses a duration nextest accepts, which
    fails a configuration the runner is happy with. A spelling the reader has
    and this file does not is an accepted unit nobody has stated the worth of,
    and its arithmetic is then unasserted.
    """
    assert set(UNIT_SECONDS) == set(EXPECTED_SECONDS), (
        f"the reader accepts {sorted(set(UNIT_SECONDS) - set(EXPECTED_SECONDS))} "
        f"that this file does not state, and lacks "
        f"{sorted(set(EXPECTED_SECONDS) - set(UNIT_SECONDS))} that it does"
    )


@pytest.mark.parametrize("spelling", sorted(EXPECTED_SECONDS))
def test_each_spelling_is_worth_what_humantime_says(spelling: str) -> None:
    """One unit of each spelling, against a value written out here.

    Exact equality rather than a tolerance: `seconds` answers a `Fraction`
    precisely so a sub-second unit has an exact value, and a comparison that
    allowed a rounding error would accept the float arithmetic the reader
    exists to avoid.
    """
    expected = EXPECTED_SECONDS[spelling]
    assert UNIT_SECONDS[spelling] == expected, (
        f"one `{spelling}` is {expected} seconds, and the table says "
        f"{UNIT_SECONDS[spelling]}"
    )
    assert seconds(f"1{spelling}") == expected, (
        f"reading `1{spelling}` must give {expected} seconds; it gave "
        f"{seconds(f'1{spelling}')}"
    )


def test_a_month_is_not_a_twelfth_of_a_year() -> None:
    """The one relation a mistyped compound constant would still satisfy.

    Both compound units are round-looking whole numbers of seconds, so a
    transposed digit in either produces another plausible constant. They are
    related by neither twelve nor any other small factor, and stating that
    here is what makes the two definitions above load-bearing rather than
    decorative.
    """
    assert _YEAR / 12 != _MONTH, "humantime's month is 30.44 days, not a twelfth"
    assert _YEAR / 12 - _MONTH == fractions.Fraction(-216), (
        "a twelfth of humantime's year is 216 seconds short of its month"
    )
