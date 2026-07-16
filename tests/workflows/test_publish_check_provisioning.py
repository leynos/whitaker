"""Integration tests for the publish-check provisioning handoff.

These tests run the real ``Makefile::publish-check`` recipe with every
external command stubbed (``cargo``, ``cargo-dylint``, ``cargo-nextest``,
``rustup``, and ``git``), proving the Makefile integration itself rather
than `scripts/install-dylint-tools.sh` in isolation:

- the recipe invokes the provisioning script;
- a stale system ``cargo-dylint`` triggers installation into the
  isolated root, and the later Dylint-facing command resolves
  ``cargo-dylint`` from the isolated ``bin/`` ahead of the stale stub;
- a failed install aborts the target, and no subsequent clone, build,
  Dylint, or packaging command executes.

No network access, Rust builds, or real tool installs occur; stubs
record their invocations to a log inspected by the assertions. The
direct tests in ``test_install_dylint_tools.py`` remain the unit-level
coverage for version detection and Cargo invocation construction.

Examples
--------
Run all tests:
    python3 -m pytest tests/workflows/test_publish_check_provisioning.py -v
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

CARGO_DYLINT_VERSION = "6.0.1"
DYLINT_LINK_VERSION = "6.0.1"


def _write_stub(directory: Path, name: str, body: str) -> Path:
    """Write an executable shell stub and return its path."""
    stub = directory / name
    stub.write_text(f"#!/bin/sh\n{body}\n")
    stub.chmod(0o755)
    return stub


def _write_harness(stub_dir: Path, *, install_exit: int = 0) -> Path:
    """Write the full stub command set for a publish-check run.

    Every stub appends ``<command> <args>`` to ``invocations.log``. The
    ``cargo`` stub additionally:

    - creates the expected lint library on ``build --release`` so the
      recipe's real ``cp`` succeeds;
    - creates ``<root>/bin/cargo-dylint`` on ``install`` (when
      ``install_exit`` is zero) so the recipe's ``PATH`` prepend fires;
    - records ``command -v cargo-dylint`` on ``dylint`` so tests can
      assert which binary a Dylint-facing command would resolve.
    """
    log = stub_dir / "invocations.log"
    _write_stub(stub_dir, "rustup", f'echo "rustup $@" >> "{log}"')
    _write_stub(stub_dir, "cargo-nextest", f'echo "cargo-nextest $@" >> "{log}"')
    _write_stub(
        stub_dir,
        "cargo-dylint",
        f"""echo "cargo-dylint $@" >> "{log}"
if [ "$1" = "dylint" ] && [ "$2" = "--version" ]; then
    echo "cargo-dylint 5.0.0"
    exit 0
fi
exit 2""",
    )
    _write_stub(
        stub_dir,
        "git",
        f"""echo "git $@" >> "{log}"
case "$1" in
clone) mkdir -p "$3" ;;
rev-parse) echo 0000000000000000000000000000000000000000 ;;
esac
exit 0""",
    )
    _write_stub(
        stub_dir,
        "cargo",
        f"""case "$1" in
+*) shift ;;
esac
echo "cargo $@" >> "{log}"
case "$1" in
install)
    if [ "$2" = "--list" ]; then
        echo "dylint-link v{DYLINT_LINK_VERSION}:"
        exit 0
    fi
    if [ {install_exit} -ne 0 ]; then
        exit {install_exit}
    fi
    root=""
    prev=""
    for arg in "$@"; do
        if [ "$prev" = "--root" ]; then root="$arg"; fi
        prev="$arg"
    done
    mkdir -p "$root/bin"
    printf '#!/bin/sh\\nexit 0\\n' > "$root/bin/cargo-dylint"
    chmod 755 "$root/bin/cargo-dylint"
    exit 0
    ;;
build)
    if [ -n "${{CARGO_TARGET_DIR:-}}" ]; then
        crate=""
        prev=""
        for arg in "$@"; do
            if [ "$prev" = "-p" ]; then crate="$arg"; fi
            prev="$arg"
        done
        if [ -n "$crate" ]; then
            mkdir -p "$CARGO_TARGET_DIR/release"
            : > "$CARGO_TARGET_DIR/release/lib$crate.so"
        fi
    fi
    exit 0
    ;;
dylint)
    echo "dylint-resolved $(command -v cargo-dylint)" >> "{log}"
    exit 0
    ;;
*)
    exit 0
    ;;
esac""",
    )
    return log


def _run_publish_check(stub_dir: Path) -> subprocess.CompletedProcess[str]:
    """Run the real publish-check target with stubs first on PATH."""
    env = os.environ.copy()
    env["PATH"] = f"{stub_dir}:/usr/bin:/bin"
    env.pop("WHITAKER", None)
    return subprocess.run(
        [
            "make",
            "publish-check",
            f"CARGO={stub_dir}/cargo",
            "LINT_CRATES=test_lint",
            "PUBLISH_PACKAGES=pkg_one",
        ],
        capture_output=True,
        text=True,
        env=env,
        cwd=REPO_ROOT,
        check=False,
    )


def test_stale_tool_installs_and_isolated_bin_wins(tmp_path: Path) -> None:
    """A stale cargo-dylint provisions the pin and yields PATH precedence."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    log_path = _write_harness(stub_dir)

    result = _run_publish_check(stub_dir)

    assert result.returncode == 0, result.stderr
    log = log_path.read_text()
    assert f"--version {CARGO_DYLINT_VERSION}" in log
    assert "cargo-dylint" in log.split("dylint-resolved", 1)[1]
    resolved = next(
        line for line in log.splitlines() if line.startswith("dylint-resolved ")
    ).removeprefix("dylint-resolved ")
    assert resolved != str(stub_dir / "cargo-dylint"), (
        "the stale system stub must not win once the isolated root exists"
    )
    assert resolved.endswith("/dylint-tools/bin/cargo-dylint")


def test_failed_install_aborts_before_clone_and_packaging(tmp_path: Path) -> None:
    """A failed install stops publish-check before any later stage runs."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    log_path = _write_harness(stub_dir, install_exit=1)

    result = _run_publish_check(stub_dir)

    assert result.returncode != 0
    log = log_path.read_text()
    assert "git clone" not in log
    assert "dylint-resolved" not in log
    assert "cargo package" not in log
    assert "build --release" not in log, (
        "no per-lint release build may run after a failed install"
    )
