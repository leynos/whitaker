"""Contract for the timers that can end a test run.

Four budgets can end a run, each set somewhere different, and they only
work if each sits above the one inside it. Three of the four were missing
or partial here.

The `ci` profile, which the Windows lane runs, had no per-test allowance
at all: `[[profile.default.overrides]]` belongs to the default profile
and another profile does not inherit it, so the toolchain-installing
tests that profile includes, and that the default profile excludes, ran
unbounded. Neither profile set a `global-timeout`, so nothing bounded the
whole run. And the Windows lane declared no `timeout-minutes`, inheriting
GitHub's six-hour default, which left the outermost tier missing on the
one lane that had no inner ones either.

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
    """Return the time nextest may take to stop the run, in seconds.

    Hitting the global timeout starts nextest's ordinary termination
    procedure rather than stopping the run: on Unix it signals the process
    group and waits ``slow-timeout.grace-period`` before killing it; on
    Windows termination is immediate and the grace period is ignored for
    timeouts. Read from the configuration rather than fixed, because a
    profile that raised its grace period past a hard-coded allowance would
    drift out of the requirement this contract exists to hold.

    Parameters
    ----------
    block : str
        One profile's text.

    Returns
    -------
    float
        The largest configured grace period, or the floor when that is
        smaller or absent.
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
    for line in run.splitlines():
        stripped = line.strip()
        if any(stripped.startswith(other) for other in NOT_SUITE_COMMANDS):
            continue
        for command in SUITE_COMMANDS:
            if stripped == command or stripped.startswith(f"{command} "):
                return stripped
    return None


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
    lanes: list[SuiteLane] = []
    for name, document in _workflow_documents().items():
        for job_name, job in (document.get("jobs") or {}).items():
            if not isinstance(job, dict):
                continue
            raw_timeout = job.get("timeout-minutes")
            timeout = None if raw_timeout is None else float(raw_timeout) * 60.0
            for step in job.get("steps") or []:
                if not isinstance(step, dict):
                    continue
                command = _suite_command(str(step.get("run", "")))
                if command is None:
                    continue
                lanes.append(
                    SuiteLane(
                        workflow=name,
                        job=str(job_name),
                        step=str(step.get("name", "")) or str(job_name),
                        command=command,
                        job_timeout=timeout,
                    )
                )
    return tuple(lanes)


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
    assert "terminate-after" in block, (
        f"[profile.{profile}] must set slow-timeout with terminate-after, or a "
        f"hung test is reported slow for ever and only the job timer ends it"
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
    offenders: list[str] = []
    for name, document in _workflow_documents().items():
        for job_name, job in (document.get("jobs") or {}).items():
            if not isinstance(job, dict):
                continue
            for step in job.get("steps") or []:
                if not isinstance(step, dict):
                    continue
                if COVERAGE_ACTION in str(step.get("uses", "")):
                    offenders.append(f"{name}:{job_name} uses {COVERAGE_ACTION}")
                if WATCHDOG_VARIABLE in (step.get("env") or {}):
                    offenders.append(f"{name}:{job_name} sets {WATCHDOG_VARIABLE}")
    assert not offenders, (
        f"the cargo watchdog tier is documented as absent here, so adopting "
        f"it needs the developers' guide updated in the same change: "
        f"{offenders}"
    )
