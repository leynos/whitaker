"""Test that run-verus.sh runs the proofs through the pinned prover tool."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
RUN_VERUS = REPOSITORY_ROOT / "scripts" / "run-verus.sh"


def _run_without_make(tmp_path: Path, *arguments: str) -> tuple[int, list[str]]:
    """Run the script with no PROVER_TOOLS, a fake `uv`, and return its arguments."""
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    capture = tmp_path / "uv-arguments.log"
    fake_uv = bin_directory / "uv"
    fake_uv.write_text(
        '#!/bin/sh\nprintf \'%s\\n\' "$@" > "$UV_CAPTURE_FILE"\nexit "${UV_STUB_EXIT:-0}"\n',
        encoding="utf-8",
    )
    fake_uv.chmod(0o755)
    environment = {key: value for key, value in os.environ.items() if key != "PROVER_TOOLS"}
    environment["PATH"] = f"{bin_directory}{os.pathsep}{environment['PATH']}"
    environment["UV_CAPTURE_FILE"] = str(capture)
    result = subprocess.run(
        ["/bin/bash", str(RUN_VERUS), *arguments],
        cwd=REPOSITORY_ROOT,
        env=environment,
        capture_output=True,
        check=False,
        text=True,
    )
    lines = capture.read_text(encoding="utf-8").splitlines() if capture.exists() else []
    return result.returncode, lines


def test_default_command_is_the_pinned_tool_and_selects_the_group(tmp_path: Path) -> None:
    """Resolve the tool from the REF file and pass the group's proof file."""
    pin = next(
        line.split()[1]
        for line in (REPOSITORY_ROOT / "tools/rust-prover-tools/REF")
        .read_text(encoding="utf-8")
        .splitlines()
        if line.startswith("ref:")
    )

    status, arguments = _run_without_make(tmp_path, "string-literals", "--time")

    assert status == 0
    assert f"git+https://github.com/leynos/rust-prover-tools@{pin}" in arguments
    assert arguments[arguments.index("verus") + 1] == "run"
    assert "--no-install-missing" in arguments, "the run must install nothing"
    proof = str(REPOSITORY_ROOT / "verus" / "string_continuation_rewrite.rs")
    assert arguments[arguments.index("--proof-file") + 1] == proof
    assert "--extra-arg=--time" in arguments, "extra flags reach Verus"


def test_a_failing_run_exits_one_with_the_install_hint(tmp_path: Path) -> None:
    """Fail closed, naming the install target, when the tool reports failure."""
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    (bin_directory / "uv").write_text("#!/bin/sh\nexit 3\n", encoding="utf-8")
    (bin_directory / "uv").chmod(0o755)
    result = subprocess.run(
        ["/bin/bash", str(RUN_VERUS), "cli"],
        cwd=REPOSITORY_ROOT,
        env={
            **{k: v for k, v in os.environ.items() if k != "PROVER_TOOLS"},
            "PATH": f"{bin_directory}{os.pathsep}{os.environ['PATH']}",
        },
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 1
    assert "make install-verus" in result.stderr
