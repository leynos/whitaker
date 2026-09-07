"""Readings of the nextest configuration the timeout ordering rests on.

Separated from `timeout_ordering_test` so the arithmetic and the
workflow reading are legible apart, and so neither module carries the
whole contract. Every function here takes a parsed profile and returns
a number, so they can be driven with configurations this repository
does not have, which is the only way to tell a correct reading from one
that happens to agree with the file in the tree.

The configuration is parsed with ``tomllib`` rather than matched as
text. A text match finds a key inside a comment, inside a ``filter``
string, or in a table nextest never consults, and reports a budget the
runner does not use. The commented-out ``global-timeout`` is the case
that matters most: this contract requires that tier to be present, and
a scraping reader would go on reporting a budget that had been switched
off.
"""

import re
import tomllib
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

NEXTEST_CONFIG = REPOSITORY_ROOT / ".config" / "nextest.toml"


class NextestConfigurationError(ValueError):
    """Raised when the configuration cannot be read as a set of budgets.

    Separate from a budget in the wrong order. A file that is not TOML,
    a profile that declares no ``slow-timeout``, or one whose
    ``global-timeout`` has been commented out, is a configuration this
    contract cannot reason about rather than one whose tiers are
    inverted.
    """


class Profile(typ.NamedTuple):
    """One nextest profile, as the runner reads it.

    Attributes
    ----------
    name : str
        The profile's name.
    own : dict[str, object]
        The ``[profile.<name>]`` table itself, without its overrides.
        The base allowance and an override's are different claims: an
        override bounds the tests its filter matches, and the profile's
        own bounds the rest, so a search across both would let the base
        allowance be deleted unnoticed.
    overrides : tuple[dict[str, object], ...]
        The profile's ``[[overrides]]`` entries, in file order.
    """

    name: str
    own: dict[str, object]
    overrides: tuple[dict[str, object], ...]

    def tables(self) -> tuple[dict[str, object], ...]:
        """Return every table the profile reads a budget from.

        Returns
        -------
        tuple of dict
            The profile's own table first, then each override.
        """
        return (self.own, *self.overrides)


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

    Raises
    ------
    NextestConfigurationError
        If the text is not a duration nextest would accept.
    """
    match = _DURATION.match(duration)
    if match is None:
        message = f"unrecognized nextest duration {duration!r}"
        raise NextestConfigurationError(message)
    return float(match["value"]) * _UNIT_SECONDS[match["unit"]]


def _table(value: object) -> dict[str, object]:
    """Return a parsed value as a table, or an empty one.

    Parameters
    ----------
    value : object
        Any value ``tomllib`` produced.

    Returns
    -------
    dict[str, object]
        The table, or an empty one when the value is not a table.
    """
    return dict(value) if isinstance(value, dict) else {}


def profiles(config_text: str) -> dict[str, Profile]:
    """Return each profile the configuration declares, keyed by name.

    Parsed rather than sliced out of the text, so a key inside a comment
    or a ``filter`` string is not read as configuration and a profile's
    own table stays distinguishable from its overrides.

    Parameters
    ----------
    config_text : str
        A nextest configuration file's text.

    Returns
    -------
    dict[str, Profile]
        Profile name to its table and overrides.

    Raises
    ------
    NextestConfigurationError
        If the text is not valid TOML.
    """
    try:
        parsed = tomllib.loads(config_text)
    except tomllib.TOMLDecodeError as error:
        message = f"the nextest configuration is not valid TOML: {error}"
        raise NextestConfigurationError(message) from error
    found: dict[str, Profile] = {}
    for name, raw in _table(parsed.get("profile")).items():
        table = _table(raw)
        overrides = tuple(
            _table(entry)
            for entry in table.get("overrides", [])
            if isinstance(entry, dict)
        )
        own = {key: value for key, value in table.items() if key != "overrides"}
        found[str(name)] = Profile(name=str(name), own=own, overrides=overrides)
    return found


def _slow_timeout(table: dict[str, object]) -> dict[str, object] | None:
    """Return one table's ``slow-timeout``, when it declares one.

    Parameters
    ----------
    table : dict[str, object]
        A profile's own table or one of its overrides.

    Returns
    -------
    dict[str, object] or None
        The inline table, or None when the key is absent or is a bare
        duration, which sets a warning period and terminates nothing.
    """
    value = table.get("slow-timeout")
    return dict(value) if isinstance(value, dict) else None


def bounds_a_single_test(profile: Profile) -> bool:
    """Return whether the profile's own table terminates a slow test.

    ``terminate-after`` is optional, and without it nextest marks a test
    slow and lets it run on. An override satisfies the profile as a
    whole while leaving every test the override does not match with no
    bound at all, so only the profile's own table counts here.

    Parameters
    ----------
    profile : Profile
        The profile to read.

    Returns
    -------
    bool
        True when the profile's own ``slow-timeout`` sets
        ``terminate-after``.
    """
    table = _slow_timeout(profile.own)
    return table is not None and table.get("terminate-after") is not None


def largest_period(profile: Profile) -> float:
    """Return the longest per-test allowance a profile sets.

    Parameters
    ----------
    profile : Profile
        The profile to read.

    Returns
    -------
    float
        The longest per-test budget, in seconds.

    Raises
    ------
    NextestConfigurationError
        If the profile declares no ``slow-timeout`` period at all.
    """
    periods = [
        seconds(period)
        for table in profile.tables()
        if (entry := _slow_timeout(table)) is not None
        and isinstance(period := entry.get("period"), str)
    ]
    if not periods:
        message = (
            f"[profile.{profile.name}] declares no slow-timeout period, so no "
            f"test is bounded and there is no per-test tier to compare against"
        )
        raise NextestConfigurationError(message)
    return max(periods)


def termination_allowance(profile: Profile) -> float:
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
    profile : Profile
        The profile to read.

    Returns
    -------
    float
        The configured grace period plus the safety margin.
    """
    periods = [
        seconds(grace)
        for table in profile.tables()
        if (entry := _slow_timeout(table)) is not None
        and isinstance(grace := entry.get("grace-period"), str)
    ]
    largest = max(periods, default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS)
    return largest + TERMINATION_SAFETY_MARGIN_SECONDS


def global_timeout(profile: Profile) -> float:
    """Return a profile's whole-run budget in seconds.

    Read from the profile's own table alone: ``global-timeout`` is a
    profile key, and an ``[[overrides]]`` entry cannot carry one.

    Parameters
    ----------
    profile : Profile
        The profile to read.

    Returns
    -------
    float
        The whole-run budget.

    Raises
    ------
    NextestConfigurationError
        If the profile declares no ``global-timeout``.
    """
    budget = profile.own.get("global-timeout")
    if not isinstance(budget, str):
        message = (
            f"[profile.{profile.name}] must set global-timeout; without it the "
            f"whole-run budget is unbounded and only the job timer ends a hung "
            f"run, by cancelling it and discarding the log"
        )
        raise NextestConfigurationError(message)
    return seconds(budget)


def required_ceiling(profile: Profile) -> float:
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
    profile : Profile
        The profile to read.

    Returns
    -------
    float
        The smallest acceptable ceiling, in seconds.
    """
    return (
        global_timeout(profile)
        + termination_allowance(profile)
        + OUTSIDE_SUITE_ALLOWANCE_SECONDS
        + CEILING_MARGIN_SECONDS
    )
