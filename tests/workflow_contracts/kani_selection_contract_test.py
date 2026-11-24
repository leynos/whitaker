"""Test Kani harness selection without installing the verifier."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
RUN_KANI = REPOSITORY_ROOT / "scripts" / "run-kani.sh"


def _write_executable(path: Path, contents: str) -> None:
    """Write an executable shell stub."""
    path.write_text(contents, encoding="utf-8")
    path.chmod(0o755)


def _run_kani_selection(
    tmp_path: Path, arguments: tuple[str, ...], runner_exit: str = "0"
) -> subprocess.CompletedProcess[str]:
    """Run harness selection against a stubbed Kani installation and runner."""
    script_directory = tmp_path / "scripts"
    script_directory.mkdir()
    (tmp_path / "tools" / "kani").mkdir(parents=True)
    (tmp_path / "tools" / "kani" / "VERSION").write_text("0.0.0\n", encoding="utf-8")
    kani_home = tmp_path / "kani-home"
    kani_directory = kani_home / "kani-0.0.0"
    (kani_directory / "bin").mkdir(parents=True)
    (kani_directory / "toolchain" / "bin").mkdir(parents=True)
    (kani_directory / "rust-toolchain-version").write_text("nightly-test\n", encoding="utf-8")
    cargo_home = tmp_path / "cargo-home"
    (cargo_home / "bin").mkdir(parents=True)
    _write_executable(
        cargo_home / "bin" / "cargo-kani",
        "#!/bin/sh\nprintf '%s\\n' \"cargo-kani ${FAKE_KANI_VERSION:-0.0.0}\"\n",
    )
    _write_executable(script_directory / "run-kani.sh", RUN_KANI.read_text(encoding="utf-8"))
    capture = tmp_path / "runner-arguments.log"
    _write_executable(
        script_directory / "run-kani-limited.sh",
        """#!/bin/bash
printf '%s\\n' "$*" >> "$KANI_CAPTURE_FILE"
exit "${KANI_STUB_EXIT:-0}"
""",
    )
    environment = os.environ.copy()
    environment.update(
        {
            "CARGO_HOME": str(cargo_home),
            "KANI_HOME": str(kani_home),
            "KANI_CAPTURE_FILE": str(capture),
            "KANI_STUB_EXIT": runner_exit,
        }
    )
    return subprocess.run(
        ["/bin/bash", str(script_directory / "run-kani.sh"), *arguments],
        cwd=REPOSITORY_ROOT,
        env=environment,
        capture_output=True,
        check=False,
        text=True,
    )


def test_kani_runner_selects_only_the_requested_harness(tmp_path: Path) -> None:
    """Pass one known clone-detector harness to the limited runner."""
    result = _run_kani_selection(
        tmp_path, ("clone-detector", "verify_missing_cover_returns_none")
    )
    capture = tmp_path / "runner-arguments.log"
    arguments = capture.read_text(encoding="utf-8").splitlines()

    assert result.returncode == 0, result.stderr
    assert len(arguments) == 1, f"runner invocations: {arguments!r}"
    assert arguments[0].startswith("verify_missing_cover_returns_none "), arguments[0]


def test_kani_runner_rejects_unknown_harnesses(tmp_path: Path) -> None:
    """Reject a harness absent from the declared clone-detector group."""
    result = _run_kani_selection(tmp_path, ("clone-detector", "verify_not_a_harness"))

    assert result.returncode == 2, result.stderr
    assert "unknown clone-detector Kani harness" in result.stderr, result.stderr
    assert not (tmp_path / "runner-arguments.log").exists(), (
        "limited runner was invoked for an unknown harness"
    )


def test_kani_runner_rejects_invalid_harness_command_arguments(tmp_path: Path) -> None:
    """Require exactly one harness with the explicit single-harness command."""
    result = _run_kani_selection(tmp_path, ("clone-detector-harness",))

    assert result.returncode == 2, result.stderr
    assert "usage: scripts/run-kani.sh clone-detector-harness HARNESS" in result.stderr, result.stderr


def test_kani_runner_propagates_limited_runner_failure(tmp_path: Path) -> None:
    """Propagate a failed limited-runner status through harness selection."""
    result = _run_kani_selection(
        tmp_path,
        ("clone-detector-harness", "verify_missing_cover_returns_none"),
        runner_exit="37",
    )

    assert result.returncode == 37, result.stderr


def test_kani_runner_fails_with_an_install_hint_when_kani_is_absent(tmp_path: Path) -> None:
    """Install nothing: a missing Kani fails and names the install target."""
    result = _run_kani_selection(tmp_path, ("clone-detector", "verify_missing_cover_returns_none"))
    (tmp_path / "cargo-home" / "bin" / "cargo-kani").unlink()
    (tmp_path / "runner-arguments.log").unlink(missing_ok=True)
    assert result.returncode == 0, result.stderr

    result = subprocess.run(
        ["/bin/bash", str(tmp_path / "scripts" / "run-kani.sh"), "clone-detector"],
        cwd=REPOSITORY_ROOT,
        env={
            **os.environ,
            "CARGO_HOME": str(tmp_path / "cargo-home"),
            "KANI_HOME": str(tmp_path / "kani-home"),
        },
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 1, result.stderr
    assert "make install-kani" in result.stderr, result.stderr
    assert not (tmp_path / "runner-arguments.log").exists(), "no proof ran"


def test_kani_runner_refuses_a_front_end_of_another_version(tmp_path: Path) -> None:
    """A shared cargo-kani of the wrong release must not run the pinned bundle."""
    os.environ["FAKE_KANI_VERSION"] = "9.9.9"
    try:
        result = _run_kani_selection(
            tmp_path, ("clone-detector", "verify_missing_cover_returns_none")
        )
    finally:
        del os.environ["FAKE_KANI_VERSION"]

    assert result.returncode == 1, result.stderr
    assert "is not Kani 0.0.0" in result.stderr, result.stderr
    assert not (tmp_path / "runner-arguments.log").exists(), "no proof ran"
