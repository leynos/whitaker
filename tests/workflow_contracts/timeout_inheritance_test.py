"""How a custom profile inherits the default, and how a count scales a period.

Split from `timeout_reading_test` to keep both under 400 lines. nextest reads a
custom profile through the default: `ci` consults every default override after
its own and takes the default `slow-timeout` when it declares none. And a test
is terminated after `terminate-after` periods, not one. Each case here is a
configuration the repository does not have, so the arithmetic is driven where
the tree alone would agree with a wrong reading.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest
from timeout_budgets import (
    TERMINATION_SAFETY_MARGIN_SECONDS,
    bounds_a_single_test,
    global_timeout,
    largest_period,
    profiles,
    termination_allowance,
)

_DEFAULT_ONLY_OVERRIDE = (
    "[profile.default]\n"
    'slow-timeout = { period = "300s", terminate-after = 1, grace-period = "5s" }\n'
    'global-timeout = "45m"\n'
    "\n"
    "[[profile.default.overrides]]\n"
    'filter = "binary(long)"\n'
    'slow-timeout = { period = "50m", terminate-after = 1, grace-period = "90s" }\n'
    "\n"
    "[profile.ci]\n"
    'slow-timeout = { period = "300s", terminate-after = 1, grace-period = "5s" }\n'
    'global-timeout = "45m"\n'
)


def test_a_custom_profile_is_bounded_by_the_default_overrides_it_consults() -> None:
    """nextest consults `[[profile.default.overrides]]` for a `ci` test.

    So a default-only override longer than `ci`'s whole-run budget breaks
    `ci`'s ordering even though `ci` declares nothing of the kind, and its
    grace period is one `ci` waits too. Reading `ci`'s own tables alone
    answered 300 s and 5 s here.
    """
    ci = profiles(_DEFAULT_ONLY_OVERRIDE)["ci"]
    assert largest_period(ci) == pytest.approx(3000.0), (
        "ci must take the default-only override's period"
    )
    assert largest_period(ci) > global_timeout(ci), "the ordering must fail"
    assert termination_allowance(ci) == pytest.approx(
        90 + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "ci must wait the default-only override's grace period"


def test_inheritance_does_not_lend_a_custom_profile_its_own_bound() -> None:
    """Whether `ci` terminates an unmatched test is still `ci`'s own answer."""
    unbounded = profiles(
        _DEFAULT_ONLY_OVERRIDE.replace(
            '[profile.ci]\nslow-timeout = { period = "300s", terminate-after = 1, grace-period = "5s" }\n',
            '[profile.ci]\nslow-timeout = { period = "300s" }\n',
        )
    )["ci"]
    assert not bounds_a_single_test(unbounded), (
        "inheritance must not lend ci the default's terminate-after"
    )


def test_a_custom_profile_s_own_slow_timeout_replaces_the_default_one() -> None:
    """A setting `ci` declares is `ci`'s; the default's is inherited only when absent.

    Lending the default's table regardless would size `ci`'s ceiling for a
    ninety-second grace period nextest never applies to it.
    """
    config = (
        "[profile.default]\n"
        'slow-timeout = { period = "300s", terminate-after = 1, grace-period = "90s" }\n'
        "\n"
        "[profile.ci]\n"
        'slow-timeout = { period = "300s", terminate-after = 1, grace-period = "5s" }\n'
        "\n"
        "[profile.lean]\n"
        'global-timeout = "45m"\n'
    )
    parsed = profiles(config)
    assert termination_allowance(parsed["ci"]) == pytest.approx(
        5 + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "ci's own slow-timeout must replace the default's"
    assert termination_allowance(parsed["lean"]) == pytest.approx(
        90 + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "a profile without slow-timeout must inherit the default's"


@pytest.mark.parametrize(
    ("terminate_after", "expected"),
    [
        pytest.param("terminate-after = 3", 3600.0, id="three-periods"),
        pytest.param("terminate-after = 1", 1200.0, id="one-period"),
        pytest.param("terminate-after = true", 1200.0, id="a-boolean"),
        pytest.param("terminate-after = 0", 1200.0, id="non-positive"),
    ],
)
def test_the_per_test_tier_is_the_period_times_its_count(
    terminate_after: str, expected: float
) -> None:
    """Twenty minutes three times is an hour, which a 45 m budget cannot hold."""
    profile = profiles(
        "[profile.example]\n"
        f'slow-timeout = {{ period = "20m", {terminate_after} }}\n'
        'global-timeout = "45m"\n'
    )["example"]
    assert largest_period(profile) == pytest.approx(expected), (
        f"{terminate_after} must give a per-test tier of {expected} s"
    )
    assert (largest_period(profile) > global_timeout(profile)) is (expected > 2700), (
        "the ordering must fail exactly when the tier exceeds the whole-run budget"
    )
