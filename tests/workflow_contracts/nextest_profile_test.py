"""The nextest profiles this repository's guide states, by value.

Split from ``timeout_ordering_test`` so neither module outgrows the
400-line limit ``AGENTS.md`` sets. The ordering assertions there hold
for a wide range of values, so a profile can drift to a budget nobody
chose without failing any of them; these pin the values the guide
records.

nextest's custom profiles inherit ``[profile.default]`` and its
``[[overrides]]`` are consulted, so restating the budgets in ``ci`` is
this repository's policy rather than a correctness requirement. The
contract holds the policy: a value present in one profile and not the
other is the state these tests exist to catch.
"""

from __future__ import annotations

import tomllib
import typing as typ

import pytest
from timeout_budgets import NEXTEST_CONFIG

#: The base per-test allowance both profiles must declare, as the guide
#: states it. Asserted by value rather than by shape, because the
#: ordering assertions hold for a wide range of values and would not
#: notice a profile drifting to a budget nobody chose.
BASE_SLOW_TIMEOUT: typ.Final[dict[str, object]] = {
    "period": "300s",
    "terminate-after": 1,
    "grace-period": "5s",
}

#: The whole-run budget both profiles must declare.
REQUIRED_GLOBAL_TIMEOUT: typ.Final[str] = "45m"

#: The overrides both profiles carry, keyed by the binary or filter they
#: exist for, with the whole ``slow-timeout`` each must declare. Both
#: are named so one cannot be dropped while the other keeps the profile
#: looking deliberate, and the table is compared whole rather than field
#: by field, so an added or removed key fails too. The grace period
#: matters as much as the period: it is what nextest waits before
#: killing a test it has signalled, and the watchdog above is sized to
#: cover it.
REQUIRED_OVERRIDES: typ.Final[dict[str, dict[str, object]]] = {
    "binary(behaviour_toolchain)": {
        "period": "30m",
        "terminate-after": 1,
        "grace-period": "5s",
    },
    "test(driver::ui::)": {
        "period": "10m",
        "terminate-after": 1,
        "grace-period": "5s",
    },
}


@pytest.fixture(scope="module")
def parsed_nextest() -> dict[str, typ.Any]:
    """Return the nextest configuration, parsed.

    Parsed rather than matched for the value assertions below: a
    commented-out budget reads as an active one to a regular expression,
    so a line nobody meant could satisfy an assertion about a value.

    Returns
    -------
    dict[str, typ.Any]
        The parsed document.
    """
    return tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))


@pytest.mark.parametrize("profile", ["default", "ci"], ids=str)
def test_each_profile_declares_the_base_allowance_the_guide_states(
    parsed_nextest: dict[str, typ.Any], profile: str
) -> None:
    """The ordering holds for many values; only one is documented.

    Asserting the ordering alone would let a profile drift to a budget
    nobody chose and the guide does not describe, while every comparison
    still passed. This pins the three fields the guide names, so a change
    to any of them has to change the guide in the same commit.
    """
    section = parsed_nextest["profile"][profile]
    assert section.get("slow-timeout") == BASE_SLOW_TIMEOUT, (
        f"[profile.{profile}] must declare the base slow-timeout the "
        f"developers' guide states, {BASE_SLOW_TIMEOUT}; got "
        f"{section.get('slow-timeout')}"
    )


@pytest.mark.parametrize("profile", ["default", "ci"], ids=str)
def test_each_profile_declares_the_whole_run_budget(
    parsed_nextest: dict[str, typ.Any], profile: str
) -> None:
    """Both profiles carry it, and carry the same one.

    `ci` would fall back to the default profile's budget if it declared
    none, so this is repository policy rather than a nextest
    requirement. The policy exists because the two profiles run
    different sets of tests, and a budget that governs one lane should
    be readable in the profile that lane selects.
    """
    section = parsed_nextest["profile"][profile]
    assert section.get("global-timeout") == REQUIRED_GLOBAL_TIMEOUT, (
        f"[profile.{profile}] must set global-timeout to "
        f"{REQUIRED_GLOBAL_TIMEOUT}; got {section.get('global-timeout')}"
    )


@pytest.mark.parametrize("profile", ["default", "ci"], ids=str)
@pytest.mark.parametrize(
    ("needle", "budget"), sorted(REQUIRED_OVERRIDES.items()), ids=str
)
def test_each_profile_states_the_overrides_its_own_tests_need(
    parsed_nextest: dict[str, typ.Any],
    profile: str,
    needle: str,
    budget: dict[str, object],
) -> None:
    """Both profiles declare both allowances, whole.

    nextest consults ``[[profile.default.overrides]]`` when ``ci`` is
    selected, so restating them is repository policy rather than a
    correctness requirement: ``ci`` is the profile that includes the
    toolchain binaries, and an allowance that only exists one section
    away is one a reader of this profile will not see. Restating it also
    makes a later divergence between the two profiles explicit rather
    than silent.

    The ``slow-timeout`` is compared as a whole table, so the grace
    period is asserted alongside the period and the multiplier. Checking
    the period alone would let the grace period be dropped, and the
    watchdog above these budgets is sized to cover exactly that wait.

    Only overrides that declare a budget are counted. The default
    profile matches ``binary(behaviour_toolchain)`` twice, once for the
    allowance and once to serialize three scenarios that contend on
    shared rustup state, and the second carries no timeout to assert.
    """
    overrides = parsed_nextest["profile"][profile].get("overrides") or []
    matching = [
        override
        for override in overrides
        if needle in str(override.get("filter", "")) and "slow-timeout" in override
    ]
    assert len(matching) == 1, (
        f"[profile.{profile}] must carry exactly one override matching "
        f"{needle!r} that declares a slow-timeout, found {len(matching)}; "
        f"each profile states its own allowances"
    )
    assert matching[0].get("slow-timeout") == budget, (
        f"[profile.{profile}]'s {needle!r} override must declare {budget}, got "
        f"{matching[0].get('slow-timeout')}"
    )
