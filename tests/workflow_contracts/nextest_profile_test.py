"""The nextest profiles this repository's guide states, by value.

Split from ``timeout_ordering_test``, and again from
``compile_contract_allowance_test``, so no module outgrows the 400-line
limit ``AGENTS.md`` sets. The ordering assertions there hold for a wide
range of values, so a profile can drift to a budget nobody chose without
failing any of them; these pin the values the guide records.
``NEXTEST`` and ``REQUIRED_OVERRIDES`` are read from here by the
compile-contract module, which asks a different question of the same
configuration.

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
from suite_lanes import PROFILE_VARIABLE, _scopes_declaring_profile
from timeout_budgets import NEXTEST_CONFIG

#: The configuration as nextest would read it, parsed once. Not a
#: fixture, because it is a property of the tree rather than of one
#: test's arrangement, and because the compile-contract module imports
#: it.
NEXTEST: typ.Final[dict[str, typ.Any]] = tomllib.loads(
    NEXTEST_CONFIG.read_text(encoding="utf-8")
)

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


class Allowance(typ.NamedTuple):
    """One override's whole contribution to a profile.

    Attributes
    ----------
    slow_timeout : dict[str, object]
        The whole ``slow-timeout`` table the override must declare.
    test_group : str or None
        The group it must name, or None when it names none. The budget
        and the group are one claim: a test allowed thirty minutes but
        no longer serialized runs concurrently with its siblings on the
        shared target directory, which is what the group exists to stop.
    retries : dict[str, object] or None
        The whole ``retries`` table, or None when the override declares
        none. Part of the same claim: the Dylint UI harnesses retry a
        transient Windows rename failure, and `tests/nextest_ui_filter.rs`
        requires the key, so a change to its count, backoff or delay
        would otherwise pass every assertion here.
    """

    slow_timeout: dict[str, object]
    test_group: str | None
    retries: dict[str, object] | None = None


#: The overrides both profiles carry, keyed by the binary or filter they
#: exist for, with the whole ``slow-timeout`` each must declare. Both
#: are named so one cannot be dropped while the other keeps the profile
#: looking deliberate, and the table is compared whole rather than field
#: by field, so an added or removed key fails too. The grace period
#: matters as much as the period: it is what nextest waits before
#: killing a test it has signalled, and the watchdog above is sized to
#: cover it.
#:
#: The filters are stated whole and matched exactly, per profile. A
#: substring would accept a narrowed one: dropping a clause from the
#: `ui` disjunction leaves the needle intact while the tests that clause
#: named lose the ten-minute allowance and fall back to the base sixty
#: seconds. The two profiles genuinely differ here, the default one
#: naming eight more `test(...)` clauses than `ci`, which is exactly the
#: divergence a shared needle would have hidden.
REQUIRED_OVERRIDES: typ.Final[dict[str, dict[str, Allowance]]] = {
    "default": {
        ("binary(behaviour_toolchain)"): Allowance(
            slow_timeout={
                "period": "30m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            # The default profile serializes these in a second override
            # matching three named scenarios, so this one names none.
            test_group=None,
        ),
        (
            "test(driver::ui::) | "
            "test(tests::ui::) | "
            "test(ui::ui) | "
            "test(sha2_0_11_pre_migration_patterns_fail_to_compile) | "
            "test(example_compiles_without_diagnostics) | "
            "test(example_harness_collects_call_site_evidence) | "
            "test(trybuild_fixtures_compile_without_diagnostics) | "
            "test(ui::example_compiles_under_test_harness) | "
            "test(ui::hand_written_test_companion_does_not_exempt_parent_"
            "function) | "
            "test(ui::rstest_unwrap_outside_tests_still_fails_in_non_harness_"
            "code) | "
            "test(ui::rstest_empty_companion_does_not_exempt_parent_function)"
            " | "
            "test(ui::aliased_test_crate_non_companion_does_not_exempt_parent_"
            "function) | "
            "(binary(ui) & test(=ui))"
        ): Allowance(
            slow_timeout={
                "period": "10m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
            retries={"backoff": "exponential", "count": 2, "delay": "5s"},
        ),
    },
    "ci": {
        ("binary(behaviour_toolchain)"): Allowance(
            slow_timeout={
                "period": "30m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-toolchain-installs",
        ),
        (
            "test(driver::ui::) | "
            "test(tests::ui::) | "
            "test(ui::ui) | "
            "test(sha2_0_11_pre_migration_patterns_fail_to_compile) | "
            "test(trybuild_fixtures_compile_without_diagnostics) | "
            "(binary(ui) & test(=ui))"
        ): Allowance(
            slow_timeout={
                "period": "10m",
                "terminate-after": 1,
                "grace-period": "5s",
            },
            test_group="serial-dylint-ui",
        ),
    },
}

#: Every group an override may name, with the whole table declaring it.
#: A group a profile names and ``[test-groups]`` does not declare is a
#: configuration nextest refuses, and one declared without
#: ``max-threads = 1`` serializes nothing: the UI harnesses would then
#: build lint libraries concurrently against the shared target
#: directory, which is the race the groups exist to stop.
REQUIRED_TEST_GROUPS: typ.Final[dict[str, dict[str, object]]] = {
    "serial-dylint-ui": {"max-threads": 1},
    "serial-toolchain-installs": {"max-threads": 1},
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


@pytest.mark.parametrize(
    ("profile", "needle", "allowance"),
    [
        pytest.param(profile, needle, allowance, id=f"{profile}-{needle[:24]}")
        for profile, overrides in sorted(REQUIRED_OVERRIDES.items())
        for needle, allowance in sorted(overrides.items())
    ],
)
def test_each_profile_states_the_overrides_its_own_tests_need(
    parsed_nextest: dict[str, typ.Any],
    profile: str,
    needle: str,
    allowance: Allowance,
) -> None:
    """Both profiles declare both allowances, whole.

    nextest consults ``[[profile.default.overrides]]`` when ``ci`` is
    selected, so restating them is repository policy rather than a
    correctness requirement: ``ci`` is the profile that includes the
    toolchain binaries, and an allowance that only exists one section
    away is one a reader of this profile will not see. Restating it also
    makes a later divergence between the two profiles explicit rather
    than silent.

    Each policy key is compared as a whole table, so the grace period is
    asserted alongside the period and the multiplier, and the retry
    backoff and delay alongside the count. Checking
    the period alone would let the grace period be dropped, and the
    watchdog above these budgets is sized to cover exactly that wait.

    The filter is compared whole and exactly, per profile. A substring
    would accept a narrowed filter: dropping a clause from the `ui`
    disjunction leaves any needle intact while the tests that clause
    named lose the ten-minute allowance and fall back to the base sixty
    seconds.

    Only overrides that declare a budget are counted. The default
    profile matches ``binary(behaviour_toolchain)`` twice, once for the
    allowance and once to serialize three scenarios that contend on
    shared rustup state, and the second carries no timeout to assert.
    """
    overrides = parsed_nextest["profile"][profile].get("overrides") or []
    matching = [
        override
        for override in overrides
        if override.get("filter") == needle and "slow-timeout" in override
    ]
    assert len(matching) == 1, (
        f"[profile.{profile}] must carry exactly one override whose filter is "
        f"{needle!r} and which declares a slow-timeout, found {len(matching)}; "
        f"each profile states its own allowances"
    )
    assert matching[0].get("slow-timeout") == allowance.slow_timeout, (
        f"[profile.{profile}]'s {needle!r} override must declare "
        f"{allowance.slow_timeout}, got {matching[0].get('slow-timeout')}"
    )
    assert matching[0].get("test-group") == allowance.test_group, (
        f"[profile.{profile}]'s {needle!r} override must name test-group "
        f"{allowance.test_group!r}, got {matching[0].get('test-group')!r}; a "
        f"longer allowance without the group lets the tests it covers run "
        f"concurrently against the shared target directory"
    )
    assert matching[0].get("retries") == allowance.retries, (
        f"[profile.{profile}]'s {needle!r} override must declare retries "
        f"{allowance.retries}, got {matching[0].get('retries')}; the Dylint UI "
        f"harnesses retry a transient Windows rename failure, and "
        f"`tests/nextest_ui_filter.rs` requires the key"
    )


@pytest.mark.parametrize(
    ("group", "declaration"),
    sorted(REQUIRED_TEST_GROUPS.items()),
    ids=sorted(REQUIRED_TEST_GROUPS),
)
def test_each_group_the_overrides_name_is_declared_serial(
    parsed_nextest: dict[str, typ.Any], group: str, declaration: dict[str, object]
) -> None:
    """A group exists to serialize, and only `max-threads = 1` does that.

    The overrides above name these groups, and nextest refuses a
    configuration naming a group `[test-groups]` does not declare. A
    group declared with any other thread count parses, reads as
    deliberate, and serializes nothing, so the lint-library builds it
    covers race on the shared target directory exactly as they did
    before the group existed. Compared as a whole table so an added key
    fails too.
    """
    declared = parsed_nextest.get("test-groups", {})
    assert declared.get(group) == declaration, (
        f"[test-groups] must declare {group} as {declaration}; got "
        f"{declared.get(group)}"
    )


def test_no_override_names_a_group_the_configuration_lacks(
    parsed_nextest: dict[str, typ.Any],
) -> None:
    """Every group named anywhere is declared, including ones not pinned above.

    The table above pins the groups this contract requires; this catches
    an override added later that names a group nobody declared, which
    nextest refuses at startup rather than at the test that needed it.
    """
    declared = set(parsed_nextest.get("test-groups", {}))
    named = {
        group
        for section in parsed_nextest["profile"].values()
        for override in section.get("overrides") or []
        if (group := override.get("test-group")) is not None
    }
    assert named <= declared, (
        f"every test-group an override names must be declared in "
        f"[test-groups]; {sorted(named - declared)} are not"
    )


def test_no_workflow_selects_a_profile_through_the_environment() -> None:
    """The lane reader decides a lane's profile from its command line.

    `timeout_ordering_test` judges each lane against the profile it
    runs, and it learns that profile from `NEXTEST_PROFILE=ci` appearing
    in the command. That reading is complete only while the variable is
    never set any other way. A job-level `env` block naming it would put
    every lane in that job under `ci` while the reader still judged them
    under `default`, so the lane would be checked against the wrong
    budgets and pass.

    Nothing fails when that happens: both profiles carry a full set of
    values, so the comparison succeeds against the wrong ones. This is
    the assertion that makes the command-line reading true rather than
    merely true so far.

    The shape is not hypothetical. It is how a sibling repository
    selects its coverage profile, because the shared coverage action
    takes no profile input and the environment is the only lever.
    """
    declared = _scopes_declaring_profile()
    assert not declared, (
        f"{PROFILE_VARIABLE} is declared in an env block at {declared}; the "
        f"lane reader takes a lane's profile from its command line, so a "
        f"lane there would be judged under the wrong profile and checked "
        f"against budgets it does not run under. Pass it on the command "
        f"line, or teach the reader to resolve the environment first"
    )
