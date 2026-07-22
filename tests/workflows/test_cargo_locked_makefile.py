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


def _run_make(
    target: str,
    cargo: Path,
    locked: str,
    stub_dir: Path,
    extra_make_args: list[str] | None = None,
) -> list[str]:
    """Run one target with stubbed tools and return recorded Cargo arguments."""
    workspace = stub_dir.parent / "workspace"
    scripts_directory = workspace / "scripts"
    scripts_directory.mkdir(parents=True, exist_ok=True)
    shutil.copy2(REPO_ROOT / "Makefile", workspace / "Makefile")
    shutil.copy2(
        REPO_ROOT / "scripts/generate_checksums.py",
        scripts_directory / "generate_checksums.py",
    )
    # `publish-check` reads `rust-toolchain.toml` for the channel and shells out
    # to `scripts/install-dylint-tools.sh`; provide the manifest and a no-op
    # stub so the recipe runs without a real toolchain or network access. Both
    # are inert for the other targets.
    shutil.copy2(REPO_ROOT / "rust-toolchain.toml", workspace / "rust-toolchain.toml")
    _write_stub(scripts_directory, "install-dylint-tools.sh", "exit 0")

    log = stub_dir / f"{target}-{locked or 'unlocked'}.log"
    environment = os.environ | {
        "CARGO_LOCKED_LOG": str(log),
        "PATH": f"{stub_dir}:/usr/bin:/bin",
    }
    environment.pop("CARGO_LOCKED", None)
    make_arguments = ["make", target, f"CARGO={cargo}"]
    if locked:
        make_arguments.append(f"CARGO_LOCKED={locked}")
    if extra_make_args:
        make_arguments.extend(extra_make_args)
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


def _write_tool_stubs(stub_dir: Path) -> None:
    """Write stand-ins for the non-Cargo tools `test`/`publish-check` invoke."""
    _write_stub(stub_dir, "cargo-nextest", "exit 0")
    _write_stub(stub_dir, "rustup", "exit 0")
    _write_stub(
        stub_dir,
        "git",
        # `git clone <repo> <dest>` must create <dest>; `git rev-parse` prints a
        # placeholder SHA; other subcommands (checkout, ...) are no-ops.
        'case "$1" in\n'
        '    clone) mkdir -p "$3" ;;\n'
        "    rev-parse) echo 0000000000000000000000000000000000000000 ;;\n"
        "esac",
    )


def _write_publish_check_cargo_stub(directory: Path) -> Path:
    """Cargo stand-in for `publish-check`: records args and fakes per-lint libs.

    A per-lint `cargo build ... -p <lint>` is expected to produce
    `$CARGO_TARGET_DIR/release/lib<lint>.so`, which the recipe then copies, so
    the stub materializes that file when it sees such a build.
    """
    return _write_stub(
        directory,
        "cargo",
        'echo "$@" >> "$CARGO_LOCKED_LOG"\n'
        'lint=""\n'
        'previous=""\n'
        'for argument in "$@"; do\n'
        '    if [ "$previous" = "-p" ]; then lint="$argument"; fi\n'
        '    previous="$argument"\n'
        "done\n"
        'case " $* " in\n'
        '    *" build "*)\n'
        '        if [ -n "$lint" ] && [ -n "${CARGO_TARGET_DIR:-}" ]; then\n'
        '            mkdir -p "$CARGO_TARGET_DIR/release"\n'
        '            : > "$CARGO_TARGET_DIR/release/lib$lint.so"\n'
        "        fi\n"
        "        ;;\n"
        "esac",
    )


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
    return _cargo_head(invocation) in {"build", "nextest run", "package"}


@pytest.mark.parametrize("locked", ["", "--locked"])
def test_make_test_forwards_cargo_locked(cargo_stub: tuple[Path, Path], locked: str) -> None:
    """`make test` threads the requested lock mode into its nextest invocation."""
    stub_dir, cargo = cargo_stub
    _write_tool_stubs(stub_dir)

    # Point the whitaker-script backup safeguard at an absent temp path so the
    # recipe never touches the real `$(HOME)/.local/bin/whitaker`.
    whitaker_script = stub_dir.parent / "whitaker-script"
    recorded = _run_make(
        "test",
        cargo,
        locked,
        stub_dir,
        extra_make_args=[f"WHITAKER_SCRIPT={whitaker_script}"],
    )

    cargo_calls = [line for line in recorded if _is_lock_relevant_invocation(line)]
    assert cargo_calls, f"make test should invoke Cargo; recorded: {recorded!r}"
    for line in cargo_calls:
        assert ("--locked" in line) == bool(locked), (
            f"make test must use {locked or 'unlocked'} mode; invocation: {line!r}"
        )


@pytest.mark.parametrize("locked", ["", "--locked"])
def test_publish_check_forwards_cargo_locked_to_every_invocation(
    tmp_path: Path, locked: str
) -> None:
    """Every lock-relevant Cargo call in `publish-check` honours the lock mode."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    cargo = _write_publish_check_cargo_stub(stub_dir)
    _write_tool_stubs(stub_dir)

    recorded = _run_make(
        "publish-check",
        cargo,
        locked,
        stub_dir,
        extra_make_args=[
            "LINT_CRATES=bumpy_road_function",
            "PUBLISH_PACKAGES=whitaker-common",
        ],
    )

    lock_relevant = [line for line in recorded if _is_lock_relevant_invocation(line)]
    heads = [_cargo_head(line) for line in lock_relevant]
    # publish-check must exercise the workspace build, nextest, a per-lint build,
    # and packaging — not just a single invocation.
    assert heads.count("build") >= 2, (
        f"publish-check should run the workspace and per-lint builds; recorded: {recorded!r}"
    )
    assert "nextest run" in heads, f"publish-check should run nextest; recorded: {recorded!r}"
    assert "package" in heads, f"publish-check should package crates; recorded: {recorded!r}"
    for line in lock_relevant:
        assert ("--locked" in line) == bool(locked), (
            f"publish-check must use {locked or 'unlocked'} mode; invocation: {line!r}"
        )


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
