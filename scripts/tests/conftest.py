"""Shared fixtures for the prune-safety checker tests."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from prune_safety_support import OLD_CRATES


def _git(repository: Path, *arguments: str) -> None:
    """Run git in ``repository``, failing the test on any error."""
    subprocess.run(
        ["git", "-C", str(repository), *arguments], check=True, capture_output=True
    )


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
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@example.invalid",
        "commit",
        "-q",
        "-m",
        "tag",
    )
    _git(root, "tag", "v0.0.1")
    return root
