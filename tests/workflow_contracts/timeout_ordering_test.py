"""Contract for the timers that can end a test run.

Four budgets can end a run, each set somewhere different, and they only
work if each sits above the one inside it. Three of the four were missing
or partial here.

No profile set a base `slow-timeout`, so an ordinary hung test was
reported slow for ever rather than terminated, on every lane. The named
exceptions were bounded: `[[profile.default.overrides]]` are consulted
when `ci` is selected too, so the toolchain and dylint allowances did
reach the Windows lane. What nothing bounded was everything else.
Neither profile set a `global-timeout`, so nothing bounded the whole
run. And the Windows lane declared no `timeout-minutes`, inheriting
GitHub's six-hour default, which left the outermost tier missing on the
one lane whose inner tiers were weakest.

The third tier of the canonical four, the shared coverage action's cargo
watchdog, does not exist here and its absence is asserted rather than
assumed: `coverage-main.yml` says why the action is not used, and a lane
that adopted it would inherit an undocumented 1,800 s default beneath a
45 m nextest budget. See "Test timeouts: four tiers, outermost last" in
``docs/developers-guide.md``, and the canonical wording in
`leynos/shared-actions`' `generate-coverage` README.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
from timeout_budgets import (
    Profile,
    NEXTEST_CONFIG,
    CEILING_MARGIN_SECONDS,
    OUTSIDE_SUITE_ALLOWANCE_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    bounds_a_single_test,
    global_timeout,
    largest_period,
    profiles,
    required_ceiling,
    termination_allowance,
)

#: The commands that run the workspace suite under nextest. A step running
#: one of these is bound by both nextest tiers; a step running anything
#: else is not, which is why the list is exact rather than a substring
#: search for "test".
from suite_lanes import (
    _watchdog_offences,
    SUITE_COMMANDS,
    SuiteLane,
    _declared_jobs,
    _disguised_suite_lines,
    _lanes_in_job,
)


@pytest.fixture(scope="module")
def nextest_profiles() -> dict[str, Profile]:
    """Return each nextest profile's text.

    Returns
    -------
    dict[str, str]
        Profile name to its section and overrides.
    """
    return profiles(NEXTEST_CONFIG.read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def suite_lanes() -> tuple[SuiteLane, ...]:
    """Return every step that runs the suite, with its job's ceiling.

    Every such step is included, not only those in jobs that declare a
    ceiling, so a job that never had one is visible as ``None`` rather
    than absent. An absent entry would let the Windows lane's missing
    ``timeout-minutes`` pass unremarked, which is how it was missing.

    Returns
    -------
    tuple[SuiteLane, ...]
        One entry per suite-running step.
    """
    return tuple(lane for job in _declared_jobs() for lane in _lanes_in_job(job))


def test_the_suite_runs_somewhere(suite_lanes: tuple[SuiteLane, ...]) -> None:
    """The contract needs a lane to assert against.

    A rename that stopped the detector matching would otherwise turn every
    assertion below into a vacuous pass over an empty list.
    """
    assert suite_lanes, (
        f"no workflow step runs one of {SUITE_COMMANDS}; either the suite "
        f"moved or this contract stopped recognizing it"
    )


@pytest.mark.parametrize("profile", ["default", "ci"], ids=str)
def test_every_profile_bounds_a_single_test(
    nextest_profiles: dict[str, Profile], profile: str
) -> None:
    """A test that hangs must be killed, not merely reported slow.

    Both profiles need this and only one had it. ``ci`` inherits the
    default profile's scalar keys but not its overrides, so the toolchain
    tests it includes and the default profile excludes ran with no
    allowance at all, on the lane whose job had no ceiling either.
    """
    parsed = nextest_profiles.get(profile)
    assert parsed is not None, f"nextest.toml must declare [profile.{profile}]"
    assert bounds_a_single_test(parsed), (
        f"[profile.{profile}] itself must set slow-timeout with "
        f"terminate-after. An override satisfies the profile as a whole while "
        f"leaving every test the override does not match with no bound at "
        f"all, which is the state this contract exists to detect"
    )


@pytest.mark.parametrize("profile", ["default", "ci"], ids=str)
def test_the_global_timeout_sits_above_the_largest_single_test(
    nextest_profiles: dict[str, Profile], profile: str
) -> None:
    """Tier two must not pre-empt tier one.

    A whole-run budget below the longest per-test allowance ends the run
    before the test that allowance exists for can finish, and the failure
    names the run rather than the test.
    """
    parsed = nextest_profiles[profile]
    whole_run = global_timeout(parsed)
    longest = largest_period(parsed)
    assert whole_run > longest, (
        f"[profile.{profile}]'s {whole_run:.0f}s global-timeout is not above "
        f"its {longest:.0f}s largest per-test slow-timeout; the run would end "
        f"before that test could use its budget"
    )


def test_every_suite_lane_declares_a_job_ceiling(
    suite_lanes: tuple[SuiteLane, ...],
) -> None:
    """The outermost tier is the one nobody notices is missing.

    A job with no ``timeout-minutes`` inherits GitHub's six-hour default,
    which is not a budget anyone chose. The Windows lane ran that way, and
    it is the lane with the fewest inner bounds.
    """
    missing = [str(lane) for lane in suite_lanes if lane.job_timeout is None]
    assert not missing, (
        f"these suite lanes declare no timeout-minutes and so inherit "
        f"GitHub's six-hour default: {missing}"
    )


def test_the_job_ceiling_covers_the_run_and_the_work_around_it(
    suite_lanes: tuple[SuiteLane, ...], nextest_profiles: dict[str, Profile]
) -> None:
    """Tier four must not pre-empt tier two.

    The two clocks do not start together. The job timer starts when the
    job starts, before the checkout, the toolchain setup and the build,
    and it is still running through whatever follows the suite. nextest's
    global timeout starts only once tests begin. A ceiling merely above
    the whole-run budget still cancels the job before nextest can report
    an overrun, and a cancellation discards the log that would have
    explained it.
    """
    for lane in suite_lanes:
        # `make coverage` re-enters `make test` without a profile, so it
        # runs under `default`; only an explicit `NEXTEST_PROFILE=ci` on
        # the command line selects the other one.
        profile = "ci" if "NEXTEST_PROFILE=ci" in lane.command else "default"
        parsed = nextest_profiles[profile]
        required = required_ceiling(parsed)
        assert lane.job_timeout is not None, str(lane)
        assert lane.job_timeout >= required, (
            f"{lane} has a job ceiling of {lane.job_timeout:.0f}s, below the "
            f"{required:.0f}s needed to cover [profile.{profile}]'s "
            f"{global_timeout(parsed):.0f}s whole-run budget, "
            f"{termination_allowance(parsed):.0f}s for nextest to terminate the "
            f"run, {OUTSIDE_SUITE_ALLOWANCE_SECONDS:.0f}s of build and "
            f"other work outside its window, and a "
            f"{CEILING_MARGIN_SECONDS:.0f}s margin above that sum; an overrun "
            f"would be cancelled rather than reported"
        )


def test_the_cargo_watchdog_tier_is_absent_rather_than_defaulted() -> None:
    """The third tier does not exist here, and must not appear unnoticed.

    The canonical section has four tiers because the shared coverage
    action wraps `cargo` in a wall-clock watchdog. This repository runs
    `cargo llvm-cov` itself and does not use that action, so the tier is
    absent by choice. A lane that adopted the action without setting
    `RUN_RUST_CARGO_WAIT_TIMEOUT` would inherit its undocumented 1,800 s
    default underneath a 45 m nextest budget, which is the inversion the
    canonical section exists to prevent, so both halves are asserted:
    the action is not used, and the variable is not set.
    """
    offenders = [
        offence for job in _declared_jobs() for offence in _watchdog_offences(job)
    ]
    assert not offenders, (
        f"the cargo watchdog tier is documented as absent here, so adopting "
        f"it needs the developers' guide updated in the same change: "
        f"{offenders}"
    )


#: The ceiling every suite-running lane must declare, in minutes, as
#: `docs/developers-guide.md` records it. Pinned as well as derived: the
#: derivation accepts any ceiling above its requirement, so a value
#: nobody chose passes it while drifting away from the guide.
REQUIRED_CEILING_MINUTES: typ.Final[int] = 80

#: The condition each suite lane legitimately carries, keyed by workflow
#: and job, as the step's ``if`` and its job's.
#:
#: A skipped step runs no suite, so none of the budgets above says
#: anything about it. `if: false` on either would leave a lane that
#: looks bounded and is not, and so would a plausible condition that
#: quietly excluded the event the lane exists for. `ci.yml`'s
#: `coverage-check` job legitimately runs on pull requests only, because
#: `coverage-main.yml` covers the trunk.
REQUIRED_CONDITIONS: typ.Final[dict[tuple[str, str], tuple[object, object]]] = {
    ("ci.yml", "coverage-check"): (None, "github.event_name == 'pull_request'"),
    ("ci.yml", "windows-compat"): (None, None),
    ("coverage-main.yml", "coverage-upload"): (None, None),
}


def test_every_suite_lane_carries_the_documented_ceiling(
    suite_lanes: tuple[SuiteLane, ...],
) -> None:
    """The value is the guide's, not merely one above the requirement.

    The derivation asserts the ordering holds, and it holds for a range
    of ceilings, so a lane drifting to a value nobody chose still passes
    it. This asserts the value the developers' guide states, which is
    that requirement plus the fifteen minutes of slack the guide asks
    for above it.
    """
    wrong = {
        str(lane): lane.job_timeout
        for lane in suite_lanes
        if lane.job_timeout != REQUIRED_CEILING_MINUTES * 60.0
    }
    assert not wrong, (
        f"these suite lanes do not carry the documented "
        f"{REQUIRED_CEILING_MINUTES}-minute ceiling: {wrong}; change the "
        f"developers' guide with them or change them back"
    )


def test_no_step_disguises_a_suite_command() -> None:
    """A suite command must be the step's command, plainly.

    Two shapes defeat a contract that only recognizes plain
    invocations, and they fail in opposite directions.

    `if false; then make test; fi` keeps the text and runs nothing. The
    lane then disappears from `suite_lanes` entirely, so its ceiling
    stops being checked and this contract passes while the lane it was
    protecting is unbounded. That is the loss the mutation record exists
    to catch, and it is silent.

    `make test || true` does run the suite but discards its verdict, so
    the lane's budgets are asserted while its result is thrown away.

    Neither is judged as an invocation. Both are reported here, because
    a contract that cannot tell what a line does should say so rather
    than guess.
    """
    disguised = [
        f"{job.workflow}:{job.name}: {line!r}"
        for job in _declared_jobs()
        for step in (job.body.get("steps") or [])
        if isinstance(step, dict)
        for line in _disguised_suite_lines(str(step.get("run", "")))
    ]
    assert not disguised, (
        f"these steps name a suite command without plainly running one, so "
        f"this contract cannot tell whether the lane runs the suite or "
        f"whether its failure would end the step: {disguised}"
    )


def test_the_required_ceiling_carries_all_four_terms() -> None:
    """Whole-run budget, termination, outside work, and the margin.

    Both lanes now sit above the requirement with the margin to spare,
    so dropping the margin from the derivation changes nothing the
    assertion over the workflows can see: the ceiling still clears the
    smaller number. Driving the derivation with a controlled profile is
    what makes the missing term visible.
    """
    config_text = (
        "[profile.example]\n"
        'slow-timeout = { period = "300s", terminate-after = 1, '
        'grace-period = "5s" }\n'
        'global-timeout = "45m"\n'
    )
    parsed = profiles(config_text)["example"]
    expected = (
        45 * 60.0
        + (5.0 + TERMINATION_SAFETY_MARGIN_SECONDS)
        + OUTSIDE_SUITE_ALLOWANCE_SECONDS
        + CEILING_MARGIN_SECONDS
    )
    assert required_ceiling(parsed) == pytest.approx(expected), (
        f"the requirement is the whole-run budget, the termination allowance, "
        f"the outside allowance and the margin, added; expected {expected}"
    )


def test_each_suite_lane_carries_the_condition_it_is_meant_to(
    suite_lanes: tuple[SuiteLane, ...],
) -> None:
    """A skipped step runs no suite, so no budget above bounds it.

    Every assertion above reads a lane's declared budgets and says
    nothing about whether the step runs. `if: false` on the step or on
    its job would leave a lane that looks bounded and is not, and this
    contract would certify it. So would a plausible condition that
    quietly excluded the event the lane exists for, which is why the
    conditions are pinned by value rather than checked for falsity:
    YAML parses `false` to a boolean, and enumerating falsy spellings
    would miss the plausible ones anyway.

    `coverage-check` legitimately runs on pull requests only, because
    `coverage-main.yml` covers the trunk, so that value is pinned rather
    than forbidden. The coordinates are compared both ways first, so a
    new lane with no entry fails rather than passing unexamined.

    Proved by mutation: `if: false` on the coverage step, the same on
    its job, `coverage-check` narrowed to a push-only condition, and a
    coordinate dropped from ``REQUIRED_CONDITIONS`` each fail this test.
    """
    found: dict[tuple[str, str], set[tuple[object, object]]] = {}
    for lane in suite_lanes:
        found.setdefault((lane.workflow, lane.job), set()).add(lane.condition)
    assert set(found) == set(REQUIRED_CONDITIONS), (
        f"the suite lanes are not the ones this contract pins: "
        f"unlisted {sorted(set(found) - set(REQUIRED_CONDITIONS))}, missing "
        f"{sorted(set(REQUIRED_CONDITIONS) - set(found))}; a lane with no "
        f"entry here is a lane whose condition nobody has judged"
    )
    wrong = {
        coordinate: (expected, found[coordinate])
        for coordinate, expected in REQUIRED_CONDITIONS.items()
        if found[coordinate] != {expected}
    }
    assert not wrong, (
        f"these suite lanes do not carry the conditions the developers' "
        f"guide records, as expected versus found: {wrong}; a lane that is "
        f"skipped runs no suite, so none of the budgets above bounds it"
    )
