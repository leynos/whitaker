"""Verify that the coverage recipe propagates its Cargo target boundary."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]


def _write_executable(path: Path, body: str) -> None:
    """Write an executable shell stub at ``path``."""
    path.write_text(f"#!/bin/sh\nset -eu\n{body}\n", encoding="utf-8")
    path.chmod(0o755)


def test_coverage_shares_target_and_profile_policy_with_nested_cargo(
    tmp_path: Path,
) -> None:
    """Require coverage and nested Cargo to inherit the shared policy."""
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
        '        printf "outer|%s|%s|%s\\n" "$CARGO_LLVM_COV_TARGET_DIR" "$CARGO_TARGET_DIR" '
        '"$LLVM_PROFILE_FILE_NAME" '
        '>> "$COVERAGE_ENV_LOG"\n'
        '        "$0" nested-cargo\n'
        '        ;;\n'
        '    nested-cargo)\n'
        '        printf "nested|%s|%s|%s\\n" "$CARGO_LLVM_COV_TARGET_DIR" "$CARGO_TARGET_DIR" '
        '"$LLVM_PROFILE_FILE_NAME" '
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
        f"outer|{expected_target}|{expected_target}|whitaker-%m.profraw",
        f"nested|{expected_target}|{expected_target}|whitaker-%m.profraw",
    ], "coverage and nested Cargo must share the target and profile policy"


def _makefile_variable(name: str) -> str:
    """Return the value of a ``NAME ?= value`` assignment in the Makefile."""
    text = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    match = re.search(rf"^{name} \?= (.+)$", text, re.MULTILINE)
    assert match, f"Makefile has no {name} assignment"
    return match.group(1)


def _cargo_arguments(tmp_path: Path, *targets: str) -> list[str]:
    """Run ``make`` with a Cargo stub and return each Cargo invocation's arguments."""
    workspace = tmp_path / "workspace"
    bin_directory = tmp_path / "bin"
    workspace.mkdir()
    bin_directory.mkdir()
    shutil.copy2(REPOSITORY_ROOT / "Makefile", workspace / "Makefile")
    arguments_log = tmp_path / "cargo-arguments.log"
    cargo = bin_directory / "cargo"
    _write_executable(cargo, 'printf "%s\\n" "$*" >> "$CARGO_ARGUMENTS_LOG"')
    _write_executable(bin_directory / "cargo-llvm-cov", "exit 0")
    _write_executable(bin_directory / "cargo-nextest", "exit 0")
    result = subprocess.run(
        ["make", *targets, f"CARGO={cargo}", f"WHITAKER_SCRIPT={tmp_path / 'whitaker'}"],
        cwd=workspace,
        capture_output=True,
        check=False,
        env=os.environ
        | {
            "CARGO_ARGUMENTS_LOG": str(arguments_log),
            "PATH": f"{bin_directory}:{os.environ['PATH']}",
        },
        text=True,
    )
    assert result.returncode == 0, result.stderr
    return arguments_log.read_text(encoding="utf-8").splitlines()


def test_coverage_passes_nextest_the_base_filter_less_the_skipped_contracts(
    tmp_path: Path,
) -> None:
    """``make coverage`` hands ``cargo llvm-cov nextest`` the composed ``-E`` filter."""
    expected = (
        f"{_makefile_variable('COVERAGE_BASE_FILTER')} & "
        f"not ({_makefile_variable('COVERAGE_SKIPPED_TRYBUILD_FILTER')})"
    )
    (line,) = [a for a in _cargo_arguments(tmp_path, "coverage") if "llvm-cov" in a]
    assert f" -E {expected}" in line, (
        f"make coverage must pass the base filter less the skipped trybuild "
        f"contracts to nextest; got: {line}"
    )


def test_make_test_runs_every_test_without_a_filter(tmp_path: Path) -> None:
    """``make test`` passes no ``-E``, so the contract tests still run in it."""
    (line,) = [a for a in _cargo_arguments(tmp_path, "test") if "nextest" in a]
    assert " -E " not in line, f"make test must not filter tests; got: {line}"


def test_contracts_target_runs_all_seven_under_the_ci_profile(tmp_path: Path) -> None:
    """``make trybuild-contracts`` filters to the seven contract tests, uninstrumented."""
    expected = _makefile_variable("TRYBUILD_CONTRACT_FILTER")
    (line,) = [a for a in _cargo_arguments(tmp_path, "trybuild-contracts") if "nextest" in a]
    assert f" -E {expected}" in line, f"expected the seven contract tests; got: {line}"
    assert "--profile ci" in line, f"expected the ci profile; got: {line}"
    assert "llvm-cov" not in line, f"expected no instrumentation; got: {line}"
