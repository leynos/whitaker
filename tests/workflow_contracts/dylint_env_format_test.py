"""Guarantee the `make dylint-env` line format the developer guide parses.

The guide tells a developer to read the target's output line by line rather
than evaluating it, so a path containing spaces or shell metacharacters stays
literal. That advice is only sound while the target prints one `NAME=value`
assignment per line and prints the path as data. These tests pin both halves
against the real producer, not a stand-in for it.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
EXPECTED_NAMES = ("DYLINT_CLIPPY_REPO", "DYLINT_DRIVER_PATH")

# Everything a shell would act on if the producer pasted the path into a
# command: spaces, both quote styles, a command separator, and a subshell
# backtick. `$$` is Make's spelling for one literal `$`, so the value also
# carries dollar signs and what looks like Make function syntax after
# expansion: Make must emit it as data and neither the producer nor the reader
# may evaluate it.
HOSTILE_PATH = "/tmp/a b\"c'd;e`f$$g$$(not_a_function)"

# What the configured value becomes once Make has expanded `$$` once.
EXPECTED_PATH = HOSTILE_PATH.replace("$$", "$")


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


def test_the_documented_loop_exports_a_hostile_path() -> None:
    """Read the real producer's output without letting a shell re-parse it.

    The overridden paths reach the target through its environment, so they are
    the values a developer would configure. The guide's loop exports each
    assignment and the exported variable must still hold the text byte for
    byte; an `eval`-based export would run the embedded `;` and backtick, and a
    lossy export would drop the dollar signs.
    """
    make = shutil.which("make")
    bash = shutil.which("bash")
    if make is None or bash is None:
        pytest.skip("make and bash are required")
    configured = {
        "DYLINT_CLIPPY_REPO": HOSTILE_PATH,
        "DYLINT_DRIVER_PATH": f"{HOSTILE_PATH}/driver",
    }

    produced = subprocess.run(
        [make, "--no-print-directory", "dylint-env"],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
        env={**os.environ, **configured},
    )
    read_loop = "\n".join(
        (
            "while IFS='=' read -r name value; do",
            '    export "$name=$value"',
            '    printf "%s|%s\\n" "$name" "${!name}"',
            "done",
        )
    )
    exported = subprocess.run(
        [bash, "-c", read_loop],
        input=produced.stdout,
        check=True,
        capture_output=True,
        text=True,
    )

    observed = dict(line.split("|", 1) for line in exported.stdout.splitlines() if line)
    expected = {
        "DYLINT_CLIPPY_REPO": EXPECTED_PATH,
        "DYLINT_DRIVER_PATH": f"{EXPECTED_PATH}/driver",
    }
    assert observed == expected