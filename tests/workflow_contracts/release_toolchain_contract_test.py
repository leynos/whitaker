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
import tomllib
import typing as typ
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
RELEASE_WORKFLOWS = ("release.yml", "rolling-release.yml")
#: A command line that selects the stable channel.
STABLE_COMMAND = re.compile(
    r"\+stable\b|toolchain\s+(?:install|default)\s+stable\b|--toolchain\s+stable\b"
    r"|rustup\s+(?:override|default)\s+(?:set\s+)?stable\b"
)


def _steps(workflow: dict[str, typ.Any]) -> typ.Iterator[tuple[str, dict[str, typ.Any]]]:
    """Yield ``(job name, step)`` for every step of every job."""
    for job_name, job in (workflow.get("jobs") or {}).items():
        for step in job.get("steps") or []:
            yield job_name, step


def _overrides_toolchain(env: typ.Any) -> bool:
    """Return whether an ``env`` mapping forces a non-pinned toolchain."""
    if not isinstance(env, dict):
        return False
    value = str(env.get("RUSTUP_TOOLCHAIN", "")).strip()
    return bool(value) and not value.startswith(("nightly", "${{"))


def stable_selections(workflow: dict[str, typ.Any]) -> list[str]:
    """Return a description of every place that selects a non-pinned toolchain."""
    found = []
    if _overrides_toolchain(workflow.get("env")):
        found.append("the workflow sets RUSTUP_TOOLCHAIN")
    for job_name, job in (workflow.get("jobs") or {}).items():
        if _overrides_toolchain(job.get("env")):
            found.append(f"{job_name} sets RUSTUP_TOOLCHAIN")
    for job_name, step in _steps(workflow):
        label = f"{job_name}: {step.get('name', step.get('uses', '<unnamed>'))}"
        if STABLE_COMMAND.search(str(step.get("run", ""))):
            found.append(f"{label} runs a stable-channel command")
        if _overrides_toolchain(step.get("env")):
            found.append(f"{label} sets RUSTUP_TOOLCHAIN")
        inputs = step.get("with") or {}
        if str(inputs.get("toolchain", "")).strip() == "stable":
            found.append(f"{label} passes toolchain: stable")
    return found


def test_the_repository_toolchain_is_a_pinned_nightly() -> None:
    """The implicit selection the release jobs rely on is a dated nightly."""
    channel = tomllib.loads((ROOT / "rust-toolchain.toml").read_text("utf-8"))["toolchain"]["channel"]
    assert re.fullmatch(r"nightly-\d{4}-\d{2}-\d{2}", channel), channel


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
        {"name": "override", "run": "rustup override set stable"},
        {"name": "env", "run": "cargo build", "env": {"RUSTUP_TOOLCHAIN": "stable"}},
    ],
)
def test_the_reader_rejects_each_way_of_selecting_stable(step: dict[str, typ.Any]) -> None:
    """Mutation: putting the stable channel back makes the check fail."""
    workflow = {"jobs": {"release": {"steps": [step]}}}
    assert stable_selections(workflow), f"not detected: {step}"


@pytest.mark.parametrize("place", ["workflow", "job"])
def test_the_reader_rejects_a_workflow_or_job_toolchain_override(place: str) -> None:
    """Mutation: ``RUSTUP_TOOLCHAIN`` at workflow or job level makes the check fail."""
    env = {"RUSTUP_TOOLCHAIN": "stable"}
    workflow: dict[str, typ.Any] = {"jobs": {"release": {"steps": [], **({"env": env} if place == "job" else {})}}}
    if place == "workflow":
        workflow["env"] = env
    assert stable_selections(workflow)


def test_the_reader_accepts_the_pinned_toolchain() -> None:
    """A plain ``cargo install`` under the pinned nightly is fine."""
    step = {"name": "install", "run": "cargo install --locked --version 6.3.0 cargo-dylint"}
    assert stable_selections({"jobs": {"release": {"steps": [step]}}}) == []
