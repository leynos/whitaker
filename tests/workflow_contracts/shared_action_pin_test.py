"""Hold every `setup-rust` call to one reviewed revision.

This repository briefly split its shared-actions pin: the developer-blocking
lanes moved to the revision that restores the caller's Actions cache-service
selection, while the tag-cutting lanes waited, on the reasoning that a green
pull request is not evidence that a release still builds.

That reasoning was wrong for this repository. Whitaker is a rolling release:
`rolling-release.yml` runs on every merge to `main`, so the release lanes were
already exercising the newer wiring on every merge, and the split was
protecting against a risk that had been taken dozens of times.

The split is collapsed. What remains is the rule it was built to carry: every
call is held to one constant by value, and a workflow that calls `setup-rust`
without being governed by that rule is rejected rather than pinned by nobody.
"""

from __future__ import annotations

import typing as typ
from typing import Final

import pytest

from ubicloud_workflow_support import SHARED_ACTIONS_REF, all_jobs

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from collections.abc import Iterator

SETUP_RUST_PREFIX: Final[str] = "leynos/shared-actions/.github/actions/setup-rust@"

#: Every workflow that calls `setup-rust`, listed rather than discovered so a
#: new caller has to be added deliberately. `test_every_setup_rust_call_is_in_a
#: _listed_workflow` is what makes the list load-bearing.
PINNED_WORKFLOWS: Final[frozenset[str]] = frozenset(
    {"ci.yml", "coverage-main.yml", "release.yml", "rolling-release.yml"}
)


def _steps(job: dict[str, object]) -> list[dict[str, object]]:
    """Return a job's steps, or none when it declares no step list."""
    # A job that calls another workflow declares `uses` at the job level and
    # has no steps, so it is skipped rather than treated as malformed. The
    # reusable-workflow pins are governed separately.
    steps = job.get("steps")
    if not isinstance(steps, list):
        return []
    return [step for step in steps if isinstance(step, dict)]


def _setup_rust_uses() -> Iterator[tuple[str, str, str]]:
    """Yield every `setup-rust` call as workflow, job, and pinned reference."""
    for workflow_name, job_name, job in all_jobs():
        for step in _steps(job):
            uses = step.get("uses")
            if isinstance(uses, str) and uses.startswith(SETUP_RUST_PREFIX):
                yield workflow_name, job_name, uses.removeprefix(SETUP_RUST_PREFIX)


def test_every_setup_rust_call_is_in_a_listed_workflow() -> None:
    """A caller outside the list would be pinned by nobody's rule."""
    strays = {
        workflow_name
        for workflow_name, _, _ in _setup_rust_uses()
        if workflow_name not in PINNED_WORKFLOWS
    }
    assert not strays, (
        f"{sorted(strays)} call setup-rust without appearing in "
        "PINNED_WORKFLOWS, so no rule governs their pin"
    )


def test_every_listed_workflow_calls_setup_rust() -> None:
    """A listed workflow with no calls makes the value assertion vacuous.

    Without this, every call could vanish from `release.yml` while the other
    three kept the suite green, which is the same hole one level up from the
    one the pin rule closes.
    """
    covered = {workflow_name for workflow_name, _, _ in _setup_rust_uses()}
    assert covered == PINNED_WORKFLOWS, (
        f"expected setup-rust calls in {sorted(PINNED_WORKFLOWS)}, found "
        f"{sorted(covered)}"
    )


@pytest.mark.parametrize("workflow", sorted(PINNED_WORKFLOWS))
def test_each_workflow_pins_the_reviewed_revision(workflow: str) -> None:
    """One revision, asserted by value, across every lane."""
    mismatched = [
        (workflow_name, job_name, reference)
        for workflow_name, job_name, reference in _setup_rust_uses()
        if workflow_name == workflow and reference != SHARED_ACTIONS_REF
    ]
    assert not mismatched, (
        f"{workflow} must pin {SHARED_ACTIONS_REF}; found {mismatched}"
    )
