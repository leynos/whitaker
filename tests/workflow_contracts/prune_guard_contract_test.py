"""Hold the pre-publish prune check ahead of the rolling-release upload.

The check stops an archive the oldest offered installer cannot prune from
reaching consumers (whitaker issues 355 and 565). It protects nothing if it
runs after the upload, can fail without ending the job, or names no installer
tag. These tests read the parsed workflow and assert those properties, not its
text.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ

from ubicloud_workflow_support import load_workflow

WORKFLOW_NAME: typ.Final[str] = "rolling-release.yml"
CHECK_COMMAND: typ.Final[str] = "check_prune_safety.py"
PUBLISH_COMMAND: typ.Final[str] = "publish-rolling-release.sh"


def _publish_steps() -> list[dict[str, typ.Any]]:
    """Return the steps of the publish job."""
    return load_workflow(WORKFLOW_NAME)["jobs"]["publish"]["steps"]


def _index_running(steps: list[dict[str, typ.Any]], command: str) -> int:
    """Return the index of the step whose ``run`` names ``command``."""
    matches = [i for i, step in enumerate(steps) if command in str(step.get("run", ""))]
    assert len(matches) == 1, f"expected one step running {command}, found {matches}"
    return matches[0]


def test_prune_check_runs_before_the_upload() -> None:
    """The check precedes the step that republishes the release."""
    steps = _publish_steps()
    assert _index_running(steps, CHECK_COMMAND) < _index_running(steps, PUBLISH_COMMAND)


def test_prune_check_failure_ends_the_job() -> None:
    """A failing check must not be tolerated by the job."""
    step = _publish_steps()[_index_running(_publish_steps(), CHECK_COMMAND)]
    assert not step.get("continue-on-error"), "the prune check must not continue on error"
    run = str(step["run"])
    assert "set -euo pipefail" in run or "set -e" in run
    assert "|| true" not in run


def test_prune_check_gates_the_same_publication() -> None:
    """The check runs under the same condition as the upload it guards."""
    steps = _publish_steps()
    check = steps[_index_running(steps, CHECK_COMMAND)]
    upload = steps[_index_running(steps, PUBLISH_COMMAND)]
    assert check.get("if") == upload.get("if")


def test_prune_check_names_a_release_tag() -> None:
    """The installer tag comes from a workflow env that looks like a release tag."""
    tag = load_workflow(WORKFLOW_NAME)["env"].get("OLDEST_INSTALLER_TAG")
    assert tag is not None, "OLDEST_INSTALLER_TAG is missing"
    assert re.fullmatch(r"v\d+\.\d+\.\d+", str(tag)), f"not a release tag: {tag!r}"
