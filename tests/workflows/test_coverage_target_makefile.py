"""Verify that the coverage recipe propagates its Cargo target boundary."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

from tests.workflows.clippy_snapshot_harness import provide_clippy_snapshot

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
    snapshot_arguments = provide_clippy_snapshot(workspace)
    cargo = bin_directory / "cargo"
    _write_coverage_environment_stubs(bin_directory)

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
            f"CARGO_TEST_WRAPPER={cargo}",
            f"CARGO_LLVM_COV={bin_directory / 'cargo-llvm-cov'}",
            f"CARGO_TEST_METADATA_ROOT={workspace}",
            "TEST_RUSTUP_TOOLCHAIN=nightly-test",
            f"WHITAKER_SCRIPT={tmp_path / 'whitaker'}",
            *snapshot_arguments,
        ],
        cwd=workspace,
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    _assert_coverage_environment(environment_log, workspace, bin_directory)


def _write_coverage_environment_stubs(bin_directory: Path) -> None:
    """Write Cargo, cargo-llvm-cov, and Nextest stubs that log their environment."""
    cargo = bin_directory / "cargo"
    _write_executable(
        cargo,
        'case "$1" in\n'
        '    fetch|run|build)\n'
        '        printf "preparation|%s|%s|%s\\n" "$1" "${CARGO_NET_OFFLINE-<unset>}" '
        '"${CARGO_TARGET_DIR-<unset>}" >> "$COVERAGE_ENV_LOG"\n'
        '        ;;\n'
        '    metadata)\n'
        '        if [ -n "${CARGO_METADATA_CACHE:-}" ]; then\n'
        '            printf "test|%s|%s|%s|%s|false|%s\\n" "$1" "${CARGO_NET_OFFLINE-<unset>}" '
        '"${CARGO-<unset>}" "$CARGO_METADATA_CACHE" "${RUSTUP_TOOLCHAIN-<unset>}" '
        '>> "$COVERAGE_ENV_LOG"\n'
        '        else\n'
        '            printf "preparation|%s|%s|%s\\n" "$1" "${CARGO_NET_OFFLINE-<unset>}" '
        '"${CARGO_TARGET_DIR-<unset>}" >> "$COVERAGE_ENV_LOG"\n'
        '        fi\n'
        '        ;;\n'
        '    nextest|test)\n'
        '        metadata_argument=false\n'
        '        for argument in "$@"; do [ "$argument" != "--cargo-metadata" ] || metadata_argument=true; done\n'
        '        printf "test|%s|%s|%s|%s|%s|%s\\n" "$1" "${CARGO_NET_OFFLINE-<unset>}" '
        '"${CARGO-<unset>}" "${CARGO_METADATA_CACHE-<unset>}" '
        '"$metadata_argument" "${RUSTUP_TOOLCHAIN-<unset>}" '
        '>> "$COVERAGE_ENV_LOG"\n'
        '        ;;\n'
        'esac',
    )
    _write_executable(
        bin_directory / "cargo-llvm-cov",
        'case "$1" in\n'
        '    llvm-cov)\n'
        '        printf "outer|%s|%s|%s\\n" "$CARGO_LLVM_COV_TARGET_DIR" "$CARGO_TARGET_DIR" '
        '"$LLVM_PROFILE_FILE_NAME" >> "$COVERAGE_ENV_LOG"\n'
        '        shift 2\n'
        '        case " $* " in *" -- "*) echo "unexpected test-binary separator" >&2; exit 1;; esac\n'
        '        printf "runner|%s\\n" "$*" >> "$COVERAGE_ENV_LOG"\n'
        '        "$CARGO" metadata --no-deps\n'
        '        "$CARGO" nextest run "$@"\n'
        '        ;;\n'
        'esac',
    )
    _write_executable(bin_directory / "cargo-nextest", "exit 0")


def _assert_coverage_environment(
    environment_log: Path,
    workspace: Path,
    bin_directory: Path,
) -> None:
    """Check the recorded runner arguments and per-recipe environments."""
    cargo = bin_directory / "cargo"
    expected_target = workspace / "target" / "llvm-cov-target"
    recorded = environment_log.read_text(encoding="utf-8").splitlines()
    runner_arguments = [
        line.removeprefix("runner|") for line in recorded if line.startswith("runner|")
    ]
    assert len(runner_arguments) == 2, (
        "coverage must instrument the workspace pass and the feature-free core "
        f"pass; got: {runner_arguments}"
    )
    for arguments in runner_arguments:
        assert " -- " not in f" {arguments} ", (
            "cargo-llvm-cov must receive Nextest options before any test-binary separator"
        )
        assert "--no-report" in arguments, (
            "both instrumented passes must defer reporting so one report composes them"
        )
        assert "--locked" in arguments, (
            "coverage must keep the locked flags of the plain test run"
        )
        assert "--cargo-metadata " in arguments, (
            "cargo-llvm-cov must forward Nextest's prepared metadata option"
        )
    assert "--locked --workspace" in runner_arguments[0], (
        "the workspace pass must keep the all-feature crate subset"
    )
    assert "-p whitaker_lint_core --no-default-features" in runner_arguments[1], (
        "the second pass must stay inside the feature-free core package"
    )
    expected_filter = (
        f"{_makefile_variable('COVERAGE_BASE_FILTER')} & "
        f"not ({_makefile_variable('COVERAGE_SKIPPED_TRYBUILD_FILTER')})"
    )
    assert f"-E {expected_filter}" in runner_arguments[0], (
        f"coverage must pass the composed filter; got: {runner_arguments[0]}"
    )
    assert "-E " not in runner_arguments[1], (
        "only the all-feature pass narrows the crate subset; the core pass runs "
        "its own package"
    )
    assert (
        "llvm-cov report --lcov --output-path $(COVERAGE_OUTPUT)"
        in (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    ), "coverage must write the composed LCOV report to the repository root"
    assert [line for line in recorded if not line.startswith("runner|")] == [
        f"preparation|run|<unset>|{workspace / 'target/clippy-snapshot-tool'}",
        f"preparation|fetch|<unset>|{expected_target}",
        f"preparation|run|<unset>|{workspace / 'target/dylint-warmup'}",
        f"preparation|metadata|<unset>|{expected_target}",
        f"preparation|metadata|<unset>|{expected_target}",
        f"preparation|build|<unset>|{workspace / 'target/dylint-warmup'}",
        f"outer|{expected_target}|{expected_target}|whitaker-%m.profraw",
        f"test|metadata|true|{cargo}|{workspace / 'target/dylint-warmup/llvm-cov-cargo-metadata.json'}|false|nightly-test",
        f"test|nextest|true|{cargo}|{workspace / 'target/dylint-warmup/llvm-cov-cargo-metadata.json'}|true|nightly-test",
        f"outer|{expected_target}|{expected_target}|whitaker-%m.profraw",
        f"test|metadata|true|{cargo}|{workspace / 'target/dylint-warmup/llvm-cov-cargo-metadata.json'}|false|nightly-test",
        f"test|nextest|true|{cargo}|{workspace / 'target/dylint-warmup/llvm-cov-cargo-metadata.json'}|true|nightly-test",
        f"test|test|true|{cargo}|<unset>|false|nightly-test",
        f"test|nextest|true|{cargo}|<unset>|false|<unset>",
        f"test|nextest|true|{cargo}|<unset>|false|<unset>",
    ], "preparation must stay online before instrumented and nested Cargo run offline"


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
    snapshot_arguments = provide_clippy_snapshot(workspace)
    arguments_log = tmp_path / "cargo-arguments.log"
    cargo = bin_directory / "cargo"
    _write_executable(cargo, 'printf "cargo|%s\\n" "$*" >> "$CARGO_ARGUMENTS_LOG"')
    _write_executable(
        bin_directory / "cargo-llvm-cov",
        'printf "runner|%s\\n" "$*" >> "$CARGO_ARGUMENTS_LOG"',
    )
    _write_executable(
        bin_directory / "cargo-nextest",
        'printf "runner|%s\\n" "$*" >> "$CARGO_ARGUMENTS_LOG"',
    )
    result = subprocess.run(
        [
            "make",
            *targets,
            f"CARGO={cargo}",
*snapshot_arguments,
            f"CARGO_TEST_WRAPPER={cargo}",
            f"CARGO_LLVM_COV={bin_directory / 'cargo-llvm-cov'}",
            f"CARGO_NEXTEST={bin_directory / 'cargo-nextest'}",
            f"WHITAKER_SCRIPT={tmp_path / 'whitaker'}",
        ],
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
    filtered = [
        a for a in _cargo_arguments(tmp_path, "coverage") if "llvm-cov" in a and " -E " in a
    ]
    assert len(filtered) == 1, (
        f"only the all-feature coverage pass may narrow the crate subset; got: {filtered}"
    )
    assert f" -E {expected}" in filtered[0], (
        "make coverage must pass the base filter less the skipped trybuild "
        f"contracts to nextest; got: {filtered[0]}"
    )


def test_make_test_runs_every_test_without_a_filter(tmp_path: Path) -> None:
    """``make test`` passes no ``-E``, so the contract tests still run in it."""
    lines = [a for a in _cargo_arguments(tmp_path, "test") if "nextest" in a]
    assert lines, "make test ran no nextest invocation"
    unfiltered = [a for a in lines if " -E " not in a]
    assert unfiltered == lines, f"make test must not filter tests; got: {lines}"


def test_contracts_target_runs_all_eight_under_the_ci_profile(tmp_path: Path) -> None:
    """``make trybuild-contracts`` filters both passes to the eight contract tests.

    The target runs the workspace pass and then the feature-free
    ``whitaker_lint_core`` pass. Both must carry the contract filter, otherwise
    the core pass would run that crate's whole suite; only the core pass needs
    ``--no-tests=pass``, because no contract test lives there today.
    """
    expected = _makefile_variable("TRYBUILD_CONTRACT_FILTER")
    lines = [a for a in _cargo_arguments(tmp_path, "trybuild-contracts") if "nextest" in a]
    filtered = [a for a in lines if " -E " in a]
    assert len(filtered) == 2, (
        f"expected the workspace and core passes to share the filter; got: {filtered}"
    )
    for line in filtered:
        assert f" -E {expected}" in line, f"expected the eight contract tests; got: {line}"
        assert "--profile ci" in line, f"expected the ci profile; got: {line}"
    tolerated_emptiness = [a for a in filtered if "--no-tests=pass" in a]
    assert len(tolerated_emptiness) == 1, (
        f"only the core pass may accept an empty filter match; got: {filtered}"
    )
    assert all("llvm-cov" not in a for a in lines), (
        f"expected no instrumentation; got: {lines}"
    )
