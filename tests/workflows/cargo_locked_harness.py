"""Run Makefile targets against stubbed tools and inspect what they invoked.

The lock-mode contract tests need the real recipe text and the real `make`
expansion, but not a real toolchain. This module supplies the scratch
workspace, the process environment, and the assertions shared by those tests.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from dataclasses import dataclass
from itertools import takewhile
from pathlib import Path

import pytest

from tests.workflows.cargo_locked_stubs import _write_stub
from tests.workflows.clippy_snapshot_harness import provide_clippy_snapshot

REPO_ROOT = Path(__file__).resolve().parents[2]

# A lock-relevant Cargo call: `$(CARGO)` (optionally `+<toolchain>`) followed by
# a dependency-fetch, metadata, build, test, package, or run subcommand. `dylint list`
# and similar read-only calls are excluded because they do not resolve the
# dependency graph.
_LOCK_RELEVANT_CARGO = re.compile(
    r"(?:\$\(CARGO\)(?:\s+\+\S+)?\s+"
    r"(?:build|fetch|metadata|package|run|test)|"
    r"\$\((?:TEST_RUNNER|CORE_TEST_RUNNER)\))"
)


def _makefile_recipe_lines(target: str) -> list[str]:
    """Return the tab-indented command lines of a Makefile target's recipe."""
    lines = (REPO_ROOT / "Makefile").read_text(encoding="utf-8").splitlines()
    recipe_start = next(
        (
            index + 1
            for index, line in enumerate(lines)
            if re.match(rf"{re.escape(target)}:", line)
        ),
        len(lines),
    )
    return list(takewhile(lambda line: line.startswith("\t"), lines[recipe_start:]))


def _cargo_invocation(recipe_line: str) -> str:
    """Return the Cargo or test-runner call, trimmed at its terminator.

    Isolating the invocation from any environment prefix or later command on the
    same line means a `$(CARGO_LOCKED)` sitting outside this Cargo call is not
    mistaken for a flag threaded into it. Match `$(CARGO)` exactly so the
    `$(CARGO_LOCKED)` variable never becomes a false start, and check both
    runner variables because `make test` runs a workspace pass and a
    feature-free core pass.
    """
    for variable in ("$(TEST_RUNNER)", "$(CORE_TEST_RUNNER)"):
        if variable in recipe_line:
            call_start = recipe_line.index(variable)
            break
    else:
        call_start = recipe_line.index("$(CARGO)")
    invocation = recipe_line[call_start:]
    for terminator in (";", "&&"):
        invocation = invocation.split(terminator, 1)[0]
    return invocation


@dataclass(frozen=True)
class MakeRunEnvironment:
    """Inputs shared by one stubbed Make invocation."""

    cargo: Path
    locked: str | None
    stub_dir: Path


# Targets whose recipes invoke `uv` directly or through a repository script.
_TARGETS_REQUIRING_UV = frozenset({"lint", "release-installer-dry-run"})


def _stub_path(target: str, stub_dir: Path) -> str:
    """Return the PATH a stubbed `make` run should see for one target."""
    entries = [str(stub_dir)]
    if target in _TARGETS_REQUIRING_UV:
        uv = shutil.which("uv")
        if uv is None:  # pragma: no cover - environment guard
            pytest.skip(f"uv is required to run the {target} recipe")
        entries.append(str(Path(uv).parent))
    entries.extend(["/usr/bin", "/bin"])
    return ":".join(entries)


def _make_log_name(environment: MakeRunEnvironment) -> str:
    """Return the stable suffix used for one lock-mode run's logs."""
    if environment.locked is None:
        return "default"
    return environment.locked or "unlocked"


def _prepare_workspace(environment: MakeRunEnvironment) -> Path:
    """Create the scratch workspace holding the Makefile, scripts, and stubs."""
    workspace = environment.stub_dir.parent / "workspace"
    scripts_directory = workspace / "scripts"
    scripts_directory.mkdir(parents=True, exist_ok=True)
    for source, destination in (
        (REPO_ROOT / "Makefile", workspace / "Makefile"),
        (
            REPO_ROOT / "scripts/generate_checksums.py",
            scripts_directory / "generate_checksums.py",
        ),
        (
            REPO_ROOT / "scripts/check_skill_metadata.py",
            scripts_directory / "check_skill_metadata.py",
        ),
        # `publish-check` reads `rust-toolchain.toml` for the channel and shells
        # out to `scripts/install-dylint-tools.sh`; provide the manifest and a
        # no-op stub so the recipe runs without a real toolchain or network
        # access. Both are inert for the other targets.
        (REPO_ROOT / "rust-toolchain.toml", workspace / "rust-toolchain.toml"),
    ):
        shutil.copy2(source, destination)
    _write_stub(scripts_directory, "install-dylint-tools.sh", "exit 0")
    return workspace


def _log_path(target: str, environment: MakeRunEnvironment) -> Path:
    """Return the path where one stubbed run records its Cargo calls."""
    return environment.stub_dir / f"{target}-{_make_log_name(environment)}.log"


def _process_environment(
    target: str,
    environment: MakeRunEnvironment,
    workspace: Path,
    extra_env: dict[str, str] | None,
) -> dict[str, str]:
    """Return the environment for one stubbed `make` run.

    `extra_env` is applied after the defaults are cleared, so a test can hand
    the recipe an inherited value the harness would otherwise remove.
    """
    log = _log_path(target, environment)
    # Keep the driver inside the scratch workspace. The Makefile would
    # otherwise default it into the real user cache, and an inherited value
    # from an outer `make` could be written to.
    process_env = os.environ | {
        "CARGO_LOCKED_LOG": str(log),
        "CARGO_LOCKED_ENV_LOG": str(log.with_suffix(".env")),
        "DYLINT_DRIVER_PATH": str(workspace / "dylint-drivers" / "test-driver"),
        "PATH": _stub_path(target, environment.stub_dir),
    }
    for name in ("CARGO_LOCKED", "CARGO_NET_OFFLINE", "RUSTUP_TOOLCHAIN", "BASH_ENV"):
        process_env.pop(name, None)
    if extra_env:
        process_env.update(extra_env)
    return process_env


def _target_arguments(
    target: str, environment: MakeRunEnvironment, workspace: Path
) -> list[str]:
    """Return the Make variable overrides one target's recipe needs."""
    if target == "test":
        return [
            f"CARGO_NEXTEST={environment.stub_dir / 'cargo-nextest'}",
            f"CARGO_TEST_WRAPPER={environment.cargo}",
            f"CARGO_TEST_METADATA_ROOT={workspace}",
            "TEST_RUSTUP_TOOLCHAIN=nightly-test",
        ]
    if target == "lint":
        # This test isolates Cargo flag forwarding; the isolated fixture does
        # not contain the repository's skills needed by the lint prerequisite.
        return ["-o", "skill-manifest-check"]
    return []


def _make_arguments(
    target: str,
    environment: MakeRunEnvironment,
    workspace: Path,
    extra_make_args: list[str] | None,
) -> list[str]:
    """Return the `make` command line for one stubbed run."""
    arguments = [
        "make",
        target,
        f"CARGO={environment.cargo}",
        *provide_clippy_snapshot(workspace),
    ]
    # Command-line PATH keeps Make's HOME-based prefix from shadowing stubs.
    arguments.append(f"PATH={_stub_path(target, environment.stub_dir)}")
    arguments.extend(_target_arguments(target, environment, workspace))
    if environment.locked is not None:
        arguments.append(f"CARGO_LOCKED={environment.locked}")
    if extra_make_args:
        arguments.extend(extra_make_args)
    return arguments


def _run_make(
    target: str,
    environment: MakeRunEnvironment,
    extra_make_args: list[str] | None = None,
    extra_env: dict[str, str] | None = None,
) -> list[str]:
    """Run one target with stubbed tools and return recorded Cargo arguments."""
    workspace = _prepare_workspace(environment)
    if target == "lint":
        # `lint` runs `interrogate` through `uv` and the installed Whitaker
        # suite. This contract tests Cargo flags, and the scratch workspace
        # carries neither Python sources to inspect nor a bootstrapped suite.
        # Only `lint` is stubbed: `release-installer-dry-run` needs the real
        # `uv` to execute `scripts/generate_checksums.py`.
        _write_stub(environment.stub_dir, "uv", "exit 0")
        _write_stub(environment.stub_dir, "whitaker", "exit 0")
    log = _log_path(target, environment)
    log_name = _make_log_name(environment)
    process_env = _process_environment(target, environment, workspace, extra_env)
    make_arguments = _make_arguments(target, environment, workspace, extra_make_args)
    result = subprocess.run(
        make_arguments,
        cwd=workspace,
        capture_output=True,
        text=True,
        check=False,
        env=process_env,
    )
    assert result.returncode == 0, (
        f"{target} exited {result.returncode}\n"
        f"stdout: {result.stdout}\nstderr: {result.stderr}"
    )
    assert log.exists(), (
        f"{target} succeeded without recording any Cargo calls to the "
        f"{log_name} log at {log}\n"
        f"stdout: {result.stdout}\nstderr: {result.stderr}"
    )
    return log.read_text(encoding="utf-8").splitlines()


def _cargo_head(invocation: str) -> str:
    """The Cargo subcommand from a recorded `$@`, skipping a leading +toolchain."""
    tokens = invocation.split()
    if tokens and tokens[0].startswith("+"):
        tokens = tokens[1:]
    if tokens[:1] == ["nextest"]:
        return "nextest run"
    return tokens[0] if tokens else ""


def _is_lock_relevant_invocation(invocation: str) -> bool:
    """Whether a recorded Cargo call resolves the dependency graph (needs --locked)."""
    return _cargo_head(invocation) in {
        "build",
        "fetch",
        "metadata",
        "nextest run",
        "package",
        "run",
        "test",
    }


def _has_locked_flag(invocation: str) -> bool:
    """Whether a recorded `$@` carries `--locked` as an exact argument token."""
    # Tokenize before matching so a flag such as `--locked-extra` does not
    # satisfy the check the way a bare `"--locked" in invocation` substring would.
    return "--locked" in invocation.split()


def _assert_make_test_invocations(
    cargo_calls: list[str],
    recorded: list[str],
    *,
    should_use_locked: bool,
    log_name: str,
) -> None:
    """Check that preparation precedes every test command and honours the lock mode."""
    assert [_cargo_head(line) for line in cargo_calls] == [
        "run",
        "fetch",
        "run",
        "metadata",
        "metadata",
        "build",
        "nextest run",
        "nextest run",
        "test",
        "nextest run",
        "nextest run",
    ], f"make test must prepare before running its test commands; recorded: {recorded!r}"
    snapshot_call, fetch_call, warm_up_call, *prepared = cargo_calls
    assert "whitaker-clippy-snapshot" in snapshot_call, (
        "the snapshot install must precede the fetch it depends on"
    )
    assert fetch_call.split()[:1] == ["fetch"], (
        f"preparation must fetch dependencies; recorded: {fetch_call!r}"
    )
    assert "warm_driver" in warm_up_call, (
        f"preparation must warm the Dylint driver; recorded: {warm_up_call!r}"
    )
    metadata_calls = [line for line in prepared if _cargo_head(line) == "metadata"]
    assert len(metadata_calls) == 2, (
        f"preparation must write both metadata files; recorded: {metadata_calls!r}"
    )
    assert sum("--no-deps" in line.split() for line in metadata_calls) == 1, (
        "the two metadata files must come from distinct queries"
    )
    driver_build = next(line for line in prepared if _cargo_head(line) == "build")
    assert "cargo_test_driver" in driver_build, (
        f"preparation must build the test driver; recorded: {driver_build!r}"
    )
    nextest_call = next(line for line in cargo_calls if _cargo_head(line) == "nextest run")
    assert "--cargo-metadata" in nextest_call, (
        "Nextest must consume a prepared metadata file"
    )
    assert "nextest-cargo-metadata.json" in nextest_call, (
        "Nextest must use metadata produced during preparation"
    )
    for line in cargo_calls:
        assert _has_locked_flag(line) is should_use_locked, (
            f"make test must use {log_name} mode; invocation: {line!r}"
        )


def _assert_make_test_environments(
    environment_log: Path,
    *,
    cargo: Path,
    expected_driver_path: str,
) -> None:
    """Check the offline, toolchain, and driver environment of each recipe."""
    recorded_environments = [
        line.split("\t", maxsplit=5) for line in environment_log.read_text().splitlines()
    ]
    # The snapshot install (`run`) precedes the fetch preparation, which
    # precedes the three test commands and the two nested test targets. Only
    # those five run offline.
    assert [record[0] for record in recorded_environments] == [
        "<unset>",
        "<unset>",
        "<unset>",
        "<unset>",
        "<unset>",
        "<unset>",
        "true",
        "true",
        "true",
        "true",
        "true",
    ], "preparation stays online while every test Cargo command runs offline"
    assert [record[5] for record in recorded_environments] == [
        "<unset>",
        "<unset>",
        "<unset>",
        "<unset>",
        "<unset>",
        "<unset>",
        "nightly-test",
        "nightly-test",
        "nightly-test",
        "<unset>",
        "<unset>",
    ], "test processes need the active Rustup toolchain environment"
    # The warm-up and both test commands share the keyed driver directory the
    # Makefile exports, so the driver is built once and then reused.
    assert [record[3] for record in recorded_environments] == [expected_driver_path] * 11, (
        "every recipe must see the same Dylint driver directory"
    )
    for record in recorded_environments[6:]:
        assert record[1] == str(cargo), "tests must inherit the selected Cargo executable"

