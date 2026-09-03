"""Integration tests for the publish-check provisioning handoff.

These tests run the real ``Makefile::publish-check`` recipe with every
external command stubbed (``cargo``, ``cargo-dylint``, ``cargo-nextest``,
``rustup``, ``git``, and ``curl``), proving the Makefile integration
itself rather than `scripts/install-dylint-tools.sh` in isolation:

- the recipe invokes the provisioning script;
- a stale system ``cargo-dylint`` triggers a verified download into the
  cached tools root, and the later Dylint-facing command resolves
  ``cargo-dylint`` from that ``bin/`` ahead of the stale stub;
- a failed download aborts the target, and no subsequent clone, build,
  Dylint, or packaging command executes.

The host tools are now fetched as checksum-verified prebuilt release
archives rather than compiled, so the harness stubs ``curl`` with a
locally generated archive and injects its real digest through the
script's documented test hook. No network access, Rust builds, or real
tool installs occur; stubs record their invocations to a log inspected
by the assertions. The direct tests in ``test_install_dylint_tools.py``
remain the unit-level coverage for probing and verification.

Examples
--------
Run all tests:
    python3 -m pytest tests/workflows/test_publish_check_provisioning.py -v
"""

from __future__ import annotations

import hashlib
import io
import os
import subprocess
import tarfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

CARGO_DYLINT_VERSION = "6.0.1"
DYLINT_LINK_VERSION = "6.0.1"
TOOLS = ("cargo-dylint", "dylint-link")


def _build_release_archive(destination: Path, tool: str, version: str) -> str:
    """Write a stand-in release archive and return its SHA-256 digest.

    The layout mirrors the upstream archives: one top-level directory
    named after the archive stem containing a single executable.
    """
    stem = f"{tool}-x86_64-unknown-linux-gnu-v{version}"
    payload = b'#!/bin/sh\nif [ "$1" = dylint ] && [ "$2" = --version ]; then\n'
    payload += f'    echo "cargo-dylint {version}"\n'.encode()
    payload += b"    exit 0\nfi\nexit 0\n"
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        info = tarfile.TarInfo(f"{stem}/{tool}")
        info.size = len(payload)
        info.mode = 0o755
        archive.addfile(info, io.BytesIO(payload))
    data = buffer.getvalue()
    destination.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def _write_curl_stub(
    stub_dir: Path, log: Path, archives: Path, *, exit_code: int
) -> None:
    """Serve the generated archives, or fail with ``exit_code``."""
    _write_stub(
        stub_dir,
        "curl",
        f"""echo "curl $@" >> "{log}"
if [ {exit_code} -ne 0 ]; then
    exit {exit_code}
fi
output=""
url=""
prev=""
for arg in "$@"; do
    if [ "$prev" = "--output" ]; then output="$arg"; fi
    case "$arg" in https://*) url="$arg" ;; esac
    prev="$arg"
done
name="${{url##*/}}"
cp "{archives}/$name" "$output"
""",
    )


def _write_stub(directory: Path, name: str, body: str) -> Path:
    """Write an executable shell stub and return its path."""
    stub = directory / name
    stub.write_text(f"#!/bin/sh\n{body}\n")
    stub.chmod(0o755)
    return stub


def _write_recording_stub(stub_dir: Path, log: Path, name: str) -> None:
    """Write a stub that only records its invocation and succeeds."""
    _write_stub(stub_dir, name, f'echo "{name} $@" >> "{log}"')


def _write_stale_cargo_dylint_stub(stub_dir: Path, log: Path) -> None:
    """Write a stale (5.0.0) cargo-dylint honouring the 6.x probe form."""
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


def _write_git_stub(stub_dir: Path, log: Path) -> None:
    """Write a git stub whose clone creates the destination directory."""
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


def _cargo_build_case() -> str:
    """Return the build branch of the cargo stub.

    Creates the expected lint library under ``CARGO_TARGET_DIR`` so the
    recipe's real ``cp`` succeeds.
    """
    return """build)
    if [ -n "${CARGO_TARGET_DIR:-}" ]; then
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
    ;;"""


def _write_cargo_stub(stub_dir: Path, log: Path) -> None:
    """Write the cargo stub; the dylint branch records what PATH resolves."""
    _write_stub(
        stub_dir,
        "cargo",
        f"""case "$1" in
+*) shift ;;
esac
echo "cargo $@" >> "{log}"
case "$1" in
{_cargo_build_case()}
dylint)
    echo "dylint-resolved $(command -v cargo-dylint)" >> "{log}"
    exit 0
    ;;
*)
    exit 0
    ;;
esac""",
    )


def _write_harness(
    stub_dir: Path, *, install_exit: int = 0
) -> tuple[Path, dict[str, str]]:
    """Write the full stub command set for a publish-check run.

    Every stub appends ``<command> <args>`` to ``invocations.log``; each
    stub's behaviour is documented on its writer. Returns the log path and
    the digest overrides the script needs to accept the generated archives.
    """
    log = stub_dir / "invocations.log"
    archives = stub_dir.parent / "archives"
    archives.mkdir(exist_ok=True)
    digests = {
        tool: _build_release_archive(
            archives
            / f"{tool}-x86_64-unknown-linux-gnu-v{CARGO_DYLINT_VERSION}.tar.gz",
            tool,
            CARGO_DYLINT_VERSION,
        )
        for tool in TOOLS
    }
    _write_recording_stub(stub_dir, log, "rustup")
    _write_recording_stub(stub_dir, log, "cargo-nextest")
    _write_stale_cargo_dylint_stub(stub_dir, log)
    _write_git_stub(stub_dir, log)
    _write_curl_stub(stub_dir, log, archives, exit_code=install_exit)
    _write_cargo_stub(stub_dir, log)
    overrides = {
        "DYLINT_TOOLS_SHA256_CARGO_DYLINT": digests["cargo-dylint"],
        "DYLINT_TOOLS_SHA256_DYLINT_LINK": digests["dylint-link"],
    }
    return log, overrides


def _run_publish_check(
    stub_dir: Path, overrides: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run the real publish-check target with stubs first on PATH."""
    env = os.environ.copy()
    env["PATH"] = f"{stub_dir}:/usr/bin:/bin"
    env.update(overrides)
    # The Makefile prepends `$HOME/.cargo/bin`; isolate it so a host-installed
    # cargo-dylint cannot bypass the harness's deliberately stale stub.
    isolated_home = stub_dir.parent / "home"
    isolated_home.mkdir()
    env["HOME"] = str(isolated_home)
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
    log_path, overrides = _write_harness(stub_dir)

    result = _run_publish_check(stub_dir, overrides)

    assert result.returncode == 0, result.stderr
    log = log_path.read_text()
    assert f"cargo-dylint-x86_64-unknown-linux-gnu-v{CARGO_DYLINT_VERSION}" in log, (
        "the pinned release archive must be downloaded"
    )
    assert "cargo install" not in log, "no host tool may be compiled from source"
    assert "cargo-dylint" in log.split("dylint-resolved", 1)[1], (
        "the resolution line must name the tool it resolved"
    )
    resolved = next(
        line for line in log.splitlines() if line.startswith("dylint-resolved ")
    ).removeprefix("dylint-resolved ")
    assert resolved != str(stub_dir / "cargo-dylint"), (
        "the stale system stub must not win once the isolated root exists"
    )
    assert resolved.endswith("/whitaker-dylint-tools/bin/cargo-dylint"), (
        f"the durable tools root must win PATH resolution, got {resolved}"
    )


def test_failed_download_aborts_before_clone_and_packaging(tmp_path: Path) -> None:
    """A failed download stops publish-check before any later stage runs."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    log_path, overrides = _write_harness(stub_dir, install_exit=1)

    result = _run_publish_check(stub_dir, overrides)

    assert result.returncode != 0, "a failed download must fail publish-check"
    log = log_path.read_text()
    assert "git clone" not in log, "no clone may run after a failed install"
    assert "dylint-resolved" not in log, (
        "no tool resolution may run after a failed install"
    )
    assert "cargo package" not in log, "no packaging may run after a failed install"
    assert "build --release" not in log, (
        "no per-lint release build may run after a failed install"
    )
