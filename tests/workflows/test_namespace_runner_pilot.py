"""Validate the deliberately narrow Namespace runner pilot placement.

The pilot moves only the repository-owned pull-request Linux validation jobs
whose prior UbiCloud resource contract matches the deployed Namespace profile.
Release, scheduled, Windows, and reusable-workflow jobs retain their existing
runner contracts.

Run this contract directly with:

```sh
PYTHONPATH=. uv run pytest tests/workflows/test_namespace_runner_pilot.py
```
"""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

import pytest
from ruamel.yaml import YAML

WORKFLOWS_DIRECTORY = Path(__file__).resolve().parents[2] / ".github/workflows"
WORKFLOW_FILE_GLOBS = ("*.yaml", "*.yml")
NAMESPACE_DEFAULT_PROFILE = "namespace-profile-default"

MIGRATED_PULL_REQUEST_JOBS = {
    "ci.yml": {
        "coverage-check": NAMESPACE_DEFAULT_PROFILE,
        "linux-full": NAMESPACE_DEFAULT_PROFILE,
    },
}

RETAINED_RUNNERS = {
    "ci.yml": {"windows-compat": "windows-latest"},
    "coverage-main.yml": {"coverage-upload": "ubicloud-standard-4-ubuntu-2404"},
    "release.yml": {
        "build-installer": "${{ matrix.os }}",
        "build-dependency-binaries": "${{ matrix.os }}",
        "release-compatibility": "ubuntu-22.04",
        "publish": "ubuntu-latest",
    },
    "rolling-release.yml": {
        "dependency-manifest-changes": "ubuntu-latest",
        "build-lints": "${{ matrix.os }}",
        "build-dependency-binaries": "${{ matrix.os }}",
        "publish": "ubuntu-latest",
    },
}

RETAINED_REUSABLE_WORKFLOW_CALLERS = {
    "dependabot-automerge.yml": "automerge",
    "mutation-testing.yml": "mutation",
}


def _workflow_jobs(workflow_name: str) -> Mapping[str, Any]:
    """Load and return the jobs mapping for a checked-in workflow.

    For example, ``_workflow_jobs("ci.yml")["linux-full"]`` returns the
    mapping whose runner is checked by the pilot contract.
    """
    workflow_path = WORKFLOWS_DIRECTORY / workflow_name
    parsed = YAML(typ="safe").load(workflow_path.read_text(encoding="utf-8"))
    if not isinstance(parsed, dict):
        pytest.fail(f"{workflow_name} must parse to a mapping")
    jobs = parsed.get("jobs")
    if not isinstance(jobs, dict):
        pytest.fail(f"{workflow_name} must declare jobs")
    return jobs


def _assert_runner_assignments(assignments: Mapping[str, Mapping[str, str]]) -> None:
    """Assert that each named workflow job uses its declared runner.

    For example, supplying ``{"ci.yml": {"linux-full": "namespace-profile-default"}}``
    fails if the job is moved to a hosted runner or another Namespace profile.
    """
    for workflow_name, expected_jobs in assignments.items():
        jobs = _workflow_jobs(workflow_name)
        for job_name, expected_runner in expected_jobs.items():
            job = jobs.get(job_name)
            if not isinstance(job, Mapping):
                pytest.fail(f"{workflow_name} must declare {job_name}")
            assert job.get("runs-on") == expected_runner, (
                f"{workflow_name}:{job_name} must run on {expected_runner}"
            )


def test_namespace_pilot_moves_only_pull_request_linux_validation_jobs() -> None:
    """Ensure matching pull-request Linux jobs use the deployed profile."""
    _assert_runner_assignments(MIGRATED_PULL_REQUEST_JOBS)


def test_namespace_pilot_does_not_expand_beyond_the_reviewed_jobs() -> None:
    """Reject unreviewed Namespace assignments in any repository workflow."""
    expected_jobs = {
        (workflow_name, job_name)
        for workflow_name, jobs in MIGRATED_PULL_REQUEST_JOBS.items()
        for job_name in jobs
    }
    actual_jobs = {
        (workflow_path.name, job_name)
        for workflow_glob in WORKFLOW_FILE_GLOBS
        for workflow_path in WORKFLOWS_DIRECTORY.glob(workflow_glob)
        for job_name, job in _workflow_jobs(workflow_path.name).items()
        if isinstance(job, Mapping)
        and isinstance(job.get("runs-on"), str)
        and job["runs-on"].startswith("namespace-profile-")
    }

    assert actual_jobs == expected_jobs, (
        "Namespace runner placement must remain limited to the reviewed pilot jobs"
    )


def test_namespace_pilot_preserves_incompatible_runner_contracts() -> None:
    """Ensure scheduled, release, and Windows jobs retain their platforms."""
    _assert_runner_assignments(RETAINED_RUNNERS)


def test_namespace_pilot_leaves_reusable_workflow_runners_to_their_owners() -> None:
    """Ensure external reusable workflows retain their caller-selected runners."""
    for workflow_name, job_name in RETAINED_REUSABLE_WORKFLOW_CALLERS.items():
        job = _workflow_jobs(workflow_name).get(job_name)
        if not isinstance(job, Mapping):
            pytest.fail(f"{workflow_name} must declare {job_name}")
        assert isinstance(job.get("uses"), str), (
            f"{workflow_name}:{job_name} must remain a reusable workflow call"
        )
        assert "runs-on" not in job, (
            f"{workflow_name}:{job_name} must leave runner selection to its reusable workflow"
        )
