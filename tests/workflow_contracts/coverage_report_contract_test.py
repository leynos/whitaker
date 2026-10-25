"""Enforce fresh-report and ownership contracts for workflow LCOV reports."""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

import pytest
from ruamel.yaml import YAML

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_NAMES = ("ci.yml", "coverage-main.yml")
CODESCENE_UPLOAD_ACTION = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage"
)


@pytest.fixture(scope="module")
def workflows() -> dict[str, dict[str, Any]]:
    """Load the pull-request and main-branch coverage workflow documents.

    Returns
    -------
    dict[str, dict[str, Any]]
        Parsed workflow mappings keyed by workflow filename.
    """
    parsed_workflows: dict[str, dict[str, Any]] = {}
    for workflow_name in WORKFLOW_NAMES:
        workflow_path = REPOSITORY_ROOT / ".github/workflows" / workflow_name
        parsed = YAML(typ="safe").load(workflow_path.read_text(encoding="utf-8"))
        assert isinstance(parsed, dict), f"{workflow_path} must contain a mapping"
        parsed_workflows[workflow_name] = parsed
    return parsed_workflows


def _job_steps(workflow: Mapping[str, Any], job_name: str) -> list[Mapping[str, Any]]:
    """Return the steps declared by a named workflow job."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, Mapping), "workflow must declare a jobs mapping"
    job = jobs.get(job_name)
    assert isinstance(job, Mapping), f"workflow must declare the {job_name} job"
    steps = job.get("steps")
    assert isinstance(steps, list), f"{job_name} must declare ordered steps"
    assert all(isinstance(step, Mapping) for step in steps), (
        f"{job_name} steps must be mappings"
    )
    return steps


def _named_step_index(steps: list[Mapping[str, Any]], name: str) -> int:
    """Return the index of a step with the requested name."""
    for index, step in enumerate(steps):
        if step.get("name") == name:
            return index
    pytest.fail(f"workflow must declare a {name!r} step")


@pytest.mark.parametrize(
    ("workflow_name", "job_name"),
    [
        pytest.param("ci.yml", "coverage-check", id="pull-request-check"),
        pytest.param("coverage-main.yml", "coverage-upload", id="main-upload"),
    ],
)
def test_coverage_lanes_validate_a_fresh_root_lcov_report(
    workflows: Mapping[str, Mapping[str, Any]],
    workflow_name: str,
    job_name: str,
) -> None:
    """Require both lanes to discard stale output and validate new LCOV.

    Parameters
    ----------
    workflows
        Workflow mappings keyed by filename.
    workflow_name
        Filename of the workflow whose coverage lane is checked.
    job_name
        Name of the coverage job within the selected workflow.
    """
    steps = _job_steps(workflows[workflow_name], job_name)
    generate_index = _named_step_index(steps, "Generate coverage")
    fetch_index = _named_step_index(steps, "Fetch dependencies and warm Dylint driver")
    assert fetch_index < generate_index, (
        f"{job_name} must fetch dependencies and warm Dylint before coverage tests"
    )
    assert steps[fetch_index].get("run") == "make fetch"
    assert generate_index > 0, f"{job_name} must clear stale LCOV before generation"
    remove_stale = steps[generate_index - 1]
    generate = steps[generate_index]
    assert remove_stale.get("name") == "Remove stale LCOV report"
    assert remove_stale.get("run") == "rm -f lcov.info"
    assert generate.get("env") == {"COVERAGE_OUTPUT": "lcov.info"}
    assert generate.get("run") == "make coverage"

    verify_index = generate_index + 1
    assert verify_index < len(steps), f"{job_name} must verify generated LCOV"
    verify = steps[verify_index]
    assert verify.get("name") == "Verify LCOV report", (
        f"{job_name} must validate LCOV immediately after generation"
    )
    script = verify.get("run")
    assert isinstance(script, str), f"{job_name} must run an LCOV validation script"
    assert "test -s lcov.info" in script, f"{job_name} must reject absent or empty LCOV"
    assert "^SF:.+" in script and "^end_of_record$" in script, (
        f"{job_name} must require complete LCOV source records"
    )
    assert "^DA:" in script and "measurements == 0" in script, (
        f"{job_name} must require at least one LCOV line measurement"
    )


def test_only_main_uploads_the_explicit_root_lcov_report(
    workflows: Mapping[str, Mapping[str, Any]],
) -> None:
    """Keep PR validation read-only and bind main upload to fresh root LCOV.

    Parameters
    ----------
    workflows
        Workflow mappings keyed by filename.
    """
    pull_request_steps = _job_steps(workflows["ci.yml"], "coverage-check")
    main_steps = _job_steps(workflows["coverage-main.yml"], "coverage-upload")
    pull_request_uploads = [
        step
        for step in pull_request_steps
        if CODESCENE_UPLOAD_ACTION in str(step.get("uses", ""))
    ]
    main_uploads = [
        step
        for step in main_steps
        if CODESCENE_UPLOAD_ACTION in str(step.get("uses", ""))
    ]

    assert not pull_request_uploads, "PR coverage validation must not upload a branch"
    assert len(main_uploads) == 1, "main must retain exactly one CodeScene uploader"
    inputs = main_uploads[0].get("with")
    assert isinstance(inputs, Mapping), "main CodeScene uploader must declare inputs"
    assert inputs.get("format") == "lcov"
    assert inputs.get("mode") == "upload"
    assert inputs.get("path") == "lcov.info"


def test_coverage_lanes_use_the_shared_online_profile_pool(
    workflows: Mapping[str, Mapping[str, Any]],
) -> None:
    """Keep both workflows on the Makefile's bounded profile pattern."""
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    assert "COVERAGE_PROFILE_FILE_NAME ?= whitaker-%m.profraw" in makefile
    assert "COVERAGE_PROFILE_FILE_NAME ?= whitaker-%p-%m.profraw" not in makefile

    for workflow_name, job_name in (
        ("ci.yml", "coverage-check"),
        ("coverage-main.yml", "coverage-upload"),
    ):
        steps = _job_steps(workflows[workflow_name], job_name)
        generate = steps[_named_step_index(steps, "Generate coverage")]
        assert generate.get("run") == "make coverage", (
            f"{workflow_name}:{job_name} must use the shared profile policy"
        )
