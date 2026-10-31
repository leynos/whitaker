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
    """Run harness selection against stubbed Kani installer and runner commands."""
    script_directory = tmp_path / "scripts"
    script_directory.mkdir()
    kani_directory = tmp_path / "kani"
    (kani_directory / "bin").mkdir(parents=True)
    (kani_directory / "toolchain" / "bin").mkdir(parents=True)
    (kani_directory / "rust-toolchain-version").write_text("nightly-test\n", encoding="utf-8")
    _write_executable(script_directory / "run-kani.sh", RUN_KANI.read_text(encoding="utf-8"))
    _write_executable(
        script_directory / "install-kani.sh",
        "#!/bin/sh\nprintf '%s\\n' \"$FAKE_KANI_DIR\"\n",
    )
    capture = tmp_path / "runner-arguments.log"
    _write_executable(
        script_directory / "run-kani-limited.sh",
        """#!/bin/bash
printf '%s\\n' "$*" >> "$KANI_CAPTURE_FILE"
exit "${KANI_STUB_EXIT:-0}"
""",
    )
    (kani_directory / "bin" / "cargo-kani").write_text("", encoding="utf-8")
    environment = os.environ.copy()
    environment.update(
        {
            "FAKE_KANI_DIR": str(kani_directory),
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
