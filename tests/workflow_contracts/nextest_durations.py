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
one this contract exists to catch.

The grammar was measured against humantime 2.4.0, the version nextest
resolves, rather than assumed. A value may carry a fractional part with
whitespace tolerated around the point, so ``1.5m`` and ``1 . 5 m`` are
both ninety seconds, and ``wk``, ``wks``, ``yr`` and ``yrs`` are
accepted alongside the longer spellings.
"""

from __future__ import annotations

import re
import typing as typ

#: One value-and-unit pair. The fractional part is optional and
#: humantime tolerates whitespace around the point; a leading point, a
#: trailing point, a second point, a sign and a digit separator are all
#: refused there and so are refused here.
_DURATION_TOKEN: typ.Final[re.Pattern[str]] = re.compile(
    r"(?P<value>\d+(?:\s*\.\s*\d+)?)\s*(?P<unit>[A-Za-zµ]+)\s*"
)

#: Every unit ``humantime`` accepts, with its length in seconds, using
#: humantime's own definitions of a month and a year. Spelt out in full
#: rather than trimmed to the plausible ones, because refusing a unit
#: nextest accepts would fail a configuration the runner is happy with.
UNIT_SECONDS: typ.Final[dict[str, float]] = {
    "nsec": 1e-9,
    "ns": 1e-9,
    "usec": 1e-6,
    "us": 1e-6,
    "µs": 1e-6,
    "msec": 0.001,
    "ms": 0.001,
    "seconds": 1.0,
    "second": 1.0,
    "secs": 1.0,
    "sec": 1.0,
    "s": 1.0,
    "minutes": 60.0,
    "minute": 60.0,
    "mins": 60.0,
    "min": 60.0,
    "m": 60.0,
    "hours": 3600.0,
    "hour": 3600.0,
    "hrs": 3600.0,
    "hr": 3600.0,
    "h": 3600.0,
    "days": 86400.0,
    "day": 86400.0,
    "d": 86400.0,
    "weeks": 604800.0,
    "week": 604800.0,
    "wks": 604800.0,
    "wk": 604800.0,
    "w": 604800.0,
    "months": 2630016.0,
    "month": 2630016.0,
    "M": 2630016.0,
    "years": 31557600.0,
    "year": 31557600.0,
    "yrs": 31557600.0,
    "yr": 31557600.0,
    "y": 31557600.0,
}


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
    text = duration.strip()
    if not text:
        message = f"unrecognized nextest duration {duration!r}: it is empty"
        raise NextestConfigurationError(message)
    total = 0.0
    position = 0
    while position < len(text):
        match = _DURATION_TOKEN.match(text, position)
        if match is None:
            message = (
                f"unrecognized nextest duration {duration!r}: humantime reads "
                f"a sequence of numbers each followed by a unit"
            )
            raise NextestConfigurationError(message)
        unit = match["unit"]
        if unit not in UNIT_SECONDS:
            message = (
                f"unrecognized nextest duration {duration!r}: {unit!r} is not "
                f"a unit humantime accepts"
            )
            raise NextestConfigurationError(message)
        # humantime tolerates whitespace around the fractional point,
        # so the matched value can read "1 . 5"; float cannot.
        total += float("".join(match["value"].split())) * UNIT_SECONDS[unit]
        position = match.end()
    return total
