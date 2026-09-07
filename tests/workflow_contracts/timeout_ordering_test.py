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

import re
import tomllib
import typing as typ

import pytest
import yaml
from ubicloud_workflow_support import REPOSITORY_ROOT, WORKFLOWS_DIRECTORY

#: The commands that run the workspace suite under nextest. A step running
#: one of these is bound by both nextest tiers; a step running anything
#: else is not, which is why the list is exact rather than a substring
#: search for "test".
SUITE_COMMANDS: typ.Final[tuple[str, ...]] = ("make test", "make coverage")

#: Commands that contain a suite command as a prefix but run something
#: else entirely. `make test-doc` is doctests, outside nextest; the other
#: two are checkers that happen to be named for what they check.
NOT_SUITE_COMMANDS: typ.Final[tuple[str, ...]] = (
    "make test-doc",
    "make test-glibc-baseline",
    "make test-workflow-contracts",
    "make test-markdown-format",
)

#: The environment variable the shared coverage action reads for its
#: cargo watchdog. Asserted absent: this repository does not use that
#: action, and a lane that adopted it would inherit its 1,800 s default.
WATCHDOG_VARIABLE: typ.Final[str] = "RUN_RUST_CARGO_WAIT_TIMEOUT"
COVERAGE_ACTION: typ.Final[str] = "shared-actions/.github/actions/generate-coverage"

#: Build time inside `cargo` before nextest starts its own clock, plus the
#: steps either side of the suite within the same job. The job timer covers
#: both; the global timeout covers neither. Taken from the worst of 38
#: successful `coverage-main.yml` runs and 8 of `ci.yml`: the coverage step
#: reached 614 s on run 33824606032 in a job of 785 s, so 171 s outside it,
#: and the Windows suite step reached 845 s on run 34070807851 in a job of
#: 1,099 s, so 254 s outside it. Fifteen minutes covers the worse of those
#: with room for a cold build, which none of those runs was.
OUTSIDE_SUITE_ALLOWANCE_SECONDS: typ.Final[float] = 15 * 60.0

#: Floor for the termination allowance, used when a profile sets no grace
#: period. Generous against nextest's ten-second default and far too small
#: to hide a real overrun.
MINIMUM_TERMINATION_ALLOWANCE_SECONDS: typ.Final[float] = 60.0

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


def _root_section(block: str) -> str:
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

    The second is a safety margin against a grace period nobody has
    read. Taking the larger of the two, as an earlier version did, hid
    which was which: a configuration with a ninety-second grace period
    and one with none produced the same answer for different reasons.

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
    largest = max((seconds(period) for period in periods), default=0.0)
    return max(largest, MINIMUM_TERMINATION_ALLOWANCE_SECONDS)


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


class SuiteLane(typ.NamedTuple):
    """One step that runs the suite, with the job budget enclosing it.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    job : str
        The job the step belongs to.
    step : str
        The step's declared name.
    command : str
        The whole command line it runs, not the matched constant, so the
        profile can be read from it.
    job_timeout : float or None
        The enclosing job's ``timeout-minutes`` in seconds, or None when
        the job declares none and so inherits GitHub's six-hour default.
    """

    workflow: str
    job: str
    step: str
    command: str
    job_timeout: float | None

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job:step`` for this lane.
        """
        return f"{self.workflow}:{self.job}:{self.step!r}"


def _suite_command(run: str) -> str | None:
    """Return the suite command a step runs, or None.

    ``make test-doc`` and the checkers named for what they check all begin
    with a suite command's text. Matching by prefix would bind them to
    budgets they do not run under, and would let a genuine suite step
    escape by being renamed.

    Parameters
    ----------
    run : str
        A step's ``run`` script.

    Returns
    -------
    str or None
        The whole command line, or None when the step runs no suite
        command. The line rather than the matched constant, because the
        profile the lane runs under is an argument on it.
    """
    candidates = (line.strip() for line in run.splitlines())
    return next((line for line in candidates if _is_suite_line(line)), None)


def _is_suite_line(line: str) -> bool:
    """Return whether one stripped line invokes the suite.

    ``make test-doc`` and the checkers named for what they check all
    begin with a suite command's text, so they are excluded first and by
    exact prefix rather than by substring.

    Parameters
    ----------
    line : str
        One stripped line of a step's script.

    Returns
    -------
    bool
        True when the line runs a suite command.
    """
    if any(line.startswith(other) for other in NOT_SUITE_COMMANDS):
        return False
    return any(
        line == command or line.startswith(f"{command} ") for command in SUITE_COMMANDS
    )


def _workflow_documents() -> dict[str, dict[str, typ.Any]]:
    """Return every workflow document, keyed by file name.

    Both extensions are read. A lane in the other one would otherwise
    escape every assertion below without failing anything.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        File name to parsed document.
    """
    documents: dict[str, dict[str, typ.Any]] = {}
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
            if isinstance(parsed, dict):
                documents[path.name] = parsed
    return documents


@pytest.fixture(scope="module")
def nextest_profiles() -> dict[str, str]:
    """Return each nextest profile's text.

    Returns
    -------
    dict[str, str]
        Profile name to its section and overrides.
    """
    return profile_blocks(NEXTEST_CONFIG.read_text(encoding="utf-8"))


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


class _Job(typ.NamedTuple):
    """One job of one workflow, with the file it came from.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    name : str
        The job's identifier.
    body : dict[str, typ.Any]
        The job's parsed mapping.
    """

    workflow: str
    name: str
    body: dict[str, typ.Any]


def _declared_jobs() -> tuple[_Job, ...]:
    """Return every job in every workflow, with its file.

    Flattening the two levels here is what keeps the callers below to
    one loop each: a job's identity travels with it rather than being
    reconstructed from an enclosing scope.

    Returns
    -------
    tuple[_Job, ...]
        Every declared job.
    """
    return tuple(
        _Job(workflow=name, name=str(job_name), body=job)
        for name, document in _workflow_documents().items()
        for job_name, job in (document.get("jobs") or {}).items()
        if isinstance(job, dict)
    )


def _job_ceiling(job: _Job) -> float | None:
    """Return a job's ``timeout-minutes`` in seconds, or None.

    Parameters
    ----------
    job : _Job
        The job to read.

    Returns
    -------
    float or None
        The ceiling in seconds, or None when the job declares none and
        so inherits GitHub's six-hour default.
    """
    raw = job.body.get("timeout-minutes")
    return None if raw is None else float(raw) * 60.0


def _lanes_in_job(job: _Job) -> tuple[SuiteLane, ...]:
    """Return the suite-running lanes one job declares.

    Parameters
    ----------
    job : _Job
        The job to read.

    Returns
    -------
    tuple[SuiteLane, ...]
        One entry per suite-running step in that job.
    """
    ceiling = _job_ceiling(job)
    steps = (step for step in job.body.get("steps") or [] if isinstance(step, dict))
    return tuple(
        SuiteLane(
            workflow=job.workflow,
            job=job.name,
            step=str(step.get("name", "")) or job.name,
            command=command,
            job_timeout=ceiling,
        )
        for step in steps
        if (command := _suite_command(str(step.get("run", "")))) is not None
    )


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
    nextest_profiles: dict[str, str], profile: str
) -> None:
    """A test that hangs must be killed, not merely reported slow.

    Both profiles need this and only one had it. ``ci`` inherits the
    default profile's scalar keys but not its overrides, so the toolchain
    tests it includes and the default profile excludes ran with no
    allowance at all, on the lane whose job had no ceiling either.
    """
    block = nextest_profiles.get(profile)
    assert block is not None, f"nextest.toml must declare [profile.{profile}]"
    root = _root_section(block)
    assert "terminate-after" in root, (
        f"[profile.{profile}] itself must set slow-timeout with "
        f"terminate-after. An override satisfies the profile as a whole while "
        f"leaving every test the override does not match with no bound at "
        f"all, which is the state this contract exists to detect"
    )


@pytest.mark.parametrize("profile", ["default", "ci"], ids=str)
def test_the_global_timeout_sits_above_the_largest_single_test(
    nextest_profiles: dict[str, str], profile: str
) -> None:
    """Tier two must not pre-empt tier one.

    A whole-run budget below the longest per-test allowance ends the run
    before the test that allowance exists for can finish, and the failure
    names the run rather than the test.
    """
    block = nextest_profiles[profile]
    whole_run = global_timeout(block)
    longest = largest_period(block)
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
    suite_lanes: tuple[SuiteLane, ...], nextest_profiles: dict[str, str]
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
        block = nextest_profiles[profile]
        required = (
            global_timeout(block)
            + termination_allowance(block)
            + OUTSIDE_SUITE_ALLOWANCE_SECONDS
        )
        assert lane.job_timeout is not None, str(lane)
        assert lane.job_timeout >= required, (
            f"{lane} has a job ceiling of {lane.job_timeout:.0f}s, below the "
            f"{required:.0f}s needed to cover [profile.{profile}]'s "
            f"{global_timeout(block):.0f}s whole-run budget, "
            f"{termination_allowance(block):.0f}s for nextest to terminate the "
            f"run, and {OUTSIDE_SUITE_ALLOWANCE_SECONDS:.0f}s of build and "
            f"other work outside its window; an overrun would be cancelled "
            f"rather than reported"
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


def _watchdog_offences(job: _Job) -> tuple[str, ...]:
    """Return the ways one job would reintroduce the watchdog tier.

    Both halves are looked for: the action itself, and the variable that
    configures it. The variable is read at all three scopes GitHub
    resolves, because a value at workflow or job level is inherited by
    every step and a check reading only the step's own environment would
    miss it entirely, which is the opposite of what this asserts.

    Parameters
    ----------
    job : _Job
        The job to read.

    Returns
    -------
    tuple[str, ...]
        One entry per offence, naming the job and what it did.
    """
    where = f"{job.workflow}:{job.name}"
    offences: list[str] = []
    if _sets_watchdog(_workflow_documents()[job.workflow]):
        offences.append(f"{job.workflow} sets {WATCHDOG_VARIABLE} at workflow level")
    if _sets_watchdog(job.body):
        offences.append(f"{where} sets {WATCHDOG_VARIABLE} at job level")
    for step in job.body.get("steps") or []:
        if not isinstance(step, dict):
            continue
        if COVERAGE_ACTION in str(step.get("uses", "")):
            offences.append(f"{where} uses {COVERAGE_ACTION}")
        if _sets_watchdog(step):
            offences.append(f"{where} sets {WATCHDOG_VARIABLE} on a step")
    return tuple(offences)


def _sets_watchdog(owner: dict[str, typ.Any]) -> bool:
    """Return whether one scope sets the watchdog variable.

    Parameters
    ----------
    owner : dict[str, typ.Any]
        A workflow, job or step mapping.

    Returns
    -------
    bool
        True when its ``env`` names the variable.
    """
    environment = owner.get("env")
    return isinstance(environment, dict) and WATCHDOG_VARIABLE in environment


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

#: The ceiling every suite-running lane must declare, in minutes, as
#: `docs/developers-guide.md` records it. Pinned as well as derived: the
#: derivation accepts any ceiling above its requirement, so a value
#: nobody chose passes it while drifting away from the guide.
REQUIRED_CEILING_MINUTES: typ.Final[int] = 70


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
    allowance and once to serialise three scenarios that contend on
    shared rustup state, and the second carries no timeout to assert.
    """
    overrides = parsed_nextest["profile"][profile].get("overrides") or []
    matching = [
        override
        for override in overrides
        if needle in str(override.get("filter", ""))
        and "slow-timeout" in override
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
