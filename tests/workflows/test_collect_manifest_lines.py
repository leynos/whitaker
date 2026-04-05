"""Tests for TOML manifest parsing in dependency_binaries_manifest script.

This module tests _collect_manifest_lines() which reads, validates, and
converts TOML manifests to TSV format.
"""

from __future__ import annotations

from pathlib import Path

import tomllib

import pytest

from tests.workflows.conftest import load_script_module
from tests.workflows.dependency_manifest_test_data import (
    DUPLICATE_MANIFEST,
    SINGLE_ENTRY_MANIFEST,
    VALID_MANIFEST,
)

# Load the dependency_binaries_manifest module
_manifest_module = load_script_module("dependency_binaries_manifest.py")
_collect_manifest_lines = _manifest_module._collect_manifest_lines


class TestCollectManifestLines:
    """Tests for _collect_manifest_lines() function."""

    def test_valid_manifest_produces_tsv_lines(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """A valid manifest produces correctly formatted TSV lines."""
        manifest = write_manifest(tmp_path / "manifest.toml", VALID_MANIFEST)

        result = _collect_manifest_lines(manifest)

        assert isinstance(result, list), f"expected list, got {type(result)}"
        assert len(result) == 2, f"expected 2 lines, got {len(result)}"

    def test_tsv_column_order(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """Output columns are package, binary, version separated by tabs."""
        manifest = write_manifest(
            tmp_path / "manifest.toml", SINGLE_ENTRY_MANIFEST
        )

        result = _collect_manifest_lines(manifest)

        assert isinstance(result, list), (
            f"expected list result, got {type(result)}"
        )
        line = result[0].decode("utf-8")
        assert line == "cargo-dylint\tcargo-dylint\t4.1.0\n", (
            f"expected tab-separated output, got {line!r}"
        )

    def test_tsv_encoding_is_utf8(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """Output lines are encoded as UTF-8 bytes."""
        manifest = write_manifest(
            tmp_path / "manifest.toml", SINGLE_ENTRY_MANIFEST
        )

        result = _collect_manifest_lines(manifest)

        assert isinstance(result, list), (
            f"expected list result, got {type(result)}"
        )
        for line in result:
            assert isinstance(line, bytes), (
                f"expected bytes, got {type(line)}"
            )
            try:
                line.decode("utf-8")
            except UnicodeDecodeError as e:
                pytest.fail(f"line should be valid UTF-8: {e}")

    def test_duplicate_package_returns_error_code(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """Duplicate package entries cause the function to return 1."""
        manifest = write_manifest(
            tmp_path / "manifest.toml", DUPLICATE_MANIFEST
        )

        result = _collect_manifest_lines(manifest)

        assert result == 1, f"expected error code 1, got {result}"

    def test_missing_file_raises(self, tmp_path: Path) -> None:
        """A non-existent manifest file raises FileNotFoundError."""
        missing = tmp_path / "does_not_exist.toml"

        with pytest.raises(FileNotFoundError, match="does_not_exist\\.toml"):
            _collect_manifest_lines(missing)

    def test_malformed_toml_raises(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """Malformed TOML raises a TOML decode error."""
        manifest = write_manifest(
            tmp_path / "bad.toml", "not valid [[ toml"
        )

        with pytest.raises(tomllib.TOMLDecodeError, match=r"(Invalid|Expected)"):
            _collect_manifest_lines(manifest)

    def test_missing_key_raises(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """A manifest entry missing a required key raises KeyError."""
        incomplete = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
"""
        manifest = write_manifest(tmp_path / "incomplete.toml", incomplete)

        with pytest.raises(KeyError, match="version"):
            _collect_manifest_lines(manifest)

    def test_empty_table_returns_empty_list(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """A manifest with an empty dependency_binaries array returns empty list."""
        empty_table = "dependency_binaries = []\n"
        manifest = write_manifest(tmp_path / "empty.toml", empty_table)

        result = _collect_manifest_lines(manifest)

        assert result == [], f"expected empty list, got {result}"

    def test_missing_table_raises_key_error(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """A manifest without a dependency_binaries table raises KeyError."""
        no_table = "[metadata]\nname = 'test'\n"
        manifest = write_manifest(tmp_path / "no_table.toml", no_table)

        with pytest.raises(KeyError, match="dependency_binaries"):
            _collect_manifest_lines(manifest)

    def test_multiple_entries_preserve_order(
        self, tmp_path: Path, write_manifest
    ) -> None:
        """Multiple entries preserve their manifest declaration order."""
        manifest = write_manifest(tmp_path / "manifest.toml", VALID_MANIFEST)

        result = _collect_manifest_lines(manifest)

        assert isinstance(result, list), (
            f"expected list result, got {type(result)}"
        )
        first_line = result[0].decode("utf-8")
        second_line = result[1].decode("utf-8")
        assert first_line.startswith("cargo-dylint\t"), (
            f"first entry should be cargo-dylint, got {first_line!r}"
        )
        assert second_line.startswith("dylint-link\t"), (
            f"second entry should be dylint-link, got {second_line!r}"
        )
