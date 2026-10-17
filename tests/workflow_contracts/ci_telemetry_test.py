"""Contracts for the CI telemetry that every `ci.yml` job carries.

The central CI observability collector reads one `DF12_CI_TELEMETRY` record
from each job in a listed workflow. `ci.yml` is listed, so each of its jobs
starts `ci-telemetry-start` right after the checkout and ends its work with
`ci-telemetry-finish` under `if: always()`: a job killed by its shape is the one
whose record matters most. A job without the pair reads as a missing record to
the collector, which is the fault the coverage numbers count.

`ci.yml` has no roll-up job, so there are no exemptions here. The finish step
passes `setup-rust`'s `sccache-status`, and reads the `sccache-stats.json` that
`scripts/record-sccache-effectiveness.sh` writes, which is the action's default
file, so the record carries the hit ratio without a second statistics call.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest

from ubicloud_workflow_support import all_jobs

PIN = "81374b6f183e59f0570ad36217ded90d3240fda6"
ACTIONS = "leynos/shared-actions/.github/actions"
START = f"{ACTIONS}/ci-telemetry-start@{PIN}"
FINISH = f"{ACTIONS}/ci-telemetry-finish@{PIN}"
STATUS = "${{ steps.setup-rust.outputs.sccache-status }}"
#: What may follow the report: the disk record and the cache saves, which are
#: housekeeping after the measured work.
AFTER_REPORT_PREFIXES = ("actions/cache/save@",)
AFTER_REPORT_NAMES = frozenset({"Record disk headroom before the saves"})

Job = tuple[str, str, dict[str, typ.Any]]


def _ci_jobs() -> list[Job]:
    """Return the jobs of `ci.yml`."""
    return [j for j in all_jobs() if j[0] == "ci.yml"]


def _ids(jobs: list[Job]) -> list[str]:
    """Return readable parameter identifiers for a job list."""
    return [f"{w}:{n}" for w, n, _ in jobs]


def _steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return a job's step mappings."""
    return [s for s in job.get("steps", []) if isinstance(s, dict)]


def _uses(step: dict[str, typ.Any]) -> str:
    """Return a step's `uses` reference, or an empty string."""
    return str(step.get("uses", ""))


def test_the_selector_matches_the_three_jobs() -> None:
    """Guard against a selector that silently matches nothing."""
    assert {n for _, n, _ in _ci_jobs()} == {
        "coverage-check",
        "linux-full",
        "windows-compat",
    }


@pytest.mark.parametrize("job", _ci_jobs(), ids=_ids(_ci_jobs()))
def test_the_start_follows_the_checkout_directly(job: Job) -> None:
    """Start telemetry right after the checkout, before any other work."""
    steps = _steps(job[2])
    assert _uses(steps[0]).startswith("actions/checkout@")
    assert _uses(steps[1]) == START, f"{job[1]} must start telemetry second"
    assert sum(_uses(s) == START for s in steps) == 1


@pytest.mark.parametrize("job", _ci_jobs(), ids=_ids(_ci_jobs()))
def test_the_report_runs_always_after_the_work(job: Job) -> None:
    """Report once, under `if: always()`, with only housekeeping after it."""
    steps = _steps(job[2])
    at = [i for i, s in enumerate(steps) if _uses(s) == FINISH]
    assert len(at) == 1, f"{job[1]} must report telemetry exactly once"
    report = steps[at[0]]
    assert report.get("if") == "always()"
    for step in steps[at[0] + 1 :]:
        assert (
            _uses(step).startswith(AFTER_REPORT_PREFIXES)
            or step.get("name") in AFTER_REPORT_NAMES
        ), f"{job[1]}: {step.get('name')!r} follows the report but is work"


@pytest.mark.parametrize("job", _ci_jobs(), ids=_ids(_ci_jobs()))
def test_the_report_carries_the_sccache_status(job: Job) -> None:
    """The record learns setup-rust's status, so a fallback never reads healthy."""
    steps = _steps(job[2])
    assert any(s.get("id") == "setup-rust" for s in steps)
    report = next(s for s in steps if _uses(s) == FINISH)
    assert (report.get("with") or {}).get("sccache-status") == STATUS


@pytest.mark.parametrize("job", _ci_jobs(), ids=_ids(_ci_jobs()))
def test_the_report_follows_the_statistics_it_reads(job: Job) -> None:
    """Finish reads `sccache-stats.json`, so the recorder must run before it."""
    steps = _steps(job[2])
    names = [str(s.get("name", "")) for s in steps]
    report_at = next(i for i, s in enumerate(steps) if _uses(s) == FINISH)
    assert names.index("Record sccache effectiveness") < report_at
