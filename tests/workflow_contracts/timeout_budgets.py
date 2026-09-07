"""Readings of the nextest configuration the timeout ordering rests on.

Separated from `timeout_ordering_test` so the arithmetic and the
workflow reading are legible apart, and so neither module carries the
whole contract. Every function here takes text and returns a number, so
they can be driven with configurations this repository does not have,
which is the only way to tell a correct reading from one that happens
to agree with the file in the tree.
"""

import re
import typing as typ

from ubicloud_workflow_support import REPOSITORY_ROOT

#: Everything the job timer covers that the whole-run budget does not:
#: the toolchain setup, the build before nextest starts its clock, and
#: whatever follows the suite.
OUTSIDE_SUITE_ALLOWANCE_SECONDS: typ.Final[float] = 15 * 60.0

#: How far a ceiling must sit above the sum it contains, rather than
#: merely reaching it. A ceiling equal to that sum cancels the job at
#: the moment the innermost timer would have reported the overrun, and
#: the report is the only thing that makes an overrun actionable.
CEILING_MARGIN_SECONDS: typ.Final[float] = 15 * 60.0

#: What nextest allows a test between `SIGTERM` and `SIGKILL` when a
#: profile names no `grace-period`. Both profiles here name five
#: seconds, so this is a fallback rather than the value in force.
NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS: typ.Final[float] = 10.0

#: Added to that grace period to cover the teardown and report writing
#: that follow it. A separate term rather than a floor over the two, so
#: raising a grace period raises the requirement instead of vanishing
#: into it.
TERMINATION_SAFETY_MARGIN_SECONDS: typ.Final[float] = 60.0

_DURATION: typ.Final[re.Pattern[str]] = re.compile(
    r"^\s*(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>ms|s|m|h)\s*$"
)

_UNIT_SECONDS: typ.Final[dict[str, float]] = {
    "ms": 0.001,
    "s": 1.0,
    "m": 60.0,
    "h": 3600.0,
}

#: ``period`` as its own key. The lookbehind keeps ``grace-period`` out:
#: the two sit in the same inline table, and a substring match would read
#: a termination allowance as a per-test budget.
_PERIOD: typ.Final[re.Pattern[str]] = re.compile(r'(?<![\w-])period\s*=\s*"([^"]+)"')

_GRACE_PERIOD: typ.Final[re.Pattern[str]] = re.compile(r'grace-period\s*=\s*"([^"]+)"')

NEXTEST_CONFIG = REPOSITORY_ROOT / ".config" / "nextest.toml"


def seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"45m"``.

    Returns
    -------
    float
        The duration in seconds.
    """
    match = _DURATION.match(duration)
    assert match is not None, f"unrecognized nextest duration {duration!r}"
    return float(match["value"]) * _UNIT_SECONDS[match["unit"]]


def profile_blocks(config_text: str) -> dict[str, str]:
    """Return each profile's own text, keyed by profile name.

    Read textually rather than through a TOML parser, because every
    assertion below must be attached to the profile it belongs to, and
    both profiles here carry the same keys with different overrides.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    dict[str, str]
        Profile name to the text of its section and its overrides.
    """
    blocks: dict[str, list[str]] = {}
    current: str | None = None
    for line in config_text.splitlines(keepends=True):
        header = re.match(r"^\[\[?profile\.([A-Za-z0-9_-]+)", line)
        if header is not None:
            current = header[1]
            blocks.setdefault(current, [])
        elif line.startswith("["):
            current = None
        if current is not None:
            blocks[current].append(line)
    return {name: "".join(lines) for name, lines in blocks.items()}


def root_section(block: str) -> str:
    """Return one profile's own text, without its overrides.

    The base allowance and an override's are different claims: an
    override bounds the tests its filter matches, and the profile's own
    bounds the rest. A search over the whole block conflates them, so
    removing the base allowance would go unnoticed as long as any
    override remained, and every unmatched test would run unbounded.

    Parameters
    ----------
    block : str
        One profile's section and its overrides.

    Returns
    -------
    str
        The text before the first ``[[profile.<name>.overrides]]``.
    """
    marker = re.search(r"^\[\[profile\.", block, re.MULTILINE)
    return block if marker is None else block[: marker.start()]


def largest_period(block: str) -> float:
    """Return the longest per-test allowance a profile sets.

    Parameters
    ----------
    block : str
        One profile's text.

    Returns
    -------
    float
        The longest per-test budget, in seconds.
    """
    periods = _PERIOD.findall(block)
    assert periods, "the profile must set at least one slow-timeout period"
    return max(seconds(period) for period in periods)


def termination_allowance(block: str) -> float:
    """Return the time to allow for stopping the run, in seconds.

    Two terms, added rather than maximized, because they answer
    different questions. The first is what nextest will spend: on Linux
    and macOS it signals the process group and waits
    ``slow-timeout.grace-period`` before killing it, read from the
    configuration so a profile that raised it raises the requirement
    too. On Windows termination is immediate and that term is zero.

    The second is a safety margin for the teardown and report writing
    that follow. Taking the larger of the two, as an earlier version of
    this function did while its docstring already described the sum, hid
    which was which: a configuration with a ninety-second grace period
    and one with none produced the same answer for different reasons,
    and a grace period below the margin was absorbed entirely.

    Parameters
    ----------
    block : str
        One profile's text.

    Returns
    -------
    float
        The configured grace period plus the safety margin.
    """
    periods = _GRACE_PERIOD.findall(block)
    largest = max(
        (seconds(period) for period in periods),
        default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    )
    return largest + TERMINATION_SAFETY_MARGIN_SECONDS


def global_timeout(block: str) -> float:
    """Return a profile's whole-run budget in seconds.

    Parameters
    ----------
    block : str
        One profile's text.

    Returns
    -------
    float
        The whole-run budget.
    """
    match = re.search(r'^global-timeout\s*=\s*"([^"]+)"', block, re.MULTILINE)
    assert match is not None, (
        "the profile must set global-timeout; without it the whole-run budget "
        "is unbounded and only the job timer ends a hung run, by cancelling "
        "it and discarding the log"
    )
    return seconds(match[1])


def required_ceiling(block: str) -> float:
    """Return the smallest acceptable job ceiling for one profile.

    Four terms. The whole-run budget is what the suite may spend. The
    termination allowance is what nextest needs to stop it. The outside
    allowance is the build and the steps either side, which the job
    timer covers and the whole-run budget does not. The margin is added
    because a ceiling equal to that sum cancels the job at the moment
    nextest would have reported the overrun, and the report is the only
    thing that makes an overrun actionable.

    Parameters
    ----------
    block : str
        The profile's section of the nextest configuration.

    Returns
    -------
    float
        The smallest acceptable ceiling, in seconds.
    """
    return (
        global_timeout(block)
        + termination_allowance(block)
        + OUTSIDE_SUITE_ALLOWANCE_SECONDS
        + CEILING_MARGIN_SECONDS
    )
