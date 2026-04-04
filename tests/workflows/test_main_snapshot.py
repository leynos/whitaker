"""Tests for main() function and snapshot tests against real manifest.

This module tests the main() entry point and validates TSV output structure
against the real installer manifest file.
"""

from __future__ import annotations

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from tests.workflows.conftest import load_script_module
from tests.workflows.dependency_manifest_test_data import (
    DUPLICATE_MANIFEST,
    VALID_MANIFEST,
)

# Load the dependency_binaries_manifest module
_manifest_module = load_script_module("dependency_binaries_manifest.py")
main = _manifest_module.main
_collect_manifest_lines = _manifest_module._collect_manifest_lines


class TestMain:
    """Tests for main() function."""

    def test_main_success(self, tmp_path: Path, write_manifest) -> None:
        """main returns 0 on valid manifest input."""
        manifest = write_manifest(tmp_path / "manifest.toml", VALID_MANIFEST)

        with patch(
            "sys.argv",
            ["dependency_binaries_manifest.py", str(manifest)],
        ):
            result = main()

        assert result == 0, f"expected exit code 0, got {result}"

    def test_main_writes_to_output_file(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """main writes TSV output to the specified file."""
        manifest = write_manifest(tmp_path / "manifest.toml", VALID_MANIFEST)
        output_file = tmp_path / "output.tsv"

        with patch(
            "sys.argv",
            [
                "dependency_binaries_manifest.py",
                str(manifest),
                "--output",
                str(output_file),
            ],
        ):
            result = main()

        assert result == 0, f"expected exit code 0, got {result}"
        content = output_file.read_text(encoding="utf-8")
        lines = content.strip().split("\n")
        assert len(lines) == 2, f"expected 2 lines, got {len(lines)}"
        assert lines[0] == "cargo-dylint\tcargo-dylint\t4.1.0", (
            f"first line mismatch: {lines[0]!r}"
        )
        assert lines[1] == "dylint-link\tdylint-link\t4.1.0", (
            f"second line mismatch: {lines[1]!r}"
        )

    def test_main_duplicate_returns_error(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """main returns 1 when the manifest contains duplicate packages."""
        manifest = write_manifest(
            tmp_path / "manifest.toml", DUPLICATE_MANIFEST
        )

        with patch(
            "sys.argv",
            ["dependency_binaries_manifest.py", str(manifest)],
        ):
            result = main()

        assert result == 1, f"expected exit code 1, got {result}"

    def test_main_missing_file_raises(self, tmp_path: Path) -> None:
        """main raises an exception for a non-existent manifest file."""
        missing = tmp_path / "does_not_exist.toml"

        with patch(
            "sys.argv",
            ["dependency_binaries_manifest.py", str(missing)],
        ):
            with pytest.raises(FileNotFoundError, match=""):
                main()

    def test_main_uses_default_manifest_path(self) -> None:
        """main uses default manifest path when no argument given."""
        mock_collect = MagicMock(return_value=[b"test\ttest\t1.0\n"])

        with patch("sys.argv", ["dependency_binaries_manifest.py"]):
            with patch.object(
                _manifest_module,
                "_collect_manifest_lines",
                mock_collect,
            ):
                result = main()

        assert result == 0, f"expected exit code 0, got {result}"
        mock_collect.assert_called_once()
        call_args = mock_collect.call_args[0]
        assert len(call_args) == 1, (
            f"expected 1 argument to _collect_manifest_lines, got {len(call_args)}"
        )
        manifest_path = call_args[0]
        assert str(manifest_path) == "installer/dependency-binaries.toml", (
            f"expected default manifest path, got {manifest_path}"
        )


class TestSnapshotOutput:
    """Snapshot-style tests for TSV output against the real manifest."""

    def test_real_manifest_output_structure(self, tmp_path: Path) -> None:
        """TSV output for the real manifest has three non-empty columns per line."""
        real_manifest = (
            Path(__file__).resolve().parents[2]
            / "installer"
            / "dependency-binaries.toml"
        )
        if not real_manifest.exists():
            pytest.skip("real manifest not available")

        result = _collect_manifest_lines(real_manifest)

        assert isinstance(result, list), f"expected list, got {type(result)}"
        assert len(result) >= 1, (
            f"expected at least 1 entry, got {len(result)}"
        )
        for line in result:
            decoded = line.decode("utf-8").rstrip("\n")
            columns = decoded.split("\t")
            assert len(columns) == 3, (
                f"expected 3 tab-separated columns, got {len(columns)}: {decoded!r}"
            )
            for i, col in enumerate(columns):
                assert col != "", (
                    f"column {i} must not be empty in: {decoded!r}"
                )

    def test_real_manifest_round_trip(self, tmp_path: Path) -> None:
        """The real manifest can be read and written to a file."""
        real_manifest = (
            Path(__file__).resolve().parents[2]
            / "installer"
            / "dependency-binaries.toml"
        )
        if not real_manifest.exists():
            pytest.skip("real manifest not available")
        output_file = tmp_path / "output.tsv"

        with patch(
            "sys.argv",
            [
                "dependency_binaries_manifest.py",
                str(real_manifest),
                "--output",
                str(output_file),
            ],
        ):
            result = main()

        assert result == 0, f"expected exit code 0, got {result}"
        content = output_file.read_text(encoding="utf-8")
        lines = content.strip().split("\n")
        assert len(lines) >= 1, f"expected at least 1 line, got {len(lines)}"
        for line in lines:
            columns = line.split("\t")
            assert len(columns) == 3, (
                f"expected 3 tab-separated columns, got {len(columns)}: {line!r}"
            )
            for i, col in enumerate(columns):
                assert col != "", (
                    f"column {i} must not be empty in: {line!r}"
                )
