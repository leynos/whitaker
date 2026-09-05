"""Hold the two shared-action pins apart, by value and by lane.

This repository stopped pinning one shared-actions revision everywhere. The
developer-blocking lanes moved to the revision that restores the caller's
Actions cache-service selection and starts the `sccache` server after those
exports; the tag-cutting lanes did not, because a green pull request is not
evidence that a release still builds.

A split like that decays quietly. Someone repins one file, or bumps the
constant without moving the lanes, and nothing goes red until a release fails
months later. These tests assert the pin each lane actually declares against
the constant that documents why, so neither half can drift alone.
"""

from __future__ import annotations

import typing as typ

import pytest

from ubicloud_workflow_support import (
    CI_LANE_SHARED_ACTIONS_REF,
    RELEASE_LANE_SHARED_ACTIONS_REF,
    all_jobs,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from collections.abc import Iterator

SETUP_RUST_PREFIX = "leynos/shared-actions/.github/actions/setup-rust@"

#: The lanes a pull request blocks on, which take the newer revision.
CI_LANE_WORKFLOWS = frozenset({"ci.yml", "coverage-main.yml"})

#: The lanes that cut tags, which stay behind until a tag proves the wiring.
RELEASE_LANE_WORKFLOWS = frozenset({"release.yml", "rolling-release.yml"})


def _steps(job: dict[str, object]) -> list[dict[str, object]]:
    """Return a job's steps, or none for a job that calls another workflow.

    A caller job declares `uses` at the job level and has no step list, so it
    is skipped rather than treated as malformed.
    """
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


def _uses_for(workflows: frozenset[str]) -> list[tuple[str, str, str]]:
    """Return the `setup-rust` calls declared by one lane group."""
    return [entry for entry in _setup_rust_uses() if entry[0] in workflows]


def test_every_setup_rust_call_belongs_to_a_known_lane() -> None:
    """A workflow outside both groups would be pinned by nobody's rule."""
    strays = {
        workflow_name
        for workflow_name, _, _ in _setup_rust_uses()
        if workflow_name not in CI_LANE_WORKFLOWS | RELEASE_LANE_WORKFLOWS
    }
    assert not strays, (
        f"{sorted(strays)} call setup-rust without belonging to either lane "
        "group; add them to a group so their pin is governed"
    )


@pytest.mark.parametrize(
    ("workflows", "expected", "reason"),
    [
        pytest.param(
            CI_LANE_WORKFLOWS,
            CI_LANE_SHARED_ACTIONS_REF,
            "the developer-blocking lanes take the newer shared-actions pin",
            id="ci-lanes",
        ),
        pytest.param(
            RELEASE_LANE_WORKFLOWS,
            RELEASE_LANE_SHARED_ACTIONS_REF,
            "the tag-cutting lanes hold their pin until a tag proves the wiring",
            id="release-lanes",
        ),
    ],
)
def test_each_lane_group_pins_its_own_revision(
    workflows: frozenset[str], expected: str, reason: str
) -> None:
    """Every call in a group must name that group's revision, not the other's."""
    calls = _uses_for(workflows)
    assert calls, f"no setup-rust call found in {sorted(workflows)}"
    mismatched = [
        (workflow_name, job_name, reference)
        for workflow_name, job_name, reference in calls
        if reference != expected
    ]
    assert not mismatched, f"{reason}; found {mismatched}"


def test_the_two_pins_are_different_revisions() -> None:
    """The split is only meaningful while the two constants disagree.

    When a release tag has proven the newer wiring the lanes converge, and this
    test is the reminder to collapse the constants rather than leave a split
    that no longer describes anything.
    """
    assert CI_LANE_SHARED_ACTIONS_REF != RELEASE_LANE_SHARED_ACTIONS_REF, (
        "the lane pins have converged; collapse them back to one constant"
    )
