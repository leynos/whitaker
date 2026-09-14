"""Nextest durations, read the way the runner reads them.

Split from ``timeout_budgets`` so both stay inside the 400-line limit
``AGENTS.md`` sets, and because the grammar is a separable claim: every
ordering this contract asserts is an inequality between two numbers
this module produced.

nextest parses durations with ``humantime`` through ``humantime_serde``,
which reads a sequence of value-and-unit pairs and sums them, so ``2h
30m`` and ``1d`` are valid and a parser taking one pair would refuse
configuration the runner accepts. Refusing it here would fail a
configuration nextest is happy with, which is a worse failure than the
one this contract exists to catch. Accepting what nextest refuses is the
other half of the same fault: the contract would then assert an ordering
over a budget the runner never had.

The grammar is a port of humantime 2.3.0's own parser, the version the
lockfile of the pinned cargo-nextest release resolves, read rather than
assumed. A value may carry a fractional part with whitespace tolerated
around the point, whitespace inside the number is ignored, and a bare
``0`` is a zero duration needing no unit at all. Fractions are exact
integer arithmetic there, not floating point, and the arithmetic differs
by unit: see :class:`_Unit`.
"""

from __future__ import annotations

import re
import typing as typ

#: Nanoseconds in a second, which is the scale the port works in.
_SECOND: typ.Final[int] = 1_000_000_000


class _Unit(typ.NamedTuple):
    """One humantime unit, as its parser treats it.

    Attributes
    ----------
    nanoseconds : int
        One of this unit in nanoseconds. A value's whole part is
        multiplied by this.
    fraction_scale : int or None
        What a fraction's numerator is multiplied by before the exact
        division humantime requires, or None when the unit admits no
        fraction at all. ``ns`` is that case: humantime refuses a
        fractional nanosecond outright rather than rounding it.
    fraction_in_seconds : bool
        Whether that division yields seconds rather than nanoseconds.
        humantime divides whole seconds for hours and longer, so
        ``0.123h`` is refused where ``0.123s`` is exact. A reader
        working in nanoseconds throughout would accept durations nextest
        rejects, and one working in floats would accept every inexact
        fraction at every unit.
    """

    nanoseconds: int
    fraction_scale: int | None
    fraction_in_seconds: bool


#: Every spelling humantime accepts, grouped by the unit it names, with
#: humantime's own definitions of a month and a year. Spelt out in full
#: rather than trimmed to the plausible ones, because refusing a unit
#: nextest accepts would fail a configuration the runner is happy with.
_UNIT_SPELLINGS: typ.Final[tuple[tuple[tuple[str, ...], _Unit], ...]] = (
    (("nanos", "nsec", "ns"), _Unit(1, None, False)),
    (("usec", "us", "µs"), _Unit(1_000, 1_000, False)),
    (("millis", "msec", "ms"), _Unit(1_000_000, 1_000_000, False)),
    (("seconds", "second", "secs", "sec", "s"), _Unit(_SECOND, _SECOND, False)),
    (
        ("minutes", "minute", "mins", "min", "m"),
        _Unit(60 * _SECOND, 60 * _SECOND, False),
    ),
    (("hours", "hour", "hrs", "hr", "h"), _Unit(3_600 * _SECOND, 3_600, True)),
    (("days", "day", "d"), _Unit(86_400 * _SECOND, 86_400, True)),
    (("weeks", "week", "wks", "wk", "w"), _Unit(604_800 * _SECOND, 604_800, True)),
    (("months", "month", "M"), _Unit(2_630_016 * _SECOND, 2_630_016, True)),
    (
        ("years", "year", "yrs", "yr", "y"),
        _Unit(31_557_600 * _SECOND, 31_557_600, True),
    ),
)

_UNITS: typ.Final[dict[str, _Unit]] = {
    spelling: unit for spellings, unit in _UNIT_SPELLINGS for spelling in spellings
}

#: Each unit's length in seconds, for callers comparing budgets.
UNIT_SECONDS: typ.Final[dict[str, float]] = {
    spelling: unit.nanoseconds / _SECOND for spelling, unit in _UNITS.items()
}

#: Digits with whitespace tolerated between them. humantime's parser
#: ignores whitespace while it accumulates a number, so ``1 0s`` is ten
#: seconds rather than a malformed duration.
_SPACED_DIGITS: typ.Final[str] = r"\d(?:\s*\d)*"

#: One value-and-unit pair. The fractional part is optional and
#: humantime tolerates whitespace around the point; a leading point, a
#: trailing point, a second point, a sign and a digit separator are all
#: refused there and so are refused here.
_DURATION_TOKEN: typ.Final[re.Pattern[str]] = re.compile(
    rf"(?P<whole>{_SPACED_DIGITS})"
    rf"(?:\s*\.\s*(?P<fraction>{_SPACED_DIGITS}))?"
    r"\s*(?P<unit>[A-Za-zµ]+)\s*"
)

#: The one duration humantime accepts with no unit. Its parser
#: special-cases the exact text before reading a single character, so
#: the comparison here is against the raw value rather than a stripped
#: one: ``" 0 "`` is not this case and nextest refuses it.
_BARE_ZERO: typ.Final[str] = "0"


class NextestConfigurationError(ValueError):
    """Raised when the configuration cannot be read as a set of budgets.

    Separate from a budget in the wrong order. A file that is not TOML,
    a profile that declares no ``slow-timeout``, or one whose
    ``global-timeout`` has been commented out, is a configuration this
    contract cannot reason about rather than one whose tiers are
    inverted.
    """


def seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"45m"`` or
        ``"2h 30m"``.

    Returns
    -------
    float
        The duration in seconds.

    Raises
    ------
    NextestConfigurationError
        If the text is not a duration nextest would accept.

    Examples
    --------
    >>> seconds("45m")
    2700.0
    >>> seconds("2h 30m")
    9000.0
    """
    if duration == _BARE_ZERO:
        return 0.0
    text = duration.strip()
    if not text:
        message = f"unrecognized nextest duration {duration!r}: it is empty"
        raise NextestConfigurationError(message)
    total = 0
    position = 0
    while position < len(text):
        nanoseconds, position = _read_pair(duration, text, position)
        total += nanoseconds
    return total / _SECOND


def _read_pair(duration: str, text: str, position: int) -> tuple[int, int]:
    """Return one value-and-unit pair in nanoseconds, and where it ends."""
    match = _DURATION_TOKEN.match(text, position)
    if match is None:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime reads "
            f"a sequence of values, each optionally fractional and each "
            f"followed by a unit, or a bare {_BARE_ZERO!r}"
        )
        raise NextestConfigurationError(message)
    unit = _UNITS.get(match["unit"])
    if unit is None:
        message = (
            f"unrecognized nextest duration {duration!r}: "
            f"{match['unit']!r} is not a unit humantime accepts"
        )
        raise NextestConfigurationError(message)
    # humantime ignores whitespace while it accumulates a number and
    # around the fractional point, so the matched digits can read "1 0"
    # or "1 . 5"; int cannot.
    whole = int(_digits(match["whole"]))
    nanoseconds = whole * unit.nanoseconds
    if match["fraction"] is not None:
        nanoseconds += _fraction_nanoseconds(duration, match["fraction"], unit)
    return nanoseconds, match.end()


def _digits(matched: str) -> str:
    """Return a matched digit run with its internal whitespace removed."""
    return "".join(matched.split())


def _fraction_nanoseconds(duration: str, matched: str, unit: _Unit) -> int:
    """Return a fractional part in nanoseconds, or refuse it as humantime does.

    humantime carries the fraction as a numerator over a power of ten
    and divides with a remainder check, so a fraction that is not a
    whole number of the unit's smallest step is an error rather than a
    rounded value. The whole duration is named in the messages below
    rather than the fraction, which is not what anybody wrote.
    """
    digits = _digits(matched)
    numerator = int(digits)
    denominator = 10 ** len(digits)
    if unit.fraction_scale is None:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime has no "
            f"step below a nanosecond, so a fractional one is an error"
        )
        raise NextestConfigurationError(message)
    scaled = numerator * unit.fraction_scale
    if scaled % denominator:
        step = "second" if unit.fraction_in_seconds else "nanosecond"
        message = (
            f"unrecognized nextest duration {duration!r}: humantime divides "
            f"exactly, and this fraction is not a whole number of the unit's "
            f"{step}s"
        )
        raise NextestConfigurationError(message)
    quotient = scaled // denominator
    return quotient * _SECOND if unit.fraction_in_seconds else quotient
