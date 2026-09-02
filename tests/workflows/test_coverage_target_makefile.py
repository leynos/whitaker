"""Verify that the coverage recipe propagates its Cargo target boundary."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]


def _write_executable(path: Path, body: str) -> None:
    """Write an executable shell stub at ``path``."""
    path.write_text(f"#!/bin/sh\nset -eu\n{body}\n", encoding="utf-8")
    path.chmod(0o755)


def test_coverage_shares_its_target_with_nested_cargo(tmp_path: Path) -> None:
    """Require coverage and nested Cargo to inherit one absolute target."""
    workspace = tmp_path / "workspace"
    bin_directory = tmp_path / "bin"
    workspace.mkdir()
    bin_directory.mkdir()
    shutil.copy2(REPOSITORY_ROOT / "Makefile", workspace / "Makefile")

    cargo = bin_directory / "cargo"
    _write_executable(
        cargo,
        'case "$1" in\n'
        '    llvm-cov)\n'
        '        printf "outer|%s|%s\\n" "$CARGO_LLVM_COV_TARGET_DIR" "$CARGO_TARGET_DIR" '
        '>> "$COVERAGE_ENV_LOG"\n'
        '        "$0" nested-cargo\n'
        '        ;;\n'
        '    nested-cargo)\n'
        '        printf "nested|%s|%s\\n" "$CARGO_LLVM_COV_TARGET_DIR" "$CARGO_TARGET_DIR" '
        '>> "$COVERAGE_ENV_LOG"\n'
        '        ;;\n'
        'esac',
    )
    _write_executable(bin_directory / "cargo-llvm-cov", "exit 0")
    _write_executable(bin_directory / "cargo-nextest", "exit 0")

    environment_log = tmp_path / "coverage-environment.log"
    environment = os.environ | {
        "COVERAGE_ENV_LOG": str(environment_log),
        "PATH": f"{bin_directory}:{os.environ['PATH']}",
    }
    result = subprocess.run(
        [
            "make",
            "coverage",
            f"CARGO={cargo}",
            f"WHITAKER_SCRIPT={tmp_path / 'whitaker'}",
        ],
        cwd=workspace,
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    expected_target = workspace / "target" / "llvm-cov-target"
    assert environment_log.read_text(encoding="utf-8").splitlines() == [
        f"outer|{expected_target}|{expected_target}",
        f"nested|{expected_target}|{expected_target}",
    ], "coverage and nested Cargo must use the same absolute target directory"
