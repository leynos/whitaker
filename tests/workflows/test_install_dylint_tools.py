"""Behavioural tests for the install-dylint-tools script.

This module exercises `scripts/install-dylint-tools.sh` under stubbed
`cargo` and `cargo-dylint` binaries, covering the pinned-tool
provisioning contract used by the `publish-check` Makefile target:

- matching system tools produce no installs and no tools root
- a stale or missing cargo-dylint triggers a pinned install into the root
- a missing dylint-link pin in ``cargo install --list`` triggers a
  pinned install into the root
- a failing install aborts the script with a non-zero exit status, so
  callers can never proceed with stale tools on PATH

Examples
--------
Run all tests:
    python3 -m pytest tests/workflows/test_install_dylint_tools.py -v
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "install-dylint-tools.sh"

CARGO_DYLINT_VERSION = "6.0.1"
DYLINT_LINK_VERSION = "6.0.1"
TOOLCHAIN = "stable"


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


def _write_cargo_stub(
    directory: Path,
    *,
    installed_dylint_link: str | None,
    install_exit: int = 0,
) -> Path:
    """Write a fake ``cargo`` recording install invocations to a log.

    ``install --list`` reports ``installed_dylint_link`` (or nothing);
    ``install`` appends its arguments to ``cargo-install.log`` and exits
    with ``install_exit``.
    """
    list_line = (
        f'echo "dylint-link v{installed_dylint_link}:"'
        if installed_dylint_link
        else "true"
    )
    return _write_stub(
        directory,
        "cargo",
        f"""case "$1" in
+*)
    echo "$1" >> "{directory}/cargo-toolchain.log"
    shift
    ;;
esac
case "$1" in
install)
    if [ "$2" = "--list" ]; then
        {list_line}
        exit 0
    fi
    echo "$@" >> "{directory}/cargo-install.log"
    exit {install_exit}
    ;;
*)
    exit 1
    ;;
esac""",
    )


def _run_script(
    stub_dir: Path,
    tools_root: Path,
    *,
    toolchain: str | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the script with PATH restricted to the stub directory."""
    env = os.environ.copy()
    env["PATH"] = f"{stub_dir}:/usr/bin:/bin"
    argv = [
        str(SCRIPT),
        str(tools_root),
        CARGO_DYLINT_VERSION,
        DYLINT_LINK_VERSION,
        str(stub_dir / "cargo"),
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


def _install_log(stub_dir: Path) -> str:
    log = stub_dir / "cargo-install.log"
    return log.read_text() if log.exists() else ""


def test_matching_tools_install_nothing(tmp_path: Path) -> None:
    """Matching system versions must not trigger any install."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    _write_cargo_dylint_stub(stub_dir, f"cargo-dylint {CARGO_DYLINT_VERSION}")
    _write_cargo_stub(stub_dir, installed_dylint_link=DYLINT_LINK_VERSION)
    tools_root = tmp_path / "tools"

    result = _run_script(stub_dir, tools_root)

    assert result.returncode == 0, result.stderr
    assert _install_log(stub_dir) == ""
    assert not tools_root.exists()


@pytest.mark.parametrize(
    ("dylint_version_output", "expected_package"),
    [
        ("cargo-dylint 5.0.0", "cargo-dylint"),
        ("", "cargo-dylint"),
    ],
    ids=["stale", "missing"],
)
def test_stale_or_missing_cargo_dylint_installs_pin(
    tmp_path: Path,
    dylint_version_output: str,
    expected_package: str,
) -> None:
    """A wrong or absent cargo-dylint installs the pin into the root."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    if dylint_version_output:
        _write_cargo_dylint_stub(stub_dir, dylint_version_output)
    _write_cargo_stub(stub_dir, installed_dylint_link=DYLINT_LINK_VERSION)
    tools_root = tmp_path / "tools"

    result = _run_script(stub_dir, tools_root)

    assert result.returncode == 0, result.stderr
    log = _install_log(stub_dir)
    assert f"--version {CARGO_DYLINT_VERSION}" in log
    assert f"--root {tools_root}" in log
    assert log.rstrip().endswith(expected_package)


def test_missing_dylint_link_installs_pin(tmp_path: Path) -> None:
    """An unpinned dylint-link in the install list triggers an install."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    _write_cargo_dylint_stub(stub_dir, f"cargo-dylint {CARGO_DYLINT_VERSION}")
    _write_cargo_stub(stub_dir, installed_dylint_link="5.0.0")
    tools_root = tmp_path / "tools"

    result = _run_script(stub_dir, tools_root)

    assert result.returncode == 0, result.stderr
    log = _install_log(stub_dir)
    assert f"--version {DYLINT_LINK_VERSION}" in log
    assert log.rstrip().endswith("dylint-link")


def test_failed_install_aborts_nonzero(tmp_path: Path) -> None:
    """A failing install must abort with a non-zero exit status."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    _write_cargo_dylint_stub(stub_dir, "cargo-dylint 5.0.0")
    _write_cargo_stub(
        stub_dir,
        installed_dylint_link=DYLINT_LINK_VERSION,
        install_exit=1,
    )

    result = _run_script(stub_dir, tmp_path / "tools")

    assert result.returncode != 0


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


def test_toolchain_argument_prefixes_installs(tmp_path: Path) -> None:
    """A toolchain argument routes installs through ``cargo +toolchain``."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    _write_cargo_dylint_stub(stub_dir, "cargo-dylint 5.0.0")
    _write_cargo_stub(stub_dir, installed_dylint_link=DYLINT_LINK_VERSION)
    tools_root = tmp_path / "tools"

    result = _run_script(stub_dir, tools_root, toolchain=TOOLCHAIN)

    assert result.returncode == 0, result.stderr
    toolchain_log = stub_dir / "cargo-toolchain.log"
    assert toolchain_log.read_text().strip() == f"+{TOOLCHAIN}"
    assert f"--version {CARGO_DYLINT_VERSION}" in _install_log(stub_dir)


def test_version_probe_ignores_toolchain(tmp_path: Path) -> None:
    """The ``install --list`` probe stays on the default cargo."""
    stub_dir = tmp_path / "bin"
    stub_dir.mkdir()
    _write_cargo_dylint_stub(stub_dir, f"cargo-dylint {CARGO_DYLINT_VERSION}")
    _write_cargo_stub(stub_dir, installed_dylint_link=DYLINT_LINK_VERSION)

    result = _run_script(stub_dir, tmp_path / "tools", toolchain=TOOLCHAIN)

    assert result.returncode == 0, result.stderr
    assert not (stub_dir / "cargo-toolchain.log").exists()
