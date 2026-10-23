"""Contracts for the disk-freeing step on the Linux jobs that run the suite.

The Ubicloud image preinstalls the Android SDK and .NET, 16.9 GB on a 73 GB
volume that none of these jobs uses. `coverage-check` and `linux-full` in
`ci.yml`, and `coverage-upload` in `coverage-main.yml`, free that space with
one step. The step measures the headroom, removes the two trees and measures
again, so the saving is visible in the log. It runs after the checkout and
before the first cache restore or compiler setup, so the space is free before
the caches and the compiler grow the disk.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest

from ubicloud_workflow_support import all_jobs

FREE_STEP = "Free preinstalled toolchains the job does not use"
MEASURE = "df -m /"
REMOVE = "sudo rm -rf /usr/local/lib/android /usr/share/dotnet"
CHECKOUT_PREFIX = "actions/checkout@"
CACHE_PREFIXES = ("actions/cache/restore@", "actions/cache@")
#: The jobs that declare the step, as (workflow, job) pairs.
RECLAIMING_JOBS = frozenset(
    {
        ("ci.yml", "coverage-check"),
        ("ci.yml", "linux-full"),
        ("coverage-main.yml", "coverage-upload"),
    }
)

Job = tuple[str, str, dict[str, typ.Any]]


def _reclaiming_jobs() -> list[Job]:
    """Return every job that declares the disk-freeing step."""
    return [j for j in all_jobs() if _free_steps(j[2])]


def _ids(jobs: list[Job]) -> list[str]:
    """Return readable parameter identifiers for a job list."""
    return [f"{w}:{n}" for w, n, _ in jobs]


def _steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return a job's step mappings."""
    return [s for s in job.get("steps", []) if isinstance(s, dict)]


def _uses(step: dict[str, typ.Any]) -> str:
    """Return a step's `uses` reference, or an empty string."""
    return str(step.get("uses", ""))


def _free_steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return a job's steps carrying the disk-freeing step's name."""
    return [s for s in _steps(job) if s.get("name") == FREE_STEP]


def _free_step(job: dict[str, typ.Any]) -> dict[str, typ.Any]:
    """Return a job's disk-freeing step, which must be the only one."""
    steps = _free_steps(job)
    assert len(steps) == 1, "a job must declare the disk-freeing step once"
    return steps[0]


def _starts_cache_or_setup(step: dict[str, typ.Any]) -> bool:
    """Return whether a step restores a cache or sets up the compiler."""
    uses = _uses(step)
    return uses.startswith(CACHE_PREFIXES) or "setup-rust" in uses


def _index(
    steps: list[dict[str, typ.Any]],
    matches: typ.Callable[[dict[str, typ.Any]], bool],
) -> int:
    """Return the position of the first step a predicate matches."""
    hits = [i for i, s in enumerate(steps) if matches(s)]
    assert hits, "no step matches the contract's predicate"
    return hits[0]


def test_the_selector_matches_the_three_jobs() -> None:
    """Guard against a selector that silently matches nothing."""
    assert {(w, n) for w, n, _ in _reclaiming_jobs()} == RECLAIMING_JOBS


@pytest.mark.parametrize("job", _reclaiming_jobs(), ids=_ids(_reclaiming_jobs()))
def test_the_job_declares_one_free_step(job: Job) -> None:
    """Free the disk once, so no job carries a second copy of the step."""
    assert len(_free_steps(job[2])) == 1, f"{job[1]} must free the disk once"


@pytest.mark.parametrize("job", _reclaiming_jobs(), ids=_ids(_reclaiming_jobs()))
def test_the_free_step_is_linux_only_and_bash(job: Job) -> None:
    """Guard the step to Linux runners and run its script under bash."""
    step = _free_step(job[2])
    assert step.get("if") == "runner.os == 'Linux'"
    assert step.get("shell") == "bash"


@pytest.mark.parametrize("job", _reclaiming_jobs(), ids=_ids(_reclaiming_jobs()))
def test_the_free_step_measures_removes_and_measures_again(job: Job) -> None:
    """Record headroom before and after the removal, in that order."""
    run = str(_free_step(job[2]).get("run", ""))
    assert run.splitlines()[0] == "set -euo pipefail"
    first = run.index(MEASURE)
    removal = run.index(REMOVE)
    last = run.rindex(MEASURE)
    assert first < removal < last, f"{job[1]} must measure, remove, then measure"


@pytest.mark.parametrize("job", _reclaiming_jobs(), ids=_ids(_reclaiming_jobs()))
def test_the_free_step_precedes_every_cache_and_compiler_setup(job: Job) -> None:
    """Free the disk after the checkout and before any cache or compiler setup."""
    steps = _steps(job[2])
    checkout_at = _index(steps, lambda s: _uses(s).startswith(CHECKOUT_PREFIX))
    free_at = _index(steps, lambda s: s.get("name") == FREE_STEP)
    cache_at = _index(steps, _starts_cache_or_setup)
    assert checkout_at < free_at < cache_at, (
        f"{job[1]} must free the disk after the checkout and before the first "
        "cache restore or compiler setup"
    )
