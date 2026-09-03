"""Validate the Ubicloud and GitHub-hosted runner placement policy.

Linux developer-blocking jobs run on Ubicloud because GitHub-hosted queue
times for this account are bimodal. Windows jobs stay GitHub-hosted because
Ubicloud publishes Linux images only. Scheduled, release, and reusable-workflow
jobs stay GitHub-hosted because they are not developer-blocking.

Run this contract directly with:

```sh
PYTHONPATH=. uv run pytest tests/workflows/test_ubicloud_runner_placement.py
```
"""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

import pytest
from ruamel.yaml import YAML

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS_DIRECTORY = REPOSITORY_ROOT / ".github/workflows"
ACTIONLINT_CONFIG = REPOSITORY_ROOT / ".github/actionlint.yaml"
WORKFLOW_FILE_GLOBS = ("*.yaml", "*.yml")

# The recipe's label reference spells an explicit Ubuntu 24.04 x64 two-vCPU
# runner this way. The bare `ubicloud-standard-2` label resolves to the same
# shape, but only the explicit form survives a change to Ubicloud's default
# image.
UBICLOUD_LINUX_RUNNER: str = "ubicloud-standard-2-ubuntu-2404"

UBICLOUD_LINUX_JOBS: dict[str, dict[str, str]] = {
    "ci.yml": {
        "coverage-check": UBICLOUD_LINUX_RUNNER,
        "linux-full": UBICLOUD_LINUX_RUNNER,
    },
    "coverage-main.yml": {"coverage-upload": UBICLOUD_LINUX_RUNNER},
}

GITHUB_HOSTED_RUNNERS = {
    "ci.yml": {"windows-compat": "windows-latest"},
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
    mapping whose runner is checked by the placement contract.
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

    For example, supplying the expected ``linux-full`` label fails if the job
    moves back to a GitHub-hosted or Namespace runner.
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


def _all_jobs() -> list[tuple[str, str, Mapping[str, Any]]]:
    """Return every checked-in workflow job as a name triple."""
    return [
        (workflow_path.name, job_name, job)
        for workflow_glob in WORKFLOW_FILE_GLOBS
        for workflow_path in sorted(WORKFLOWS_DIRECTORY.glob(workflow_glob))
        for job_name, job in _workflow_jobs(workflow_path.name).items()
        if isinstance(job, Mapping)
    ]


def _labels_from_sequence(selector: list[object]) -> list[str] | None:
    """Return a sequence selector's labels, or ``None`` if any is not a string."""
    if all(isinstance(entry, str) for entry in selector):
        return [entry for entry in selector if isinstance(entry, str)]
    return None


def _mapping_labels(selector: Mapping[str, object]) -> list[str] | None:
    """Return the ``labels`` entry of a mapping selector as a string list.

    A bare string is one label. Anything that is not a string or a list of
    strings is unreadable, so the caller fails closed rather than guessing.
    """
    match selector.get("labels", []):
        case str() as label:
            return [label]
        case list() as labels if all(isinstance(entry, str) for entry in labels):
            return [entry for entry in labels if isinstance(entry, str)]
        case _:
            return None


def _labels_from_mapping(selector: Mapping[str, object]) -> list[str] | None:
    """Return a ``group``/``labels`` selector's labels, including the group.

    The group name joins the label list because a runner group named for a
    provider must not exempt its job from the placement checks.
    """
    labels = _mapping_labels(selector)
    if labels is None:
        return None
    match selector.get("group"):
        case None:
            return labels
        case str() as group:
            return [*labels, group]
        case _:
            return None


def _labels_from_selector(selector: object) -> list[str] | None:
    """Return the labels one ``runs-on`` selector requests, or ``None``.

    ``runs-on`` accepts a scalar label, a sequence of labels, or a
    ``group``/``labels`` mapping. ``None`` means the selector matches none of
    those shapes, so callers fail closed instead of skipping the job.
    """
    match selector:
        case str() as label:
            return [label]
        case list() as entries:
            return _labels_from_sequence(entries)
        case Mapping() as mapping:
            return _labels_from_mapping(mapping)
        case _:
            return None


def _runner_labels(job: Mapping[str, object]) -> list[str] | None:
    """Return every runner label a job requests, or ``None`` if unreadable.

    A reusable-workflow call declares ``uses`` and no ``runs-on`` at all, and
    requests no labels of its own. For example, a job whose selector is
    ``{"group": "linux", "labels": ["ubicloud-standard-2-ubuntu-2404"]}``
    yields both strings, so a label smuggled into a mapping is still
    classified.
    """
    if "runs-on" not in job:
        return [] if isinstance(job.get("uses"), str) else None
    return _labels_from_selector(job["runs-on"])


def _jobs_with_label_prefix(prefix: str) -> set[tuple[str, str]]:
    """Return every job requesting at least one label with ``prefix``."""
    return {
        (workflow_name, job_name)
        for workflow_name, job_name, job in _all_jobs()
        if any(label.startswith(prefix) for label in _runner_labels(job) or ())
    }


def test_linux_developer_blocking_jobs_use_the_reviewed_ubicloud_label() -> None:
    """Ensure each blocking Linux job uses the measured two-vCPU shape."""
    _assert_runner_assignments(UBICLOUD_LINUX_JOBS)


def test_every_job_declares_a_classifiable_runner_selector() -> None:
    """Reject a selector the placement contract cannot read.

    An unreadable selector would silently exempt its job from both the
    Namespace and Ubicloud checks below, so it fails here instead.
    """
    unreadable = [
        f"{workflow_name}:{job_name}"
        for workflow_name, job_name, job in _all_jobs()
        if _runner_labels(job) is None
    ]
    assert not unreadable, (
        f"every job must declare a readable runs-on selector: {unreadable}"
    )


def test_no_namespace_runner_labels_remain() -> None:
    """Reject any surviving Namespace profile label."""
    offenders = sorted(_jobs_with_label_prefix("namespace-profile-"))
    assert not offenders, f"Namespace runner labels must be gone: {offenders}"


def test_ubicloud_placement_does_not_expand_beyond_the_reviewed_jobs() -> None:
    """Reject unreviewed Ubicloud assignments in any repository workflow."""
    expected_jobs = {
        (workflow_name, job_name)
        for workflow_name, jobs in UBICLOUD_LINUX_JOBS.items()
        for job_name in jobs
    }
    assert _jobs_with_label_prefix("ubicloud") == expected_jobs, (
        "Ubicloud runner placement must remain limited to the reviewed Linux jobs"
    )


@pytest.mark.parametrize(
    ("job", "expected"),
    [
        pytest.param({"runs-on": "ubuntu-latest"}, ["ubuntu-latest"], id="scalar"),
        pytest.param(
            {"runs-on": ["self-hosted", "namespace-profile-default"]},
            ["self-hosted", "namespace-profile-default"],
            id="sequence",
        ),
        pytest.param(
            {"runs-on": {"group": "linux", "labels": ["ubicloud-standard-2"]}},
            ["ubicloud-standard-2", "linux"],
            id="group-and-labels",
        ),
        pytest.param(
            {"runs-on": {"labels": "ubicloud-standard-2"}},
            ["ubicloud-standard-2"],
            id="scalar-labels-in-mapping",
        ),
        pytest.param({"uses": "./.github/workflows/x.yml"}, [], id="reusable-call"),
        pytest.param({"runs-on": 4}, None, id="non-string-scalar"),
        pytest.param({"runs-on": ["ok", 4]}, None, id="non-string-in-sequence"),
        pytest.param({"runs-on": {"group": 4}}, None, id="non-string-group"),
        pytest.param({"steps": []}, None, id="missing-selector"),
    ],
)
def test_runner_labels_reads_every_supported_selector_form(
    job: Mapping[str, object],
    expected: list[str] | None,
) -> None:
    """Every documented selector form is read, and nothing else is trusted."""
    assert _runner_labels(job) == expected, f"unexpected labels for {job}"


def test_windows_and_release_jobs_stay_github_hosted() -> None:
    """Ensure Windows, release, and rolling-release jobs keep hosted runners."""
    _assert_runner_assignments(GITHUB_HOSTED_RUNNERS)


def test_scheduled_and_administrative_workflows_stay_github_hosted() -> None:
    """Ensure the nightly mutation lane never acquires a Ubicloud label."""
    for workflow_name, job_name in RETAINED_REUSABLE_WORKFLOW_CALLERS.items():
        job = _workflow_jobs(workflow_name).get(job_name)
        if not isinstance(job, Mapping):
            pytest.fail(f"{workflow_name} must declare {job_name}")
        assert isinstance(job.get("uses"), str), (
            f"{workflow_name}:{job_name} must remain a reusable workflow call"
        )
        assert "runs-on" not in job, (
            f"{workflow_name}:{job_name} must leave runner selection to its "
            "reusable workflow"
        )


def test_ubicloud_jobs_declare_a_timeout() -> None:
    """Ubicloud runners are self-hosted, so GitHub's six-hour cap does not apply.

    A hung job would otherwise bill for days against the five-day self-hosted
    limit, so every Ubicloud job must cap itself.
    """
    for workflow_name, expected_jobs in UBICLOUD_LINUX_JOBS.items():
        jobs = _workflow_jobs(workflow_name)
        for job_name in expected_jobs:
            timeout = jobs[job_name].get("timeout-minutes")
            assert isinstance(timeout, int) and 0 < timeout <= 60, (
                f"{workflow_name}:{job_name} must declare a bounded timeout-minutes"
            )


def test_actionlint_registers_only_the_labels_in_use() -> None:
    """Keep the actionlint label registry equal to the deployed label set."""
    parsed = YAML(typ="safe").load(ACTIONLINT_CONFIG.read_text(encoding="utf-8"))
    labels = parsed["self-hosted-runner"]["labels"]
    assert labels == [UBICLOUD_LINUX_RUNNER], (
        "actionlint must register exactly the self-hosted labels in use"
    )


def test_required_status_check_contexts_are_not_renamed() -> None:
    """The `main` ruleset requires `linux-full` and `windows-compat` by name.

    GitHub derives a check context from the job name, so giving either job an
    explicit `name` (or a matrix that embeds the runner label) would leave the
    ruleset waiting for a context the workflow no longer emits.
    """
    jobs = _workflow_jobs("ci.yml")
    for job_name in ("linux-full", "windows-compat"):
        assert "name" not in jobs[job_name], (
            f"{job_name} must keep its job id as its required check context"
        )
        assert "strategy" not in jobs[job_name], (
            f"{job_name} must not become a matrix job; that renames its context"
        )
