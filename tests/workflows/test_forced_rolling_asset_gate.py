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


ASSET_REMOVAL_CASES = (
    *(("lint", target, None, None) for target in TARGETS),
    *(("manifest", target, None, None) for target in TARGETS),
    *(
        (kind, target, package, version)
        for target in TARGETS
        for package, version in _dependencies()
        for kind in ("archive", "checksum")
    ),
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
                f"{digest}  dist/{archive.name}\n", encoding="utf-8"
            )
    return dist


def test_complete_forced_rebuild_passes(complete_dist: Path) -> None:
    """A complete matrix output can reach the publisher."""
    result = _run_gate(complete_dist)
    assert result.returncode == 0, result.stderr


def test_binary_checksum_marker_passes(complete_dist: Path) -> None:
    """The Windows binary checksum marker approves its matching archive."""
    package, version = _dependencies()[0]
    archive = complete_dist / f"{package}-x86_64-pc-windows-msvc-v{version}.zip"
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (complete_dist / f"{archive.name}.sha256").write_text(
        f"{digest.upper()} *dist/{archive.name}\n", encoding="utf-8"
    )
    result = _run_gate(complete_dist)
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("failed_matrix", ["LINT_BUILD_RESULT", "DEPENDENCY_BUILD_RESULT"])
def test_failed_matrix_blocks_forced_publication(
    complete_dist: Path, failed_matrix: str
) -> None:
    """Either failed matrix prevents mutation even with complete artefacts."""
    result = _run_gate(complete_dist, **{failed_matrix: "failure"})
    assert result.returncode != 0, (
        f"{failed_matrix} failure must block forced publication:\n{result.stderr}"
    )
    assert "requires both complete build matrices" in result.stderr, (
        "the gate must identify an incomplete build matrix"
    )


@pytest.mark.parametrize(
    "case", ASSET_REMOVAL_CASES
)
def test_missing_forced_asset_blocks_publication(
    complete_dist: Path,
    case: tuple[str, str, str | None, str | None],
) -> None:
    """Every required target and dependency asset blocks forced publication."""
    missing_kind, target, package, version = case
    if missing_kind == "lint":
        missing_path = next(complete_dist.glob(f"whitaker-lints-*-{target}.tar.zst"))
    elif missing_kind == "manifest":
        missing_path = complete_dist / f"manifest-{target}.json"
    else:
        assert package is not None and version is not None, (
            "dependency asset cases require a package and version"
        )
        extension = "zip" if "windows" in target else "tgz"
        archive_name = f"{package}-{target}-v{version}.{extension}"
        if missing_kind == "archive":
            missing_path = complete_dist / archive_name
        else:
            missing_path = complete_dist / f"{archive_name}.sha256"

    missing_name = missing_path.name
    missing_path.unlink()
    result = _run_gate(complete_dist)
    assert result.returncode != 0, (
        f"missing {missing_name} must block forced publication:\n{result.stderr}"
    )
    assert missing_name in result.stderr, (
        "the gate must identify the missing asset"
    )


@pytest.mark.parametrize("target", TARGETS)
def test_empty_lint_archive_blocks_forced_publication(
    complete_dist: Path, target: str
) -> None:
    """An empty lint archive is not a complete forced-rebuild output."""
    lint_archive = next(complete_dist.glob(f"whitaker-lints-*-{target}.tar.zst"))
    lint_archive.write_bytes(b"")
    result = _run_gate(complete_dist)
    assert result.returncode != 0, (
        f"empty {lint_archive.name} must block forced publication:\n{result.stderr}"
    )
    assert lint_archive.name in result.stderr, "the gate must identify the empty asset"


def test_wrong_checksum_blocks_forced_publication(complete_dist: Path) -> None:
    """A present sidecar cannot approve an archive with different bytes."""
    package, version = _dependencies()[0]
    archive = complete_dist / f"{package}-x86_64-unknown-linux-gnu-v{version}.tgz"
    archive.write_bytes(b"changed")
    result = _run_gate(complete_dist)
    assert result.returncode != 0, (
        f"a mismatched checksum must block forced publication:\n{result.stderr}"
    )
    assert "Checksum or filename does not match" in result.stderr, (
        "the gate must identify a mismatched checksum"
    )


@pytest.mark.parametrize(
    "sidecar_case", ["different_path", "malformed", "trailing_content"]
)
def test_invalid_sidecar_blocks_forced_publication(
    complete_dist: Path, sidecar_case: str
) -> None:
    """A sidecar must be one GNU record for the expected archive path."""
    package, version = _dependencies()[0]
    archive = complete_dist / f"{package}-x86_64-unknown-linux-gnu-v{version}.tgz"
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if sidecar_case == "different_path":
        other = complete_dist / "unrelated"
        other.write_bytes(b"unrelated")
        other_digest = hashlib.sha256(other.read_bytes()).hexdigest()
        sidecar_contents = f"{other_digest}  dist/{other.name}\n"
    elif sidecar_case == "malformed":
        sidecar_contents = f"{digest} dist/{archive.name}\n"
    else:
        sidecar_contents = f"{digest}  dist/{archive.name}\nunexpected\n"
    (complete_dist / f"{archive.name}.sha256").write_text(
        sidecar_contents, encoding="utf-8"
    )
    result = _run_gate(complete_dist)
    assert result.returncode != 0, (
        f"a {sidecar_case} sidecar must block publication:\n{result.stderr}"
    )
    assert archive.name in result.stderr, "the gate must identify the expected archive"


def test_push_keeps_partial_publication_policy(tmp_path: Path) -> None:
    """The push path does not demand complete forced-rebuild artefacts."""
    result = _run_gate(
        tmp_path / "missing", GITHUB_EVENT_NAME="push", LINT_BUILD_RESULT="failure"
    )
    assert result.returncode == 0, result.stderr


def test_unforced_dispatch_keeps_partial_publication_policy(tmp_path: Path) -> None:
    """An unforced manual republish does not require complete rebuild outputs."""
    result = _run_gate(
        tmp_path / "missing",
        FORCE_DEPENDENCY_BINARY_REBUILD="false",
        LINT_BUILD_RESULT="failure",
        DEPENDENCY_BUILD_RESULT="failure",
    )
    assert result.returncode == 0, result.stderr


def test_workflow_runs_gate_before_publisher(workflow_text: str) -> None:
    """The publisher cannot run without the gate's successful outcome."""
    workflow = _load_workflow_mapping(workflow_text)
    publish = _get_job_dict(_get_job_dict(workflow, "jobs"), "publish")
    steps = publish["steps"]
    gate = _find_step_by_name(steps, "Check forced rebuild before publication")
    publisher = _find_step_by_name(steps, "Republish the rolling release in place")
    assert gate is not None and publisher is not None, (
        "publish must include the forced-rebuild gate and release step"
    )
    assert steps.index(gate) < steps.index(publisher), (
        "the forced-rebuild gate must run before the publisher"
    )
    assert gate["run"] == "bash scripts/check-forced-rolling-assets.sh", (
        "the workflow must invoke the forced-rebuild gate script"
    )
    assert gate["env"]["LINT_BUILD_RESULT"] == "${{ needs.build-lints.result }}", (
        "the gate must receive the lint matrix result"
    )
    assert gate["env"]["DEPENDENCY_BUILD_RESULT"] == "${{ needs.build-dependency-binaries.result }}", (
        "the gate must receive the dependency matrix result"
    )
    assert gate["env"]["FORCE_DEPENDENCY_BINARY_REBUILD"] == (
        "${{ github.event.inputs.force_dependency_binary_rebuild }}"
    )
    assert _github_expression_mentions_operand(
        publisher["if"], "steps.forced_rebuild.outcome"
    ), "the publisher must depend on the forced-rebuild gate"
    assert "steps.forced_rebuild.outcome == 'success'" in publisher["if"], (
        "the publisher must require a successful forced-rebuild gate"
    )


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
