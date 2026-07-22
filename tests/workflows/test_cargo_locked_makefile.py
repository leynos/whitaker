"""Verify that Makefile Cargo invocations consistently honour CARGO_LOCKED."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from itertools import takewhile
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]

# A lock-relevant Cargo call: `$(CARGO)` (optionally `+<toolchain>`) followed by
# a build/test/package subcommand. `dylint list` and similar read-only calls are
# intentionally excluded because they do not resolve the dependency graph.
_LOCK_RELEVANT_CARGO = re.compile(
    r"\$\(CARGO\)(?:\s+\+\S+)?\s+(?:\$\(TEST_RUNNER\)|build|package|nextest\s+run)"
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


def test_recipe_lines_are_empty_for_an_absent_target() -> None:
    """An unknown Makefile target yields no recipe lines."""
    assert _makefile_recipe_lines("definitely-not-a-real-target") == []


@pytest.mark.parametrize("target", ["test", "publish-check"])
def test_recipe_cargo_calls_thread_cargo_locked(target: str) -> None:
    """Lock-relevant Cargo calls in the recipe forward `$(CARGO_LOCKED)`.

    `make test` and `publish-check` are not runnable under a stubbed toolchain
    (they resolve real toolchains, clone the repository, and install Dylint
    tooling), so this asserts the recipe text threads the lock flag through each
    build/test/package invocation instead.
    """
    lock_relevant = [
        line for line in _makefile_recipe_lines(target) if _LOCK_RELEVANT_CARGO.search(line)
    ]
    assert lock_relevant, f"the {target} recipe should invoke Cargo build/test/package"
    for line in lock_relevant:
        assert "$(CARGO_LOCKED)" in line, (
            f"{target} must thread $(CARGO_LOCKED) through Cargo call: {line.strip()!r}"
        )


def _write_stub(directory: Path, name: str, body: str) -> Path:
    """Write an executable shell stub and return its path."""
    path = directory / name
    path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    path.chmod(0o755)
    return path


def _write_cargo_stub(directory: Path) -> Path:
    """Write a Cargo stand-in that records metadata and build invocations."""
    return _write_stub(
        directory,
        "cargo",
        '''echo "$@" >> "$CARGO_LOCKED_LOG"
case "$1" in
metadata) echo '{{"packages":[{{"name":"whitaker-installer","version":"0.2.5"}}]}}' ;;
build)
    target=""
    for argument in "$@"; do
        if [ "$previous" = "--target" ]; then target="$argument"; fi
        previous="$argument"
    done
    mkdir -p "target/$target/release"
    : > "target/$target/release/whitaker-installer"
    cat > "target/$target/release/whitaker-package-installer" <<'EOF'
#!/bin/sh
for argument in "$@"; do
    if [ "$previous" = "--output-dir" ]; then output_dir="$argument"; fi
    previous="$argument"
done
touch "$output_dir/whitaker-installer.tgz"
EOF
    chmod 755 "target/$target/release/whitaker-package-installer"
    ;;
esac''',
    )


def _run_make(target: str, cargo: Path, locked: str, stub_dir: Path) -> list[str]:
    """Run one target with stubbed tools and return recorded Cargo arguments."""
    workspace = stub_dir.parent / "workspace"
    scripts_directory = workspace / "scripts"
    scripts_directory.mkdir(parents=True, exist_ok=True)
    shutil.copy2(REPO_ROOT / "Makefile", workspace / "Makefile")
    shutil.copy2(
        REPO_ROOT / "scripts/generate_checksums.py",
        scripts_directory / "generate_checksums.py",
    )
    log = stub_dir / f"{target}-{locked or 'unlocked'}.log"
    environment = os.environ | {
        "CARGO_LOCKED_LOG": str(log),
        "PATH": f"{stub_dir}:/usr/bin:/bin",
    }
    environment.pop("CARGO_LOCKED", None)
    make_arguments = ["make", target, f"CARGO={cargo}"]
    if locked:
        make_arguments.append(f"CARGO_LOCKED={locked}")
    result = subprocess.run(
        make_arguments,
        cwd=workspace,
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    assert result.returncode == 0, result.stderr
    return log.read_text(encoding="utf-8").splitlines()


@pytest.fixture
def cargo_stub(tmp_path: Path) -> tuple[Path, Path]:
    """Create the stub ``bin`` directory with a Cargo stand-in.

    Returns the ``(stub_dir, cargo)`` pair shared by the Makefile tests.
    """
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    cargo = _write_cargo_stub(stub_dir)
    return stub_dir, cargo


@pytest.mark.parametrize("locked", ["", "--locked"])
def test_representative_targets_forward_cargo_locked(
    cargo_stub: tuple[Path, Path], locked: str
) -> None:
    """Ordinary Cargo targets include the requested lock mode and no other one."""
    stub_dir, cargo = cargo_stub

    recorded = [
        invocation
        for target in ("typecheck", "lint")
        for invocation in _run_make(target, cargo, locked, stub_dir)
    ]

    assert recorded, (
        f"typecheck/lint should invoke Cargo in {'locked' if locked else 'unlocked'} mode; "
        f"recorded invocations: {recorded!r}"
    )
    assert all(("--locked" in invocation) == bool(locked) for invocation in recorded), (
        f"typecheck/lint should use {locked or 'unlocked'} mode; "
        f"recorded invocations: {recorded!r}"
    )


@pytest.mark.parametrize("locked", ["", "--locked"])
def test_release_dry_run_forwards_cargo_locked_to_metadata_and_builds(
    cargo_stub: tuple[Path, Path], locked: str
) -> None:
    """Installer metadata and both builds share the caller's lock mode."""
    stub_dir, cargo = cargo_stub
    _write_stub(stub_dir, "rustc", 'echo "host: fake-host"')
    _write_stub(stub_dir, "jq", "echo 0.2.5")

    recorded = _run_make("release-installer-dry-run", cargo, locked, stub_dir)

    assert len(recorded) == 3, (
        "release-installer-dry-run should record metadata plus two builds in "
        f"{'locked' if locked else 'unlocked'} mode; recorded invocations: {recorded!r}"
    )
    assert recorded[0].startswith("metadata "), (
        "release-installer-dry-run should record metadata first in "
        f"{'locked' if locked else 'unlocked'} mode; recorded invocations: {recorded!r}"
    )
    assert all(("--locked" in invocation) == bool(locked) for invocation in recorded), (
        f"release-installer-dry-run should use {locked or 'unlocked'} mode; "
        f"recorded invocations: {recorded!r}"
    )
