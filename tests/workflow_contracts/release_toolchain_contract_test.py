"""Contract: the release workflows build on the repository's pinned nightly.

Whitaker is an application, so it takes no stable exemption. A release job
selects its toolchain from ``rust-toolchain.toml`` through ``setup-rust``, and
no step names the stable channel: no ``cargo +stable``, no
``rustup toolchain install stable`` and no ``toolchain: stable`` input. The
check parses the workflows and inspects every step, so it does not depend on
how a step is laid out or commented.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
RELEASE_WORKFLOWS = ("release.yml", "rolling-release.yml")
#: A command line that selects the stable channel.
STABLE_COMMAND = re.compile(r"\+stable\b|toolchain\s+(?:install|default)\s+stable\b|--toolchain\s+stable\b")


def _steps(workflow: dict[str, typ.Any]) -> typ.Iterator[tuple[str, dict[str, typ.Any]]]:
    """Yield ``(job name, step)`` for every step of every job."""
    for job_name, job in (workflow.get("jobs") or {}).items():
        for step in job.get("steps") or []:
            yield job_name, step


def stable_selections(workflow: dict[str, typ.Any]) -> list[str]:
    """Return a description of every step that selects the stable channel."""
    found = []
    for job_name, step in _steps(workflow):
        label = f"{job_name}: {step.get('name', step.get('uses', '<unnamed>'))}"
        if STABLE_COMMAND.search(str(step.get("run", ""))):
            found.append(f"{label} runs a stable-channel command")
        inputs = step.get("with") or {}
        if str(inputs.get("toolchain", "")).strip() == "stable":
            found.append(f"{label} passes toolchain: stable")
    return found


@pytest.mark.parametrize("name", RELEASE_WORKFLOWS)
def test_release_workflows_select_no_stable_toolchain(name: str) -> None:
    """No release step falls back to the stable channel."""
    workflow = yaml.safe_load((ROOT / ".github" / "workflows" / name).read_text("utf-8"))
    assert stable_selections(workflow) == [], f"{name} selects the stable channel"


@pytest.mark.parametrize(
    "step",
    [
        {"name": "install", "run": "cargo +stable install --locked cargo-dylint"},
        {"name": "tool", "run": "rustup toolchain install stable --profile minimal"},
        {"name": "target", "run": "rustup target add --toolchain stable x86_64-unknown-linux-gnu"},
        {"name": "action", "uses": "x/y@z", "with": {"toolchain": "stable"}},
    ],
)
def test_the_reader_rejects_each_way_of_selecting_stable(step: dict[str, typ.Any]) -> None:
    """Mutation: putting the stable channel back makes the check fail."""
    workflow = {"jobs": {"release": {"steps": [step]}}}
    assert stable_selections(workflow), f"not detected: {step}"


def test_the_reader_accepts_the_pinned_toolchain() -> None:
    """A plain ``cargo install`` under the pinned nightly is fine."""
    step = {"name": "install", "run": "cargo install --locked --version 6.3.0 cargo-dylint"}
    assert stable_selections({"jobs": {"release": {"steps": [step]}}}) == []
