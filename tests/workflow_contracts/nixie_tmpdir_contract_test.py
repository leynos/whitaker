"""Exercise the Makefile's temporary-directory choice for Nixie."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]


def _write_nixie_stub(path: Path) -> None:
    """Write a Nixie stub that reports its temporary directory and arguments."""
    path.write_text(
        "#!/bin/sh\nprintf 'NIXIE_TMPDIR=%s\\n' \"$TMPDIR\"\nprintf 'NIXIE_ARGS=%s\\n' \"$*\"\n",
        encoding="utf-8",
    )
    path.chmod(0o755)


def _write_mktemp_stub(path: Path) -> None:
    """Reject the preferred directory and delegate other probes to coreutils."""
    path.write_text(
        "#!/bin/sh\n"
        'case "$2" in "$FAIL_MKTEMP_PREFIX"*) exit 1 ;; esac\n'
        'exec /usr/bin/mktemp "$@"\n',
        encoding="utf-8",
    )
    path.chmod(0o755)


def _run_nixie_target(tmp_path: Path, environment: dict[str, str]) -> str:
    """Run the real Makefile recipe with a hermetic Nixie stub."""
    stub = tmp_path / "nixie"
    _write_nixie_stub(stub)
    result = subprocess.run(
        ["make", "--silent", "--no-print-directory", "nixie", f"NIXIE={stub}"],
        cwd=REPOSITORY_ROOT,
        env=environment,
        capture_output=True,
        check=False,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def test_nixie_prefers_a_writable_xdg_runtime_directory(tmp_path: Path) -> None:
    """Use writable XDG_RUNTIME_DIR ahead of the shared temporary directory."""
    runtime = tmp_path / "runtime"
    fallback = tmp_path / "fallback"
    runtime.mkdir()
    fallback.mkdir()
    assert os.access(runtime, os.W_OK), f"runtime directory is not writable: {runtime}"
    environment = os.environ.copy()
    environment.update({"XDG_RUNTIME_DIR": str(runtime), "TMPDIR": str(fallback)})

    output = _run_nixie_target(tmp_path, environment)

    assert f"NIXIE_TMPDIR={runtime}" in output, output
    assert "NIXIE_ARGS=--no-sandbox" in output, output


def test_nixie_falls_back_when_preferred_directory_cannot_create_scratch(
    tmp_path: Path,
) -> None:
    """Skip a writable runtime directory when it cannot hold scratch files."""
    runtime = tmp_path / "runtime"
    fallback = tmp_path / "fallback"
    runtime.mkdir()
    fallback.mkdir()
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    _write_mktemp_stub(bin_directory / "mktemp")

    environment = os.environ.copy()
    environment.update(
        {
            "XDG_RUNTIME_DIR": str(runtime),
            "TMPDIR": str(fallback),
            "FAIL_MKTEMP_PREFIX": str(runtime),
            "PATH": os.pathsep.join(
                [str(bin_directory), environment.get("PATH", "")]
            ),
        }
    )

    output = _run_nixie_target(tmp_path, environment)

    assert f"NIXIE_TMPDIR={fallback}" in output, output


@pytest.mark.parametrize("invalid_runtime", ["unset", "missing", "file"])
def test_nixie_falls_back_to_tmpdir_for_invalid_runtime(
    tmp_path: Path, invalid_runtime: str
) -> None:
    """Fall back to TMPDIR when XDG_RUNTIME_DIR is absent or unusable."""
    fallback = tmp_path / "fallback"
    fallback.mkdir()
    environment = os.environ.copy()
    environment["TMPDIR"] = str(fallback)

    if invalid_runtime == "unset":
        environment.pop("XDG_RUNTIME_DIR", None)
    elif invalid_runtime == "missing":
        environment["XDG_RUNTIME_DIR"] = str(tmp_path / "missing")
    else:
        runtime_file = tmp_path / "runtime-file"
        runtime_file.touch()
        environment["XDG_RUNTIME_DIR"] = str(runtime_file)

    output = _run_nixie_target(tmp_path, environment)

    assert f"NIXIE_TMPDIR={fallback}" in output, output


@pytest.mark.parametrize(
    ("candidate", "probe_pattern"),
    [("home", '"$HOME"/*'), ("current", "./nixie.*")],
)
def test_nixie_uses_later_candidate_after_other_probes_fail(
    tmp_path: Path, candidate: str, probe_pattern: str
) -> None:
    """Select HOME or the current directory after earlier probes fail."""
    home = tmp_path / "home"
    if candidate == "home":
        home.mkdir()
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    mktemp = bin_directory / "mktemp"
    mktemp.write_text(
        "#!/bin/sh\n"
        f'case "$2" in {probe_pattern}) exec /usr/bin/mktemp "$@" ;; esac\n'
        "exit 1\n",
        encoding="utf-8",
    )
    mktemp.chmod(0o755)
    environment = os.environ.copy()
    environment.update(
        {
            "HOME": str(home),
            "XDG_RUNTIME_DIR": str(tmp_path / "missing-runtime"),
            "TMPDIR": str(tmp_path / "missing-tmp"),
            "PATH": os.pathsep.join([str(bin_directory), environment.get("PATH", "")]),
        }
    )

    output = _run_nixie_target(tmp_path, environment)

    expected = str(home) if candidate == "home" else "."
    assert f"NIXIE_TMPDIR={expected}\n" in output, output


def test_nixie_fails_when_no_candidate_can_create_scratch(tmp_path: Path) -> None:
    """Fail without starting Nixie when every scratch probe fails."""
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    mktemp = bin_directory / "mktemp"
    mktemp.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
    mktemp.chmod(0o755)
    stub = tmp_path / "nixie"
    _write_nixie_stub(stub)
    environment = os.environ.copy()
    environment.update(
        {
            "XDG_RUNTIME_DIR": str(tmp_path / "missing-runtime"),
            "TMPDIR": str(tmp_path / "missing-tmp"),
            "HOME": str(tmp_path / "missing-home"),
            "PATH": os.pathsep.join([str(bin_directory), environment.get("PATH", "")]),
        }
    )

    result = subprocess.run(
        ["make", "--silent", "--no-print-directory", "nixie", f"NIXIE={stub}"],
        cwd=REPOSITORY_ROOT,
        env=environment,
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode != 0, result.stderr
    assert "nixie: no usable temporary directory is available" in result.stderr, result.stderr
    assert "NIXIE_TMPDIR=" not in result.stdout, result.stdout
