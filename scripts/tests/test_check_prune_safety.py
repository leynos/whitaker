"""Test the prune-safety checker that guards the rolling-release publish.

Run with ``make test-prune-safety``.
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
_SPEC = importlib.util.spec_from_file_location(
    "check_prune_safety", SCRIPTS / "check_prune_safety.py"
)
assert _SPEC is not None
assert _SPEC.loader is not None
checker = importlib.util.module_from_spec(_SPEC)
sys.modules["check_prune_safety"] = checker
_SPEC.loader.exec_module(checker)

OLD_CRATES = ["alpha", "beta"]
TARGET = "x86_64-unknown-linux-gnu"
ARCHIVE = f"whitaker-lints-abc1234-nightly-2026-05-28-{TARGET}.tar.zst"


def _git(repository: Path, *arguments: str) -> None:
    """Run git in ``repository``, failing the test on any error."""
    subprocess.run(["git", "-C", str(repository), *arguments], check=True, capture_output=True)


@pytest.fixture
def repository(tmp_path: Path) -> Path:
    """Create a repository whose ``v0.0.1`` tag lists ``OLD_CRATES``."""
    root = tmp_path / "repo"
    (root / "installer/src").mkdir(parents=True)
    names = ", ".join(f'"{name}"' for name in OLD_CRATES)
    (root / "installer/src/resolution.rs").write_text(
        f"pub const LINT_CRATES: &[&str] = &[{names}];\n", encoding="utf-8"
    )
    _git(root, "init", "-q")
    _git(root, "add", "-A")
    _git(
        root,
        "-c", "user.name=t", "-c", "user.email=t@example.invalid",
        "commit", "-q", "-m", "tag",
    )
    _git(root, "tag", "v0.0.1")
    return root


def _publish(directory: Path, libraries: list[str], manifest: list[str] | None = None) -> None:
    """Write an archive holding ``libraries`` and its sidecar manifest."""
    directory.mkdir(exist_ok=True)
    staging = directory / "staging"
    staging.mkdir()
    for crate in libraries:
        (staging / f"lib{crate}.so").write_bytes(b"x")
    subprocess.run(
        ["tar", "--zstd", "-cf", str(directory / ARCHIVE), "-C", str(staging), "."],
        check=True,
    )
    for crate in libraries:
        (staging / f"lib{crate}.so").unlink()
    staging.rmdir()
    files = [f"lib{crate}.so" for crate in libraries] if manifest is None else manifest
    (directory / f"manifest-{TARGET}.json").write_text(
        json.dumps({"files": files}), encoding="utf-8"
    )


def test_suite_only_after_prune_passes(repository: Path, tmp_path: Path) -> None:
    """An archive of known crates plus the suite is safe."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    status = checker.main(["--installer-tag", "v0.0.1", "--repository", str(repository), str(tmp_path / "dist")])
    assert status == 0


def test_unknown_standalone_library_fails(
    repository: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The #565 shape: a crate the installer cannot prune survives and fails."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "newcomer", "whitaker_suite"])
    status = checker.main(["--installer-tag", "v0.0.1", "--repository", str(repository), str(tmp_path / "dist")])
    assert status == 1
    assert "newcomer" in capsys.readouterr().err


def test_archive_without_suite_fails(repository: Path, tmp_path: Path) -> None:
    """An archive that leaves nothing after the prune is also refused."""
    _publish(tmp_path / "dist", list(OLD_CRATES))
    status = checker.main(["--installer-tag", "v0.0.1", "--repository", str(repository), str(tmp_path / "dist")])
    assert status == 1


def test_manifest_disagreeing_with_archive_fails(
    repository: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """A manifest naming a library the archive lacks is refused."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"], manifest=["libalpha.so"])
    status = checker.main(["--installer-tag", "v0.0.1", "--repository", str(repository), str(tmp_path / "dist")])
    assert status == 1
    assert "manifest" in capsys.readouterr().err


def test_missing_tag_is_an_error_not_a_pass(repository: Path, tmp_path: Path) -> None:
    """Failing to read the installer's list must stop the publish."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    status = checker.main(["--installer-tag", "v9.9.9", "--repository", str(repository), str(tmp_path / "dist")])
    assert status == 2


def test_empty_directory_is_an_error(repository: Path, tmp_path: Path) -> None:
    """No archives at all is not a pass."""
    (tmp_path / "dist").mkdir()
    status = checker.main(["--installer-tag", "v0.0.1", "--repository", str(repository), str(tmp_path / "dist")])
    assert status == 2
