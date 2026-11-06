"""Guarantee the `make dylint-env` line format the developer guide parses.

The guide tells a developer to read the target's output line by line rather
than evaluating it, so a path containing spaces or shell metacharacters stays
literal. That advice is only sound while the target prints one `NAME=value`
assignment per line, which is what these tests pin down.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
EXPECTED_NAMES = ("DYLINT_CLIPPY_REPO", "DYLINT_DRIVER_PATH")


def _env_lines() -> list[str]:
    """Return the non-empty lines printed by `make dylint-env`."""
    make = shutil.which("make")
    if make is None:
        pytest.skip("make is not installed")
    completed = subprocess.run(
        [make, "--no-print-directory", "dylint-env"],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return [line for line in completed.stdout.splitlines() if line]


def test_prints_one_assignment_per_line() -> None:
    """Keep every line a single `NAME=value` pair in a stable order."""
    names: list[str] = []
    for line in _env_lines():
        name, separator, _value = line.partition("=")
        assert separator == "=", f"line is not an assignment: {line!r}"
        assert name.isidentifier(), f"line has no variable name: {line!r}"
        names.append(name)

    assert tuple(names) == EXPECTED_NAMES


def test_reading_the_format_keeps_a_value_literal() -> None:
    """Show that the documented read loop does not re-parse a value.

    A path holding a space, a `$`, and a semicolon survives unchanged, which
    is what makes the guide's snippet safe where `eval` was not.
    """
    bash = shutil.which("bash")
    if bash is None:
        pytest.skip("bash is not installed")
    script = "\n".join(
        (
            "while IFS='=' read -r name value; do",
            '    printf "%s|%s\\n" "$name" "$value"',
            "done <<'EOF'",
            "DYLINT_DRIVER_PATH=/tmp/a b/$HOME; echo",
            "EOF",
        )
    )

    completed = subprocess.run(
        [bash, "-c", script],
        check=True,
        capture_output=True,
        text=True,
    )

    assert completed.stdout == "DYLINT_DRIVER_PATH|/tmp/a b/$HOME; echo\n"
