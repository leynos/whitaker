"""Every gha lane keeps its sccache evidence and fails on a broken integration.

A green build says nothing about the compiler cache: sccache falls back to
local disk, or fails every store, and still compiles. So each lane on the
Actions backend uploads its statistics under `if: always()`, then runs
`scripts/check_sccache_health.py` against them, which fails the job on the
structural signatures and only warns on isolated errors. The upload comes
first so the evidence survives the failure it explains.

`health_violations` takes a job, so the synthetic cases below prove the rule
fails on a missing or misplaced step rather than trusting the repository's own
lanes to be the only evidence.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
from shell_commands import runs_unconditionally
from ubicloud_workflow_support import SUITE_JOBS, job_steps, load_job

#: The command each gha lane runs, as its step's whole script.
HEALTH_COMMAND: typ.Final[str] = (
    "python3 scripts/check_sccache_health.py --expect-location ghac"
)
RECORD_SCRIPT: typ.Final[str] = "scripts/record-sccache-effectiveness.sh"
STATISTICS_FILE: typ.Final[str] = "sccache-stats.json"
UPLOAD_ACTION: typ.Final[str] = "actions/upload-artifact"
#: The skip every sccache evidence step carries. `setup-rust` reports `fallback`
#: when sccache's server would not start and the job compiles uncached
#: (shared-actions #546); a dead server has no statistics, and asking for them
#: would start it again, so the steps that read them stand down rather than turn
#: a documented fail-open into a red job.
NOT_FALLBACK: typ.Final[str] = "steps.setup-rust.outputs.sccache-status!='fallback'"


def _condition(step: dict[str, typ.Any]) -> str:
    """Return a step's `if` with whitespace and expression braces removed."""
    text = str(step.get("if", "")).replace(" ", "")
    return text.removeprefix("${{").removesuffix("}}")


def _skips_a_fallback(step: dict[str, typ.Any]) -> bool:
    """Return whether a step's whole condition ends in the fallback skip."""
    return _condition(step).endswith(NOT_FALLBACK)


def _index(
    steps: list[dict[str, typ.Any]], predicate: typ.Callable[[dict[str, typ.Any]], bool]
) -> int | None:
    """Return the index of the first step satisfying a predicate, or `None`."""
    return next((i for i, step in enumerate(steps) if predicate(step)), None)


def _records(step: dict[str, typ.Any]) -> bool:
    """Return whether a step records the sccache statistics, past a fallback."""
    return RECORD_SCRIPT in str(step.get("run", "")) and _skips_a_fallback(step)


def _uploads_the_statistics(step: dict[str, typ.Any]) -> bool:
    """Return whether a step always uploads the statistics and fails without them."""
    inputs = step.get("with") if isinstance(step.get("with"), dict) else {}
    return (
        str(step.get("uses", "")).split("@", 1)[0] == UPLOAD_ACTION
        and str(inputs.get("path", "")).strip() == STATISTICS_FILE
        and inputs.get("if-no-files-found") == "error"
        and _condition(step) == f"always()&&{NOT_FALLBACK}"
    )


def _checks_health(step: dict[str, typ.Any]) -> bool:
    """Return whether a step runs the health check, and only it, past a fallback."""
    return _condition(step) == NOT_FALLBACK and runs_unconditionally(
        str(step.get("run", "")), HEALTH_COMMAND
    )


def health_violations(job: dict[str, typ.Any]) -> list[str]:
    """Return what a gha lane lacks for its sccache evidence and health check.

    >>> health_violations({"steps": []})[0]
    'no step records the sccache statistics'
    """
    steps = job_steps(job)
    positions = {
        "record": _index(steps, _records),
        "upload": _index(steps, _uploads_the_statistics),
        "health": _index(steps, _checks_health),
    }
    missing = {
        "record": "no step records the sccache statistics",
        "upload": f"no step uploads {STATISTICS_FILE} under if: always() with if-no-files-found: error and skipping a fallback",
        "health": f"no step of its own runs `{HEALTH_COMMAND}` skipping only a fallback",
    }
    violations = [missing[name] for name, index in positions.items() if index is None]
    if violations:
        return violations
    if not positions["record"] < positions["upload"] < positions["health"]:
        violations.append(
            "the order must be record, then upload, then the health check"
        )
    return violations


def _gha_lanes() -> list[str]:
    """Return the suite jobs, whose sccache runs on Ubicloud's cache proxy.

    Every Ubicloud job reaches the proxy now, through `setup-rust`, but only
    the jobs that run this repository's gates record the evidence and check
    its health; the rolling-release build jobs report their statistics and
    stop there.
    """
    return sorted(SUITE_JOBS)


def test_there_are_gha_lanes_to_hold() -> None:
    """The presence half: a rule over no lanes passes by deleting them."""
    assert {"coverage-check", "linux-full", "coverage-upload"} <= set(_gha_lanes())


@pytest.mark.parametrize("job_name", _gha_lanes())
def test_every_gha_lane_keeps_its_evidence_and_checks_its_health(job_name: str) -> None:
    """Each lane records, uploads, then checks, in that order."""
    violations = health_violations(load_job(job_name))
    assert not violations, f"{job_name}: {violations}"


_GUARD: typ.Final = "steps.setup-rust.outputs.sccache-status != 'fallback'"
_RECORD: typ.Final = {
    "if": f"always() && {_GUARD}",
    "run": f"bash {RECORD_SCRIPT}",
}
_UPLOAD: typ.Final = {
    "if": f"always() && {_GUARD}",
    "uses": f"{UPLOAD_ACTION}@abc",
    "with": {"name": "s", "path": STATISTICS_FILE, "if-no-files-found": "error"},
}
_HEALTH: typ.Final = {"if": _GUARD, "run": f"{HEALTH_COMMAND} {STATISTICS_FILE}"}


@pytest.mark.parametrize(
    ("steps", "expected"),
    [
        pytest.param([_RECORD, _UPLOAD], "runs `python3", id="no-health-check"),
        pytest.param([_RECORD, _HEALTH], "uploads", id="no-upload"),
        pytest.param(
            [_RECORD, _UPLOAD | {"if": "success()"}, _HEALTH],
            "uploads",
            id="an-upload-lost-on-failure",
        ),
        pytest.param(
            [
                _RECORD,
                _UPLOAD | {"with": {"name": "s", "path": STATISTICS_FILE}},
                _HEALTH,
            ],
            "uploads",
            id="an-upload-that-tolerates-a-missing-file",
        ),
        pytest.param(
            [_RECORD, _UPLOAD, _HEALTH | {"if": "false"}],
            "runs `python3",
            id="a-conditional-check",
        ),
        pytest.param(
            [_RECORD, _UPLOAD, {"run": _HEALTH["run"]}],
            "runs `python3",
            id="a-check-that-ignores-a-fallback",
        ),
        pytest.param(
            [_RECORD | {"if": "always()"}, _UPLOAD, _HEALTH],
            "records",
            id="a-record-that-ignores-a-fallback",
        ),
        pytest.param(
            [_RECORD, _UPLOAD | {"if": "always()"}, _HEALTH],
            "uploads",
            id="an-upload-that-ignores-a-fallback",
        ),
        pytest.param(
            [_RECORD, _UPLOAD, {"run": f"true || {HEALTH_COMMAND}"}],
            "runs `python3",
            id="a-check-that-need-not-run",
        ),
        pytest.param(
            [_RECORD, _HEALTH, _UPLOAD], "order", id="checked-before-uploaded"
        ),
        pytest.param([_UPLOAD, _HEALTH], "records", id="nothing-recorded"),
    ],
)
def test_a_lane_missing_a_piece_is_refused(
    steps: list[dict[str, typ.Any]], expected: str
) -> None:
    """Each piece can be removed or misplaced on its own, and each is caught."""
    violations = health_violations({"steps": steps})
    assert any(expected in violation for violation in violations), violations


def test_the_complete_shape_is_accepted() -> None:
    """The narrow half: the shape the lanes use passes."""
    assert health_violations({"steps": [_RECORD, _UPLOAD, _HEALTH]}) == []
