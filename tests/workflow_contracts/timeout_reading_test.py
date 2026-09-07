"""What the nextest reading counts as configuration, and what it does not.

The contract in ``timeout_ordering_test`` compares budgets read out of
``.config/nextest.toml``. Those readings can be wrong while the file is
right, and this repository's own configuration cannot expose the ways
they can be: nothing here is commented out and no ``filter`` string
names a timeout key.

The reading parses the file with ``tomllib``. A text match would find a
key inside a comment, inside a ``filter`` string, or in a table nextest
never consults. The ``global-timeout`` case is the one that matters
most, because the contract requires that tier to be present, so a
scraping reader would go on reporting a budget somebody had switched
off.
"""

import pytest
from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    NextestConfigurationError,
    bounds_a_single_test,
    global_timeout,
    largest_period,
    profiles,
    termination_allowance,
)


def _profile(*lines: str) -> str:
    """Return a profile declaring the given keys.

    Parameters
    ----------
    *lines : str
        Lines to put inside ``[profile.example]``.

    Returns
    -------
    str
        A configuration document.
    """
    return "\n".join(("[profile.example]", *lines)) + "\n"


def _example(*lines: str):
    """Return the parsed profile for those lines.

    Parameters
    ----------
    *lines : str
        Lines to put inside ``[profile.example]``.

    Returns
    -------
    Profile
        The parsed profile.
    """
    return profiles(_profile(*lines))["example"]


def test_a_commented_out_global_timeout_is_absent() -> None:
    """Tier two must read as missing when it has been switched off.

    This is the reading the contract's presence assertion rests on. A
    text match would keep reporting the budget from the comment, so the
    tier could be commented out and the four-tier contract would go on
    passing with three.
    """
    with pytest.raises(NextestConfigurationError, match=r"global-timeout"):
        global_timeout(_example('# global-timeout = "45m"'))
    assert global_timeout(_example('global-timeout = "45m"')) == pytest.approx(2700.0)


def test_a_commented_out_slow_timeout_is_not_a_budget() -> None:
    """A comment is not configuration, and TOML is what says so.

    A reader that scraped the text would find the commented period and
    report an allowance from a line nextest never reads, so deleting the
    live entry and leaving the comment behind would look like a change
    of value rather than the loss of a tier.
    """
    live = _example(
        '# slow-timeout = { period = "30m", terminate-after = 1 }',
        'slow-timeout = { period = "300s", terminate-after = 1 }',
    )
    assert largest_period(live) == pytest.approx(300.0)
    with pytest.raises(NextestConfigurationError, match=r"no slow-timeout period"):
        largest_period(_example('# slow-timeout = { period = "300s" }'))


def test_a_commented_out_grace_period_is_not_in_force() -> None:
    """The grace period sets the floor the ceiling is measured against.

    A scraped comment would raise the termination allowance and with it
    the ceiling this contract demands, so the file would appear to ask
    more of the tier above it than nextest actually does.
    """
    parsed = _example(
        '# slow-timeout = { period = "300s", terminate-after = 1, '
        'grace-period = "30m" }',
        'slow-timeout = { period = "300s", terminate-after = 1, grace-period = "5s" }',
    )
    assert termination_allowance(parsed) == pytest.approx(
        5.0 + TERMINATION_SAFETY_MARGIN_SECONDS
    )
    unset = _example('slow-timeout = { period = "300s", terminate-after = 1 }')
    assert termination_allowance(unset) == pytest.approx(
        NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS + TERMINATION_SAFETY_MARGIN_SECONDS
    )


def test_a_filter_naming_a_timeout_key_is_not_a_budget() -> None:
    """An override's ``filter`` is a string, not configuration.

    A binary named after one of these keys would be matched by a text
    search and read as a budget nextest never applies.
    """
    config_text = (
        "[profile.example]\n"
        'slow-timeout = { period = "300s", terminate-after = 1 }\n'
        'global-timeout = "45m"\n'
        "\n"
        "[[profile.example.overrides]]\n"
        'filter = "binary(global_timeout_probe) | binary(grace_period_probe)"\n'
        'slow-timeout = { period = "600s", terminate-after = 1 }\n'
    )
    parsed = profiles(config_text)["example"]
    assert largest_period(parsed) == pytest.approx(600.0)
    assert global_timeout(parsed) == pytest.approx(2700.0)
    assert termination_allowance(parsed) == pytest.approx(
        NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS + TERMINATION_SAFETY_MARGIN_SECONDS
    )


def test_only_the_profile_s_own_table_bounds_an_unmatched_test() -> None:
    """An override bounds what its filter matches, and nothing else.

    A profile whose only ``terminate-after`` sits in an override leaves
    every test the override does not match running with no bound at all,
    which is the state this contract exists to detect.
    """
    only_in_override = profiles(
        "[profile.example]\n"
        'slow-timeout = { period = "300s" }\n'
        "\n"
        "[[profile.example.overrides]]\n"
        'filter = "binary(slow)"\n'
        'slow-timeout = { period = "600s", terminate-after = 1 }\n'
    )["example"]
    assert not bounds_a_single_test(only_in_override)
    assert bounds_a_single_test(
        _example('slow-timeout = { period = "300s", terminate-after = 1 }')
    )
