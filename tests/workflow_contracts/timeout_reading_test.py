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

import fractions

import pytest
from suite_commands import _disguised_suite_lines, _suite_commands
from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    Profile,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    NextestConfigurationError,
    bounds_a_single_test,
    global_timeout,
    largest_period,
    profiles,
    required_ceiling,
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


def _example(*lines: str) -> Profile:
    """Return the parsed ``[profile.example]`` those lines declare."""
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


def test_the_allowance_is_exact_rather_than_nearest_representable() -> None:
    """A tenth of a second is not a float, and the ceiling is compared to it.

    `seconds` answers a `Fraction` precisely so a budget can be compared
    without a tolerance. A default or a margin left as a `float` puts the whole
    sum back on binary floating point, where `0.1 + 60` is 60.099999999999994,
    and `required_ceiling` then propagates a value that can round below the
    exact sum it is meant to cover. Asserted with `==` rather than `approx`,
    because `approx` is exactly what would hide it.
    """
    parsed = _example(
        'slow-timeout = { period = "300s", terminate-after = 1, '
        'grace-period = "100ms" }'
    )
    assert termination_allowance(parsed) == fractions.Fraction(1, 10) + (
        TERMINATION_SAFETY_MARGIN_SECONDS
    ), "a tenth of a second plus the margin must be exact"
    assert termination_allowance(parsed) != 0.1 + float(
        TERMINATION_SAFETY_MARGIN_SECONDS
    ), "the float sum is the value this contract exists to avoid"
    # The ceiling is what a lane is compared against, so the exactness has to
    # survive the two constants added after the allowance. The expected value
    # is written out rather than composed from those constants: composing it
    # would make the mutation that turns one of them back into a float change
    # both sides of the comparison, and the assertion would pass either way.
    #
    # 45 m + 0.1 s + 60 s + 15 m + 15 m, in seconds.
    exact = fractions.Fraction(2700) + fractions.Fraction(1, 10)
    exact += fractions.Fraction(60) + fractions.Fraction(900) * 2
    whole = _example(
        'slow-timeout = { period = "300s", terminate-after = 1, '
        'grace-period = "100ms" }',
        'global-timeout = "45m"',
    )
    assert required_ceiling(whole) == exact, (
        "the required ceiling must be the exact sum, not the nearest float"
    )


def test_an_omitted_grace_period_contributes_nextest_s_default() -> None:
    """An omission is a ten-second wait, not a zero-second one.

    A profile that declares five seconds of its own and an override that
    declares none waits ten for anything the override matches. Reading
    only the declared periods answered five, so the ceiling above was
    sized for a wait five seconds shorter than nextest's.
    """
    config_text = (
        "[profile.example]\n"
        'slow-timeout = { period = "300s", terminate-after = 1, '
        'grace-period = "5s" }\n'
        "\n"
        "[[profile.example.overrides]]\n"
        'filter = "binary(probe)"\n'
        'slow-timeout = { period = "600s", terminate-after = 1 }\n'
    )
    parsed = profiles(config_text)["example"]
    assert termination_allowance(parsed) == pytest.approx(
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


@pytest.mark.parametrize(
    ("run", "expected"),
    [
        pytest.param("make coverage", ["make coverage"], id="one-command"),
        pytest.param(
            "make coverage\nmake test NEXTEST_PROFILE=ci",
            ["make coverage", "make test NEXTEST_PROFILE=ci"],
            id="two-commands-under-two-profiles",
        ),
        pytest.param(
            "make coverage\nmake test-doc\nmake test NEXTEST_PROFILE=ci",
            ["make coverage", "make test NEXTEST_PROFILE=ci"],
            id="a-non-suite-command-between-them",
        ),
        pytest.param("make test-doc", [], id="no-suite-command"),
        pytest.param(
            "make test \\\n  NEXTEST_PROFILE=ci",
            ["make test NEXTEST_PROFILE=ci"],
            id="a-command-continued-across-two-lines",
        ),
        pytest.param(
            "make coverage \\\n  --keep-going\nmake test-doc",
            ["make coverage --keep-going"],
            id="a-continuation-followed-by-another-command",
        ),
    ],
)
def test_every_suite_command_in_a_step_becomes_a_lane(
    run: str, expected: list[str]
) -> None:
    """A step running the suite twice is two lanes, not one.

    The reading returned the first match, so a `run` block invoking
    `make coverage` and then `make test NEXTEST_PROFILE=ci` reported the
    default-profile lane alone and the `ci` invocation was held to no
    ceiling at all. That is the case the lane discovery exists to cover,
    since the two run under different profiles with different budgets.

    A backslash continuation is the same fault seen from the other
    side: read as two physical lines, ``make test \\`` followed by
    ``NEXTEST_PROFILE=ci`` reported a lane whose command carried no
    profile, so the lane was held to the default profile's ceilings
    rather than to the ones it runs under.

    Driven here rather than against a workflow because
    ``lane_deduplication_contract_test`` forbids a second plain test
    step in a Linux job, so the tree cannot carry the shape this reading
    has to handle.
    """
    assert _suite_commands(run) == expected, (
        f"{run!r} must yield {expected!r}; every suite command in a step is a "
        f"lane, because each may name its own profile"
    )


@pytest.mark.parametrize(
    ("run", "expected"),
    [
        pytest.param(
            "make test-doc; make test",
            ["make test-doc; make test"],
            id="a-suite-command-after-a-non-suite-one",
        ),
        pytest.param(
            "make test || true",
            ["make test || true"],
            id="a-verdict-discarded",
        ),
        pytest.param("make test-doc", [], id="a-plain-non-suite-command"),
        pytest.param(
            "make test-doc --nocapture",
            [],
            id="a-non-suite-command-with-arguments",
        ),
    ],
)
def test_a_line_naming_the_suite_without_plainly_running_it_is_reported(
    run: str, expected: list[str]
) -> None:
    """A line the reading cannot judge must be reported, not dropped.

    The non-suite commands are excluded by prefix, and the exclusion
    used to apply to any line beginning with one. `make test-doc; make
    test` therefore escaped both halves of the contract: it was not a
    lane, so no ceiling was checked against it, and it was not
    disguised either, so nothing said so. The exclusion now holds only
    for a line that runs that command and nothing else.
    """
    assert _disguised_suite_lines(run) == expected, (
        f"{run!r} must be reported as {expected!r}; a line this reading "
        f"cannot judge is neither a lane nor safely ignored"
    )
