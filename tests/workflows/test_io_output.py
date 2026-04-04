"""Tests for I/O output functions in dependency_binaries_manifest script.

This module tests _open_output() and _write_lines() functions that handle
writing TSV output to files or stdout.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

from tests.workflows.conftest import load_script_module

# Load the dependency_binaries_manifest module
_manifest_module = load_script_module("dependency_binaries_manifest.py")
_open_output = _manifest_module._open_output
_write_lines = _manifest_module._write_lines


class TestOpenOutput:
    """Tests for _open_output() context manager."""

    def test_none_yields_stdout_buffer(self) -> None:
        """None output yields sys.stdout.buffer."""
        with _open_output(None) as handle:
            assert handle is sys.stdout.buffer, (
                "expected sys.stdout.buffer for None output"
            )

    def test_path_yields_writable_handle(self, tmp_path: Path) -> None:
        """A file path yields a writable binary handle."""
        output_file = tmp_path / "output.tsv"

        with _open_output(str(output_file)) as handle:
            handle.write(b"test\n")

        assert output_file.read_bytes() == b"test\n", (
            "file content should match what was written"
        )

    def test_empty_path_exits(self) -> None:
        """An empty output path raises SystemExit."""
        with pytest.raises(SystemExit, match=""):
            with _open_output(""):
                pass  # pragma: no cover


class TestWriteLines:
    """Tests for _write_lines() function."""

    def test_writes_to_file(self, tmp_path: Path) -> None:
        """Lines are written to the specified file."""
        output_file = tmp_path / "output.tsv"
        lines = [b"line1\n", b"line2\n"]

        _write_lines(lines, str(output_file))

        assert output_file.read_bytes() == b"line1\nline2\n", (
            "file should contain all written lines"
        )

    def test_writes_to_stdout(self, capsys: pytest.CaptureFixture) -> None:
        """Lines are written to stdout when output is None."""
        lines = [b"cargo-dylint\tcargo-dylint\t4.1.0\n"]

        _write_lines(lines, None)

        captured = capsys.readouterr()
        assert "cargo-dylint" in captured.out, (
            "stdout should contain the written line content"
        )
