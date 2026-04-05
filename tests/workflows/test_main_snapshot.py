"""Tests for main() function and snapshot tests against real manifest.

This module tests the main() entry point and validates TSV output structure
against the real installer manifest file.
"""

from __future__ import annotations

from collections.abc import Callable
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


def _assert_three_non_empty_tsv_columns(result: list[bytes]) -> None:
    """Validate each line has exactly three non-empty tab-separated columns."""
    for line in result:
        decoded = line.decode("utf-8").rstrip("\n")
        columns = decoded.split("\t")
        assert len(columns) == 3, (
            f"expected 3 tab-separated columns, got {len(columns)}: {decoded!r}"
        )
        for i, col in enumerate(columns):
            assert col != "", f"column {i} must not be empty in: {decoded!r}"


class TestMain:
    """Tests for main() function."""

    @pytest.mark.parametrize(
        ("manifest_content", "expected_code"),
        [
            (VALID_MANIFEST, 0),
            (DUPLICATE_MANIFEST, 1),
        ],
        ids=["valid_manifest", "duplicate_manifest"],
    )
    def test_main_exit_code(
        self,
        tmp_path: Path,
        write_manifest: Callable[[Path, str], Path],
        manifest_content: str,
        expected_code: int,
    ) -> None:
        """main returns 0 for a valid manifest and 1 for a manifest with duplicate packages."""
        manifest = write_manifest(tmp_path / "manifest.toml", manifest_content)

        with patch(
            "sys.argv",
            ["dependency_binaries_manifest.py", str(manifest)],
        ):
            result = main()

        assert result == expected_code, (
            f"expected exit code {expected_code}, got {result}"
        )

    def test_main_writes_to_output_file(
        self, tmp_path: Path, write_manifest: Callable[[Path, str], Path]
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

    def test_main_missing_file_raises(self, tmp_path: Path) -> None:
        """main raises an exception for a non-existent manifest file."""
        missing = tmp_path / "does_not_exist.toml"

        with patch(
            "sys.argv",
            ["dependency_binaries_manifest.py", str(missing)],
        ), pytest.raises(FileNotFoundError, match="does_not_exist\\.toml"):
            main()

    def test_main_uses_default_manifest_path(self) -> None:
        """main uses default manifest path when no argument given."""
        mock_collect = MagicMock(return_value=[b"test\ttest\t1.0\n"])

        with patch("sys.argv", ["dependency_binaries_manifest.py"]), patch.object(
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
        expected_path = Path("installer/dependency-binaries.toml")
        assert manifest_path == expected_path, (
            f"expected default manifest path {expected_path}, got {manifest_path}"
        )


class TestSnapshotOutput:
    """Snapshot-style tests for TSV output against the real manifest."""

    def test_real_manifest_output_structure(self, real_manifest: Path) -> None:
        """TSV output for the real manifest has three non-empty columns per line."""
        result = _collect_manifest_lines(real_manifest)

        match result:
            case list():
                pass
            case _:
                pytest.fail(f"expected list, got {type(result)}")

        assert len(result) >= 1, f"expected at least 1 entry, got {len(result)}"
        _assert_three_non_empty_tsv_columns(result)

    def test_real_manifest_round_trip(
        self, tmp_path: Path, real_manifest: Path
    ) -> None:
        """The real manifest can be read and written to a file."""
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

        # Convert lines to bytes for consistency with the helper
        result_bytes = [(line + "\n").encode("utf-8") for line in lines]
        _assert_three_non_empty_tsv_columns(result_bytes)
