"""Shared loaders for the Ubicloud workflow contract tests.

The contract tests read the checked-in workflows as data rather than running
them, so they need a small vocabulary for "the jobs that run on Ubicloud",
"the cache steps in a job", and "the paths a cache step owns". Keeping that
vocabulary here lets each contract module stay focused on one policy.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS_DIRECTORY = REPOSITORY_ROOT / ".github/workflows"

#: `actions/cache` v6.1.0. Ubicloud's transparent cache intercepts this
#: version, so Linux archives land in Ubicloud's store and Windows archives
#: land in GitHub's, from one action and one pin. The deprecated
#: `ubicloud/cache` fork is deliberately not used.
CACHE_ACTION_SHA = "55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
RESTORE_ACTION = f"actions/cache/restore@{CACHE_ACTION_SHA}"
SAVE_ACTION = f"actions/cache/save@{CACHE_ACTION_SHA}"
INSTALL_ACTION = "taiki-e/install-action@18b1216eba7f8039b0f8d131d5473787f0edce68"
SETUP_RUST_ACTION = (
    "leynos/shared-actions/.github/actions/setup-rust@"
    "5daae0a332441d170d88ca648c9e71f0bbe96cb3"
)

#: Every Ubicloud job, mapped to the workflow that declares it.
UBICLOUD_JOBS: dict[str, str] = {
    "coverage-check": "ci.yml",
    "linux-full": "ci.yml",
    "coverage-upload": "coverage-main.yml",
}

#: Every job that owns cache archives, including the GitHub-hosted Windows
#: lane. Cache ownership rules apply to all of them; Ubicloud-specific rules
#: such as the backend selector apply only to `UBICLOUD_JOBS`.
CACHING_JOBS: dict[str, str] = UBICLOUD_JOBS | {"windows-compat": "ci.yml"}

#: The single job permitted to save each cache key family.
CACHE_KEY_WRITERS: dict[str, str] = {
    "cargo-registry-coverage-v1-": "coverage-upload",
    "cargo-registry-lint-v1-": "linux-full",
    "tools-coverage-v1-": "coverage-upload",
    "tools-lint-v1-": "linux-full",
    "dylint-tools-v1-": "linux-full",
    "clippy-mirror-v1-": "coverage-upload",
    "sccache-coverage-v1-": "coverage-upload",
    "sccache-lint-v1-": "linux-full",
    "cargo-registry-windows-v1-": "windows-compat",
}


def load_workflow(workflow_name: str) -> dict[str, Any]:
    """Return one checked-in workflow parsed as a mapping.

    For example, ``load_workflow("ci.yml")["jobs"]`` yields the CI job set.
    """
    workflow_path = WORKFLOWS_DIRECTORY / workflow_name
    workflow = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), f"{workflow_name} must parse to a mapping"
    return workflow


def load_job(job_name: str) -> dict[str, Any]:
    """Return one cache-owning job mapping by name.

    For example, ``load_job("linux-full")["steps"]`` yields its step list.
    """
    workflow = load_workflow(CACHING_JOBS[job_name])
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "workflow must declare a jobs mapping"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"workflow must declare the {job_name} job"
    return job


def job_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the ordered step list for one job."""
    steps = job.get("steps")
    assert isinstance(steps, list), "job must declare a step list"
    return steps


def step_names(job: dict[str, Any]) -> list[str]:
    """Return the ordered step names for one job."""
    return [step["name"] for step in job_steps(job) if isinstance(step.get("name"), str)]


def steps_by_name(job: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Index a job's named steps.

    For example, ``steps_by_name(job)["Setup Rust"]`` returns that step.
    """
    return {step["name"]: step for step in job_steps(job) if "name" in step}


def cache_paths(step: dict[str, Any]) -> list[str]:
    """Return the paths one cache step owns, in declaration order.

    For example, a step whose ``path`` is a two-line block returns both
    entries so a caller can detect two steps claiming the same directory.
    """
    raw = step.get("with", {}).get("path", "")
    return [line.strip() for line in str(raw).splitlines() if line.strip()]


def restore_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the job's cache restore steps in declaration order."""
    return [step for step in job_steps(job) if step.get("uses") == RESTORE_ACTION]


def save_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the job's cache save steps in declaration order."""
    return [step for step in job_steps(job) if step.get("uses") == SAVE_ACTION]


def run_scripts(job: dict[str, Any]) -> str:
    """Return every inline shell script in a job, joined for substring checks."""
    return "\n".join(str(step.get("run", "")) for step in job_steps(job))
