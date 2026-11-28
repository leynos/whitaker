"""Shared fixtures and helpers for the prune-safety checker tests.

The checker and its rules module are loaded once here so both test modules
exercise the same module objects, and the archive fixtures stay in one place.
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
# The checker imports its sibling rules module by name, and an importlib load
# does not add the script's directory to sys.path the way direct execution
# does.
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

rules = importlib.import_module("prune_safety_rules")

_SPEC = importlib.util.spec_from_file_location(
    "check_prune_safety", SCRIPTS / "check_prune_safety.py"
)
assert _SPEC is not None, "the checker script must have an import spec"
assert _SPEC.loader is not None, "the checker script must have a loader"
checker = importlib.util.module_from_spec(_SPEC)
sys.modules["check_prune_safety"] = checker
_SPEC.loader.exec_module(checker)

OLD_CRATES = ["alpha", "beta"]
TARGET = "x86_64-unknown-linux-gnu"
ARCHIVE = f"whitaker-lints-abc1234-nightly-2026-05-28-{TARGET}.tar.zst"


def _publish(
    directory: Path,
    libraries: list[str],
    manifest: list[str] | None = None,
    toolchain: str = "nightly-2026-05-28",
) -> None:
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
        json.dumps(
            {
                "files": files,
                "toolchain": toolchain,
                "git_sha": "abc1234",
                "target": TARGET,
            }
        ),
        encoding="utf-8",
    )


def _check(repository: Path, directory: Path, tag: str = "v0.0.1") -> int:
    """Run the checker's entry point against ``directory`` and return its status."""
    return checker.main(
        ["--installer-tag", tag, "--repository", str(repository), str(directory)]
    )


def _check_snapshots(
    repository: Path, directory: Path, table: Path, tag: str = "v0.0.1"
) -> int:
    """Run the checker with a snapshot table and return its status."""
    return checker.main(
        [
            "--installer-tag",
            tag,
            "--repository",
            str(repository),
            "--snapshots-tsv",
            str(table),
            str(directory),
        ]
    )


def _table(tmp_path: Path, toolchain: str) -> Path:
    """Write a snapshot table mapping installer 0.0.1 to ``toolchain``."""
    table = tmp_path / "clippy-snapshots.tsv"
    table.write_text(f"# header\n0.0.1 abc 6.3.0 {toolchain}\n", encoding="utf-8")
    return table
