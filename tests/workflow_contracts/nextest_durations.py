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

That arithmetic is bounded. humantime accumulates in 64-bit unsigned
integers and checks every multiplication and addition, so a duration can
be refused for its size as readily as for its spelling, and the bound is
not one number but several: the value itself, the value times its unit's
scale, and the running total in each of its two parts. Python's integers
have no such bound, so the checks are written out here; without them the
reader would report a budget the runner refuses at startup, which is the
failure this module exists to avoid. See :class:`_Total`.
"""

from __future__ import annotations

import fractions
import re
import typing as typ

from nextest_units import (
    _SECOND,
    UNIT_SECONDS,
    _Scaling,
    _Unit,
    _UNITS,
)

#: Re-exported so callers keep one import for the reader and its
#: unit table; the split below is about file length, not about the
#: two being separable claims.
__all__ = ["UNIT_SECONDS", "NextestConfigurationError", "seconds", "display_seconds"]

#: The characters humantime treats as whitespace, enumerated.
#:
#: humantime skips on Rust's ``char::is_whitespace``, which is the
#: Unicode White_Space property. Python's ``\s`` is that property plus
#: U+001C to U+001F, the file, group, record and unit separators, and
#: ``str.strip`` and ``str.split`` have the same four-character excess.
#: A reader spelling the class ``\s`` therefore reads ``1\x1cs`` as one
#: second and ``\x1c45m`` as forty-five minutes, both of which nextest
#: refuses at startup. That is the accept-what-the-runner-refuses
#: direction this module exists to avoid, and it is why the class is
#: written out rather than abbreviated.
#: ``test_the_whitespace_class_is_rusts_own`` pins the difference in
#: both directions, so a change in either language's notion of
#: whitespace fails there rather than in a runner.
_SPACE_CHARS: typ.Final[str] = (
    "\t\n\v\f\r \x85\xa0\u1680"
    "\u2000\u2001\u2002\u2003\u2004\u2005"
    "\u2006\u2007\u2008\u2009\u200a"
    "\u2028\u2029\u202f\u205f\u3000"
)

#: The same set as a regular-expression character class.
_SPACE: typ.Final[str] = f"[{re.escape(_SPACE_CHARS)}]"

#: Digits with whitespace tolerated between them. humantime's parser
#: ignores whitespace while it accumulates a number, so ``1 0s`` is ten
#: seconds rather than a malformed duration.
#:
#: Spelt ``[0-9]`` rather than ``\d``, which in Python matches every
#: Unicode decimal digit. humantime matches ``'0'..='9'`` and nothing
#: else, so an Arabic-Indic or Devanagari numeral is a duration this
#: reader would otherwise convert happily and nextest would refuse at
#: startup: the accept-what-the-runner-refuses direction this module
#: exists to avoid.
_SPACED_DIGITS: typ.Final[str] = rf"[0-9](?:{_SPACE}*[0-9])*"

#: One value-and-unit pair. The fractional part is optional and
#: humantime tolerates whitespace around the point; a leading point, a
#: trailing point, a second point, a sign and a digit separator are all
#: refused there and so are refused here.
_DURATION_TOKEN: typ.Final[re.Pattern[str]] = re.compile(
    rf"(?P<whole>{_SPACED_DIGITS})"
    rf"(?:{_SPACE}*\.{_SPACE}*(?P<fraction>{_SPACED_DIGITS}))?"
    rf"{_SPACE}*(?P<unit>[A-Za-zµ]+){_SPACE}*"
)

#: The one duration humantime accepts with no unit. Its parser
#: special-cases the exact text before reading a single character, so
#: the comparison here is against the raw value rather than a stripped
#: one: ``" 0 "`` is not this case and nextest refuses it.
_BARE_ZERO: typ.Final[str] = "0"


#: The largest value humantime's parser can hold. Its accumulators and
#: its intermediate products are all ``u64``, checked at every step.
_U64_MAX: typ.Final[int] = 2**64 - 1


class NextestConfigurationError(ValueError):
    """Raised when the configuration cannot be read as a set of budgets.

    Separate from a budget in the wrong order. A file that is not TOML,
    a profile that declares no ``slow-timeout``, or one whose
    ``global-timeout`` has been commented out, is a configuration this
    contract cannot reason about rather than one whose tiers are
    inverted.
    """


class _Total:
    """humantime's running total: whole seconds and a nanosecond part.

    Both are 64-bit unsigned there, and every step is checked, so this
    carries the pair rather than a single count of nanoseconds. The two
    differ: ``18446744073709551615ns 18446744073709551615ns`` names
    thirty-seven seconds, well inside a duration humantime can hold, and
    is refused all the same because the second value overflows the
    nanosecond accumulator before it is carried.
    """

    def __init__(self) -> None:
        self.seconds = 0
        self.nanoseconds = 0

    def add(self, duration: str, seconds: int, nanoseconds: int) -> None:
        """Add one part, refusing what humantime's checks would refuse."""
        nanos = _u64(duration, self.nanoseconds + nanoseconds)
        total = _u64(duration, self.seconds + seconds)
        # humantime carries twice: its parser normalizes each pair on a
        # strict `>`, and `Duration::new` carries the remainder on `>=`,
        # aborting the process rather than erroring when that carry
        # overflows. nextest cannot run either way, so both are one
        # refusal here. Written as one branch, because a reader with
        # both has an unfalsifiable one: over all seventy-one inputs of
        # the estate differential, removing the strict branch changes
        # nothing, while removing this one lets three overflowing
        # composites through.
        if nanos >= _SECOND:
            total = _u64(duration, total + nanos // _SECOND)
            nanos %= _SECOND
        self.seconds = total
        self.nanoseconds = nanos

    def as_seconds(self) -> fractions.Fraction:
        """Return the total in seconds, exactly.

        A ``Fraction`` rather than a ``float`` because the values here
        run to humantime's whole 64-bit range, and a float carries 53
        bits of significand. Above 2**53 seconds it cannot hold two
        budgets that differ by one second, so an ordering assertion
        between them compares equal and passes whichever way round they
        are. ``18446744073709551614s`` and ``18446744073709551615s`` are
        both in the differential this reader is measured against, and
        both convert to the same float.

        The nanosecond part makes the same point at the other end:
        ``0.1s`` has no exact float, so a budget assembled from tenths
        and one written as a decimal compare unequal by a rounding
        error rather than by anything a reader wrote.
        """
        return fractions.Fraction(self.seconds) + fractions.Fraction(
            self.nanoseconds, _SECOND
        )


def _u64(duration: str, value: int) -> int:
    """Return a value humantime could hold, or refuse it as humantime does.

    Every multiplication and addition in its parser is checked against
    this bound, and Python's integers are not, so each of those steps
    passes through here.
    """
    if value > _U64_MAX:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime "
            f"accumulates in 64-bit integers and this exceeds their range"
        )
        raise NextestConfigurationError(message)
    return value


def seconds(duration: str) -> fractions.Fraction:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"45m"`` or
        ``"2h 30m"``.

    Returns
    -------
    fractions.Fraction
        The duration in seconds, exactly. Exact because these values
        are compared with each other: humantime's range reaches
        2**64 seconds and a float holds 53 bits, so two budgets a
        second apart can convert to the same float and an ordering
        between them passes whichever way round it is written. Use
        ``display_seconds`` when a number is going into a message.

    Raises
    ------
    NextestConfigurationError
        If the text is not a duration nextest would accept.

    Examples
    --------
    >>> seconds("45m")
    Fraction(2700, 1)
    >>> seconds("2h 30m")
    Fraction(9000, 1)
    >>> seconds("0.5s")
    Fraction(1, 2)
    >>> float(seconds("45m"))
    2700.0
    """
    if duration == _BARE_ZERO:
        return fractions.Fraction(0)
    text = duration.strip(_SPACE_CHARS)
    if not text:
        message = f"unrecognized nextest duration {duration!r}: it is empty"
        raise NextestConfigurationError(message)
    total = _Total()
    position = 0
    while position < len(text):
        position = _read_pair(duration, text, position, total)
    return total.as_seconds()


def display_seconds(duration: str) -> float:
    """Convert a duration to seconds as a float, for putting in a message.

    Lossy on purpose, and separate from ``seconds`` on purpose. A float
    is what a reader wants to see in an assertion message; it is not
    what a comparison should be made on, because above 2**53 seconds it
    cannot tell two budgets a second apart apart. Keeping the two
    behind different names means a caller chooses which it wants rather
    than getting the lossy one by default.

    Examples
    --------
    >>> display_seconds("45m")
    2700.0
    >>> display_seconds("1.5h")
    5400.0
    """
    return float(seconds(duration))


def _read_pair(duration: str, text: str, position: int, total: _Total) -> int:
    """Add one value-and-unit pair to the total, and return where it ends."""
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
    whole = _u64(duration, int(_digits(match["whole"])))
    _add_scaled(duration, total, whole, unit.whole)
    if match["fraction"] is not None:
        _add_fraction(duration, total, match["fraction"], unit)
    return match.end()


def _add_scaled(duration: str, total: _Total, value: int, scaling: _Scaling) -> None:
    """Scale one part by its unit and add it, in the unit it lands in."""
    _add_landed(duration, total, _u64(duration, value * scaling.scale), scaling)


def _add_landed(duration: str, total: _Total, amount: int, scaling: _Scaling) -> None:
    """Add an already-scaled amount to whichever part it belongs in."""
    if scaling.in_seconds:
        total.add(duration, amount, 0)
    else:
        total.add(duration, 0, amount)


def _digits(matched: str) -> str:
    """Return a matched digit run with its internal whitespace removed.

    Removes exactly the characters the pattern tolerated. ``str.split``
    would additionally drop U+001C to U+001F, which the pattern never
    let through, so a separator inside a number would vanish rather
    than be refused.
    """
    return re.sub(_SPACE, "", matched)


def _add_fraction(duration: str, total: _Total, matched: str, unit: _Unit) -> None:
    """Add a fractional part, or refuse it as humantime does.

    humantime carries the fraction as a numerator over a power of ten
    and divides with a remainder check, so a fraction that is not a
    whole number of the unit's smallest step is an error rather than a
    rounded value. The whole duration is named in the messages below
    rather than the fraction, which is not what anybody wrote.

    Worked through, on ``0.000000001m``. The numerator is 1 and the
    denominator a thousand million, the digits being read as written. A
    minute's ``fraction_scale`` is sixty thousand million, one minute in
    nanoseconds, so the product is that and the exact division leaves
    sixty: ``0.000000001m`` is sixty nanoseconds, not one. That factor
    of sixty is the whole reason a minute needs an entry of its own
    rather than a second's; the same input under a second's scale reads
    as a single nanosecond, and no configuration in the tree would show
    the difference.

    The denominator is a power of ten built one digit at a time, and
    that too is checked: a fraction of twenty digits overflows it where
    one of nineteen does not, whatever the digits are.
    """
    digits = _digits(matched)
    numerator = _u64(duration, int(digits))
    denominator = _u64(duration, 10 ** len(digits))
    scaling = unit.fraction
    if scaling is None:
        message = (
            f"unrecognized nextest duration {duration!r}: humantime has no "
            f"step below a nanosecond, so a fractional one is an error"
        )
        raise NextestConfigurationError(message)
    scaled = _u64(duration, numerator * scaling.scale)
    if scaled % denominator:
        step = "second" if scaling.in_seconds else "nanosecond"
        message = (
            f"unrecognized nextest duration {duration!r}: humantime divides "
            f"exactly, and this fraction is not a whole number of the unit's "
            f"{step}s"
        )
        raise NextestConfigurationError(message)
    _add_landed(duration, total, scaled // denominator, scaling)
