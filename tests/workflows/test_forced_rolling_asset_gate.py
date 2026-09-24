"""Exercise the forced rolling rebuild gate before release publication."""

from __future__ import annotations

import hashlib
import os
import re
import subprocess
import tomllib
from pathlib import Path

import pytest

from tests.workflows.rolling_release_workflow_test_support import (
    _find_step_by_name,
    _get_job_dict,
    _github_expression_mentions_operand,
    _load_workflow_mapping,
)

ROOT = Path(__file__).resolve().parents[2]
GATE = ROOT / "scripts/check-forced-rolling-assets.sh"
TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)

def _dependencies() -> tuple[tuple[str, str], ...]:
    """Read package names and versions from the workflow's actual manifest."""
    manifest = tomllib.loads(
        (ROOT / "installer/dependency-binaries.toml").read_text(encoding="utf-8")
    )
    return tuple(
        (entry["package"], entry["version"])
        for entry in manifest["dependency_binaries"]
    )


def _run_gate(dist: Path, **overrides: str) -> subprocess.CompletedProcess[str]:
    """Run the real prepublication gate with controlled workflow inputs."""
    environment = os.environ.copy()
    environment.update(
        GITHUB_EVENT_NAME="workflow_dispatch",
        FORCE_DEPENDENCY_BINARY_REBUILD="true",
        LINT_BUILD_RESULT="success",
        DEPENDENCY_BUILD_RESULT="success",
        ROLLING_DIST_DIR=str(dist),
    )
    environment.update(overrides)
    return subprocess.run(
        ["bash", str(GATE)],
        cwd=ROOT,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )


@pytest.fixture
def complete_dist(tmp_path: Path) -> Path:
    """Create all expected rolling assets with matching checksum sidecars."""
    dist = tmp_path / "dist"
    dist.mkdir()
    short_sha = subprocess.check_output(
        ["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, text=True
    ).strip()
    toolchain = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8").split('"')[1]
    for target in TARGETS:
        (dist / f"whitaker-lints-{short_sha}-{toolchain}-{target}.tar.zst").write_bytes(b"lint")
        (dist / f"manifest-{target}.json").write_text("{}", encoding="utf-8")
        extension = "zip" if "windows" in target else "tgz"
        for package, version in _dependencies():
            archive = dist / f"{package}-{target}-v{version}.{extension}"
            content = f"{package}-{target}".encode()
            archive.write_bytes(content)
            digest = hashlib.sha256(content).hexdigest()
            (dist / f"{archive.name}.sha256").write_text(
                f"{digest}  {archive}\n", encoding="utf-8"
            )
    return dist


def test_complete_forced_rebuild_passes(complete_dist: Path) -> None:
    """A complete matrix output can reach the publisher."""
    result = _run_gate(complete_dist)
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("failed_matrix", ["LINT_BUILD_RESULT", "DEPENDENCY_BUILD_RESULT"])
def test_failed_matrix_blocks_forced_publication(
    complete_dist: Path, failed_matrix: str
) -> None:
    """Either failed matrix prevents mutation even with complete artefacts."""
    result = _run_gate(complete_dist, **{failed_matrix: "failure"})
    assert result.returncode != 0
    assert "requires both complete build matrices" in result.stderr


@pytest.mark.parametrize("missing_kind", ["manifest", "archive", "checksum"])
def test_missing_forced_asset_blocks_publication(
    complete_dist: Path, missing_kind: str
) -> None:
    """Missing lint metadata, dependency archive, or checksum each fail."""
    package, version = _dependencies()[0]
    archive = f"{package}-x86_64-unknown-linux-gnu-v{version}.tgz"
    missing_name = {
        "manifest": "manifest-x86_64-unknown-linux-gnu.json",
        "archive": archive,
        "checksum": f"{archive}.sha256",
    }[missing_kind]
    (complete_dist / missing_name).unlink()
    result = _run_gate(complete_dist)
    assert result.returncode != 0
    assert missing_name in result.stderr


def test_wrong_checksum_blocks_forced_publication(complete_dist: Path) -> None:
    """A present sidecar cannot approve an archive with different bytes."""
    package, version = _dependencies()[0]
    archive = complete_dist / f"{package}-x86_64-unknown-linux-gnu-v{version}.tgz"
    archive.write_bytes(b"changed")
    result = _run_gate(complete_dist)
    assert result.returncode != 0
    assert "Checksum or filename does not match" in result.stderr


def test_sidecar_cannot_verify_a_different_file(complete_dist: Path) -> None:
    """A matching digest for another file does not approve the archive."""
    package, version = _dependencies()[0]
    archive = complete_dist / f"{package}-x86_64-unknown-linux-gnu-v{version}.tgz"
    other = complete_dist / "unrelated"
    other.write_bytes(b"unrelated")
    digest = hashlib.sha256(other.read_bytes()).hexdigest()
    (complete_dist / f"{archive.name}.sha256").write_text(
        f"{digest}  {other}\n", encoding="utf-8"
    )
    result = _run_gate(complete_dist)
    assert result.returncode != 0
    assert archive.name in result.stderr


def test_push_keeps_partial_publication_policy(tmp_path: Path) -> None:
    """The push path does not demand complete forced-rebuild artefacts."""
    result = _run_gate(
        tmp_path / "missing", GITHUB_EVENT_NAME="push", LINT_BUILD_RESULT="failure"
    )
    assert result.returncode == 0, result.stderr


def test_workflow_runs_gate_before_publisher(workflow_text: str) -> None:
    """The publisher cannot run without the gate's successful outcome."""
    workflow = _load_workflow_mapping(workflow_text)
    publish = _get_job_dict(_get_job_dict(workflow, "jobs"), "publish")
    steps = publish["steps"]
    gate = _find_step_by_name(steps, "Check forced rebuild before publication")
    publisher = _find_step_by_name(steps, "Republish the rolling release in place")
    assert gate is not None and publisher is not None
    assert steps.index(gate) < steps.index(publisher)
    assert gate["run"] == "bash scripts/check-forced-rolling-assets.sh"
    assert gate["env"]["LINT_BUILD_RESULT"] == "${{ needs.build-lints.result }}"
    assert gate["env"]["DEPENDENCY_BUILD_RESULT"] == "${{ needs.build-dependency-binaries.result }}"
    assert gate["env"]["FORCE_DEPENDENCY_BINARY_REBUILD"] == (
        "${{ github.event.inputs.force_dependency_binary_rebuild }}"
    )
    assert _github_expression_mentions_operand(
        publisher["if"], "steps.forced_rebuild.outcome"
    )
    assert "steps.forced_rebuild.outcome == 'success'" in publisher["if"]


def test_gate_covers_both_workflow_matrices(workflow_text: str) -> None:
    """Removing one target from the gate or either matrix breaks the contract."""
    workflow = _load_workflow_mapping(workflow_text)
    jobs = _get_job_dict(workflow, "jobs")
    for name in ("build-lints", "build-dependency-binaries"):
        job = _get_job_dict(jobs, name)
        targets = tuple(entry["target"] for entry in job["strategy"]["matrix"]["include"])
        assert targets == TARGETS, f"{name} targets drifted from the forced gate"
    script = GATE.read_text(encoding="utf-8")
    match = re.search(r"(?ms)^targets=\(\n(.*?)^\)", script)
    assert match is not None, "forced gate must declare its target list"
    gate_targets = tuple(line.strip() for line in match.group(1).splitlines())
    assert gate_targets == TARGETS, "forced gate omits a workflow matrix target"
