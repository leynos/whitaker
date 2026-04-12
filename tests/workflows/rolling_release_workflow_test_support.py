"""Helpers for rolling-release workflow contract tests.

This module centralizes the small parsing and shell-inspection utilities used
by the rolling-release workflow contract tests under `tests/workflows/`.

It provides:
- `_load_workflow_mapping()` to parse workflow YAML into a validated mapping.
- `_get_job_dict()` to fetch job mappings such as `jobs["publish"]`.
- `_workflow_dispatch_inputs()` to return `workflow_dispatch.inputs`.
- `_github_expression_value()` to normalize `${{ ... }}` expressions.
- `_get_needs_list()` to normalize a job `needs` field to `list[str]`.
- `_find_step_by_name()` to locate named workflow steps in a step list.
- `_nesting_delta()`, `_collect_branch_lines()`, and
  `_workflow_dispatch_branch_body()` to inspect shell-script branch bodies
  without including the closing `fi`.

Typical usage in tests is to pass raw workflow YAML text or a step `run`
script, then assert against the validated mapping or extracted branch text.
Helpers fail with `pytest.fail()` when the workflow structure is missing or
malformed so tests report a precise contract error.

Example
-------
```python
from tests.workflows.rolling_release_workflow_test_support import (
    _load_workflow_mapping,
    _workflow_dispatch_inputs,
)

workflow_mapping = _load_workflow_mapping(workflow_text)
inputs = _workflow_dispatch_inputs(workflow_mapping)
```
"""

from __future__ import annotations

import re
from collections.abc import Mapping
from typing import Any, cast

import pytest
from ruamel.yaml import YAML


def _load_workflow_mapping(yaml_text: str) -> dict[str, object]:
    """Load YAML text and return workflow mapping."""
    parsed = YAML(typ="safe").load(yaml_text)
    match parsed:
        case dict() as workflow_mapping:
            return workflow_mapping
        case _:
            pytest.fail("rolling-release workflow must parse to a mapping")


def _get_job_dict(jobs: Mapping[str, Any], job_name: str) -> dict[str, Any]:
    """Return the requested job mapping from the jobs map."""
    match jobs.get(job_name):
        case dict() as job_dict:
            return job_dict
        case _:
            if job_name == "jobs":
                pytest.fail("rolling-release workflow must declare jobs")
            pytest.fail(f"rolling-release workflow must declare {job_name} job")


def _workflow_dispatch_inputs(workflow_mapping: Mapping[str, Any]) -> dict[str, Any]:
    """Return the `workflow_dispatch.inputs` mapping."""
    on_mapping = workflow_mapping.get("on")
    match on_mapping:
        case {"workflow_dispatch": {"inputs": dict() as inputs}}:
            return inputs
        case {"workflow_dispatch": dict()}:
            pytest.fail("workflow_dispatch must declare an inputs mapping")
        case {"workflow_dispatch": _}:
            pytest.fail("workflow_dispatch trigger must be a mapping")
        case _:
            pytest.fail("rolling-release workflow must declare workflow_dispatch")


def _github_expression_value(value: object) -> str:
    """Return a GitHub Actions expression with wrapper delimiters removed."""
    if not isinstance(value, str):
        pytest.fail("workflow expression value must be a string")
    stripped = value.strip()
    if stripped.startswith("${{") and stripped.endswith("}}"):
        return stripped[3:-2].strip()
    return stripped


def _get_needs_list(publish_job: dict[str, Any]) -> list[str]:
    """Return publish job dependency names as a list."""
    needs: str | list[str] | None = publish_job.get("needs")
    match needs:
        case str():
            return [needs]
        case list():
            if all(isinstance(item, str) for item in needs):
                return cast("list[str]", needs)
            pytest.fail("publish job needs list must contain only strings")
        case _:
            pytest.fail("publish job needs must be a string or list")


def _find_step_by_name(steps: object, name: str) -> dict[str, object] | None:
    """Find a step dict by its name."""
    match steps:
        case list():
            pass
        case _:
            pytest.fail("publish job must declare steps")

    for step in steps:
        match step:
            case {"name": step_name} if step_name == name:
                return step
            case _:
                continue
    return None


def _nesting_delta(stripped_line: str) -> int:
    """Return the change in shell if/fi nesting depth for a given stripped line."""
    if re.match(r"^if\b", stripped_line):
        return 1
    if stripped_line == "fi":
        return -1
    return 0


def _collect_branch_lines(script_tail: str) -> list[str] | None:
    """Collect branch-body lines up to the closing fi and return them or None."""
    branch_lines: list[str] = []
    nesting_depth = 1
    for line in script_tail.splitlines():
        nesting_depth += _nesting_delta(line.strip())
        if nesting_depth == 0:
            return branch_lines
        branch_lines.append(line)
    return None


def _workflow_dispatch_branch_body(run_script: str) -> str:
    """Extract the outer `workflow_dispatch` branch body from a shell script."""
    dispatch_branch_match = re.search(
        r'^\s*if\s+\[\[\s+"\$\{\{\s*github\.event_name\s*\}\}"\s+==\s+"workflow_dispatch"\s+\]\]\s*;\s*then\s*$',
        run_script,
        re.MULTILINE,
    )
    if dispatch_branch_match is None:
        pytest.fail(
            "change-detection step must branch explicitly on workflow_dispatch"
        )
    branch_lines = _collect_branch_lines(run_script[dispatch_branch_match.end() :])
    if branch_lines is None:
        pytest.fail("workflow_dispatch branch must terminate with fi")
    return "\n".join(branch_lines)
