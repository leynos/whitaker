"""Behavioural tests for the install-dylint-tools script.

This module exercises `scripts/install-dylint-tools.sh` against stubbed
`curl`, `cargo-dylint`, and `uname` binaries plus locally generated
release archives, covering the checksum-verified provisioning contract
used by the `publish-check` Makefile target:

- matching system tools produce no download and no tools root;
- a stale or missing cargo-dylint downloads, verifies, and installs the
  pinned archive into the tools root;
- a missing dylint-link does the same for its own archive;
- a digest mismatch aborts without installing anything;
- an unsupported architecture aborts;
- a version with no pinned digest aborts rather than downloading an
  unverified archive or falling back to a source build;
- the script never shells out to `cargo install`.

No network access occurs: the `curl` stub serves archives built by the
tests, whose real SHA-256 is injected through the script's
``DYLINT_TOOLS_SHA256_*`` test override.

Examples
--------
Run all tests:
    python3 -m pytest tests/workflows/test_install_dylint_tools.py -v
"""

from __future__ import annotations

import hashlib
import io
import os
import platform
import subprocess
import tarfile
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "install-dylint-tools.sh"

PINNED_VERSION = "6.0.1"
CARGO_DYLINT_VERSION = PINNED_VERSION
DYLINT_LINK_VERSION = PINNED_VERSION
TOOLCHAIN = "stable"

DIGEST_ENV = {
    "cargo-dylint": "DYLINT_TOOLS_SHA256_CARGO_DYLINT",
    "dylint-link": "DYLINT_TOOLS_SHA256_DYLINT_LINK",
}
TARGET_TRIPLES = {
    "x86_64": "x86_64-unknown-linux-gnu",
    "aarch64": "aarch64-unknown-linux-gnu",
    "arm64": "aarch64-unknown-linux-gnu",
}


def _host_target() -> str:
    """Return the release target triple the script will resolve here."""
    machine = platform.machine()
    triple = TARGET_TRIPLES.get(machine)
    if triple is None:
        pytest.skip(f"no dylint release target for host machine {machine}")
    return triple


def _write_stub(directory: Path, name: str, body: str) -> Path:
    """Write an executable shell stub and return its path."""
    stub = directory / name
    stub.write_text(f"#!/bin/sh\n{body}\n")
    stub.chmod(0o755)
    return stub


def _write_cargo_dylint_stub(directory: Path, version_line: str) -> Path:
    """Write a fake ``cargo-dylint`` honouring only the 6.x probe form.

    Since 6.x the binary rejects a bare ``--version``; the stub mirrors
    that so the script's probe is exercised against the real contract.
    """
    return _write_stub(
        directory,
        "cargo-dylint",
        f"""if [ "$1" = "dylint" ] && [ "$2" = "--version" ]; then
    echo "{version_line}"
    exit 0
fi
echo "error: unexpected argument" >&2
exit 2""",
    )


def _make_archive(fixture_dir: Path, tool: str, payload: str) -> str:
    """Build the release archive for ``tool`` and return its SHA-256.

    The archive mirrors the upstream layout: a single top-level
    directory named after the archive stem holding one executable named
    after the tool.
    """
    stem = f"{tool}-{_host_target()}-v{PINNED_VERSION}"
    archive = fixture_dir / f"{stem}.tar.gz"
    contents = f"#!/bin/sh\n{payload}\n".encode()
    with tarfile.open(archive, "w:gz") as bundle:
        info = tarfile.TarInfo(f"{stem}/{tool}")
        info.size = len(contents)
        info.mode = 0o755
        bundle.addfile(info, io.BytesIO(contents))
    return hashlib.sha256(archive.read_bytes()).hexdigest()


def _write_curl_stub(stub_dir: Path, fixture_dir: Path) -> Path:
    """Write a ``curl`` stub serving archives from ``fixture_dir``.

    Unknown URLs exit 22, matching ``curl --fail`` on a 404, so a test
    that forgets a fixture fails loudly rather than silently passing.
    """
    return _write_stub(
        stub_dir,
        "curl",
        f"""output=""
url=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) output=$2; shift 2 ;;
        https://*) url=$1; shift ;;
        *) shift ;;
    esac
done
echo "$url" >> "{stub_dir}/curl.log"
name=${{url##*/}}
if [ -f "{fixture_dir}/$name" ]; then
    cp "{fixture_dir}/$name" "$output"
    exit 0
fi
exit 22""",
    )


class Harness(typ.NamedTuple):
    """Paths and environment for one scripted run."""

    stub_dir: Path
    fixture_dir: Path
    tools_root: Path
    digests: dict[str, str]


def _make_harness(tmp_path: Path) -> Harness:
    """Create the stub directory, fixture directory, and tools root."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    fixture_dir = tmp_path / "fixtures"
    fixture_dir.mkdir()
    _write_curl_stub(stub_dir, fixture_dir)
    return Harness(stub_dir, fixture_dir, tmp_path / "tools", {})


def _publish(harness: Harness, tool: str, payload: str) -> None:
    """Publish a fixture archive for ``tool`` and record its digest."""
    harness.digests[tool] = _make_archive(harness.fixture_dir, tool, payload)


def _run_script(
    harness: Harness,
    *,
    cargo_dylint_version: str = CARGO_DYLINT_VERSION,
    dylint_link_version: str = DYLINT_LINK_VERSION,
    toolchain: str | None = None,
    extra_env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the script with PATH restricted to the stub directory."""
    env = os.environ.copy()
    env["PATH"] = f"{harness.stub_dir}:/usr/bin:/bin"
    for tool, digest in harness.digests.items():
        env[DIGEST_ENV[tool]] = digest
    env.update(extra_env or {})
    argv = [
        str(SCRIPT),
        str(harness.tools_root),
        cargo_dylint_version,
        dylint_link_version,
        "cargo",
    ]
    if toolchain is not None:
        argv.append(toolchain)
    return subprocess.run(
        argv,
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )


def _downloads(harness: Harness) -> str:
    log = harness.stub_dir / "curl.log"
    return log.read_text() if log.exists() else ""


def test_matching_tools_download_nothing(tmp_path: Path) -> None:
    """Matching system tools must not download or create the root."""
    harness = _make_harness(tmp_path)
    _write_cargo_dylint_stub(harness.stub_dir, f"cargo-dylint {PINNED_VERSION}")
    _write_stub(harness.stub_dir, "dylint-link", "exit 0")

    result = _run_script(harness, toolchain=TOOLCHAIN)

    assert result.returncode == 0, result.stderr
    assert _downloads(harness) == ""
    assert not harness.tools_root.exists()


@pytest.mark.parametrize(
    "installed_version",
    ["5.0.0", None],
    ids=["stale", "missing"],
)
def test_stale_or_missing_cargo_dylint_installs_pin(
    tmp_path: Path,
    installed_version: str | None,
) -> None:
    """A wrong or absent cargo-dylint installs the verified archive."""
    harness = _make_harness(tmp_path)
    if installed_version is not None:
        _write_cargo_dylint_stub(harness.stub_dir, f"cargo-dylint {installed_version}")
    _write_stub(harness.stub_dir, "dylint-link", "exit 0")
    _publish(
        harness,
        "cargo-dylint",
        f'echo "cargo-dylint {PINNED_VERSION}"',
    )

    result = _run_script(harness)

    assert result.returncode == 0, result.stderr
    installed = harness.tools_root / "bin" / "cargo-dylint"
    assert os.access(installed, os.X_OK)
    assert f"cargo-dylint-{_host_target()}-v{PINNED_VERSION}.tar.gz" in _downloads(
        harness
    )
    assert not list((harness.tools_root / "bin").glob("*.new"))


def test_missing_dylint_link_installs_pin(tmp_path: Path) -> None:
    """An absent dylint-link installs the verified archive."""
    harness = _make_harness(tmp_path)
    _write_cargo_dylint_stub(harness.stub_dir, f"cargo-dylint {PINNED_VERSION}")
    _publish(harness, "dylint-link", "exit 0")

    result = _run_script(harness)

    assert result.returncode == 0, result.stderr
    installed = harness.tools_root / "bin" / "dylint-link"
    assert os.access(installed, os.X_OK)
    assert not (harness.tools_root / "bin" / "cargo-dylint").exists()


def test_digest_mismatch_aborts_and_installs_nothing(tmp_path: Path) -> None:
    """A checksum mismatch aborts before the tools root is created."""
    harness = _make_harness(tmp_path)
    _write_stub(harness.stub_dir, "dylint-link", "exit 0")
    _publish(harness, "cargo-dylint", f'echo "cargo-dylint {PINNED_VERSION}"')
    harness.digests["cargo-dylint"] = "0" * 64

    result = _run_script(harness)

    assert result.returncode != 0
    assert "SHA-256 mismatch" in result.stderr
    assert not harness.tools_root.exists()


def test_unsupported_architecture_aborts(tmp_path: Path) -> None:
    """An architecture with no prebuilt archive is a hard error."""
    harness = _make_harness(tmp_path)
    _write_stub(harness.stub_dir, "dylint-link", "exit 0")
    _write_stub(
        harness.stub_dir,
        "uname",
        """case "$1" in
    -s) echo Linux ;;
    -m) echo riscv64 ;;
esac""",
    )

    result = _run_script(harness)

    assert result.returncode != 0
    assert "riscv64" in result.stderr
    assert not harness.tools_root.exists()


def test_unpinned_version_aborts(tmp_path: Path) -> None:
    """A version with no pinned digest must never be downloaded."""
    harness = _make_harness(tmp_path)
    _write_stub(harness.stub_dir, "dylint-link", "exit 0")

    result = _run_script(harness, cargo_dylint_version="5.0.0")

    assert result.returncode != 0
    assert "5.0.0" in result.stderr
    assert _downloads(harness) == ""
    assert not harness.tools_root.exists()


def test_script_never_builds_from_source() -> None:
    """The provisioning path must not compile a host tool."""
    assert "cargo install" not in SCRIPT.read_text()


def test_rejects_wrong_argument_count() -> None:
    """The script demands its documented argument list."""
    result = subprocess.run(
        [str(SCRIPT), "only-one-arg"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 2
    assert "usage:" in result.stderr
