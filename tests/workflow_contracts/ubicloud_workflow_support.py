"""Shared loaders for the Ubicloud workflow contract tests.

The contract tests read the checked-in workflows as data rather than running
them, so they need a small vocabulary for "the jobs that run on Ubicloud",
"the cache steps in a job", and "the paths a cache step owns". Keeping that
vocabulary here lets each contract module stay focused on one policy.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path
from typing import Any

import yaml

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from collections.abc import Iterable

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
#: shared-actions `main` at the merge of #422, #425, #427, #428, and #432.
#: This is the first revision whose built-in `github` cache provider stops
#: archiving `target/<profile>`, so no shared action can reintroduce the
#: second owner of compiler output that `sccache` already owns. Every caller
#: in this repository pins this one revision.
SHARED_ACTIONS_REF = "f6d4d5f549655c118f86f371b8d55c200d3efa50"
SETUP_RUST_ACTION = (
    f"leynos/shared-actions/.github/actions/setup-rust@{SHARED_ACTIONS_REF}"
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


def all_jobs() -> list[tuple[str, str, dict[str, Any]]]:
    """Return every job in every checked-in workflow as a name triple.

    For example, a caller can scan the result for jobs whose inline scripts
    execute the test suite, without listing the workflow files itself.
    """
    discovered: list[tuple[str, str, dict[str, Any]]] = []
    for pattern in ("*.yml", "*.yaml"):
        for workflow_path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            jobs = load_workflow(workflow_path.name).get("jobs")
            if not isinstance(jobs, dict):
                continue
            discovered.extend(
                (workflow_path.name, job_name, job)
                for job_name, job in jobs.items()
                if isinstance(job, dict)
            )
    return discovered


def job_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the ordered step list for one job."""
    steps = job.get("steps")
    assert isinstance(steps, list), "job must declare a step list"
    return steps


def step_names(job: dict[str, Any]) -> list[str]:
    """Return the ordered step names for one job.

    Every step must be named. Silently dropping an unnamed step would let it
    sit anywhere in the order, including ahead of the compiler-cache
    credential export whose position the ordering contracts police.
    """
    names: list[str] = []
    for index, step in enumerate(job_steps(job)):
        name = step.get("name")
        assert isinstance(name, str), f"step {index} must declare a name: {step!r}"
        names.append(name)
    return names


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


def duplicate_path_owners(steps: list[dict[str, Any]]) -> dict[str, list[str]]:
    """Return each cached path claimed by more than one of ``steps``.

    Ownership is the invariant the whole cache design rests on, so it is
    computed here as a pure function over a step list rather than asserted
    inline: the concrete workflows and the generated cases in the property
    suite then exercise one implementation.

    For example, two steps that both list ``~/.cargo/bin`` yield
    ``{"~/.cargo/bin": ["Restore Cargo registry", "Restore the tools"]}``.
    """
    owners: dict[str, list[str]] = {}
    for step in steps:
        name = str(step.get("name", ""))
        for path in cache_paths(step):
            owners.setdefault(path, []).append(name)
    return {path: names for path, names in owners.items() if len(names) > 1}


def key_family(key: str, families: Iterable[str]) -> str | None:
    """Return the reviewed key family a rendered cache key belongs to.

    The longest matching prefix wins, so a family that is itself a prefix of
    another never captures the more specific one. Returns ``None`` when no
    family claims the key, which the contract treats as an unreviewed key.

    For example, ``key_family("sccache-lint-v1-Linux", CACHE_KEY_WRITERS)``
    returns ``"sccache-lint-v1-"``.
    """
    matches = [family for family in families if key.startswith(family)]
    return max(matches, key=len) if matches else None


def restore_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the job's cache restore steps in declaration order."""
    return [step for step in job_steps(job) if step.get("uses") == RESTORE_ACTION]


def save_steps(job: dict[str, Any]) -> list[dict[str, Any]]:
    """Return the job's cache save steps in declaration order."""
    return [step for step in job_steps(job) if step.get("uses") == SAVE_ACTION]


def run_scripts(job: dict[str, Any]) -> str:
    """Return every inline shell script in a job, joined for substring checks."""
    return "\n".join(str(step.get("run", "")) for step in job_steps(job))
