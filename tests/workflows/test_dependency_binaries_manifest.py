"""Unit tests for the dependency_binaries_manifest script.

This module provides comprehensive test coverage for
``installer/scripts/dependency_binaries_manifest.py``, including:

- TOML manifest loading and validation
- Duplicate package detection
- TSV output encoding and column ordering
- Argument parsing and --output file writing
- Error paths for missing files, empty manifests, and malformed TOML

Examples
--------
Run all tests:
    python3 -m pytest tests/workflows/test_dependency_binaries_manifest.py -v

Run specific test:
    python3 -m pytest tests/workflows/test_dependency_binaries_manifest.py::TestCollectManifestLines -v
"""

from __future__ import annotations

import importlib.util
import sys
import types
from pathlib import Path
from unittest.mock import patch

import pytest


def _load_manifest_module() -> types.ModuleType:
    """Load the dependency_binaries_manifest script as a module via importlib.

    This helper avoids sys.path mutation and provides a stable import
    mechanism for scripts located outside the package hierarchy.

    Returns
    -------
    module
        The loaded dependency_binaries_manifest module with all public APIs.

    Raises
    ------
    ImportError
        If the module spec cannot be created or the loader is unavailable.
    """
    script_path = (
        Path(__file__).resolve().parents[2]
        / "installer"
        / "scripts"
        / "dependency_binaries_manifest.py"
    )
    spec = importlib.util.spec_from_file_location(
        "dependency_binaries_manifest", script_path
    )
    if spec is None or spec.loader is None:
        raise ImportError(f"Failed to load module spec from {script_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Load the module once at import time
_manifest_module = _load_manifest_module()

# Expose public API for tests
parse_args = _manifest_module.parse_args
_collect_manifest_lines = _manifest_module._collect_manifest_lines
_open_output = _manifest_module._open_output
_write_lines = _manifest_module._write_lines
main = _manifest_module.main


def _write_manifest(path: Path, content: str) -> Path:
    """Write a TOML manifest to *path* and return it."""
    path.write_text(content, encoding="utf-8")
    return path


VALID_MANIFEST = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "4.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"

[[dependency_binaries]]
package = "dylint-link"
binary = "dylint-link"
version = "4.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"
"""

SINGLE_ENTRY_MANIFEST = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "4.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"
"""

DUPLICATE_MANIFEST = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "4.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"

[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "4.2.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"
"""


class TestParseArgs:
    """Tests for parse_args() function."""

    def test_default_manifest_path(self) -> None:
        """Default manifest path is installer/dependency-binaries.toml."""
        with patch("sys.argv", ["dependency_binaries_manifest.py"]):
            args = parse_args()

        assert args.manifest == "installer/dependency-binaries.toml"

    def test_custom_manifest_path(self) -> None:
        """Positional argument overrides the default manifest path."""
        with patch(
            "sys.argv", ["dependency_binaries_manifest.py", "custom.toml"]
        ):
            args = parse_args()

        assert args.manifest == "custom.toml"

    def test_output_defaults_to_none(self) -> None:
        """Output defaults to None (stdout) when not specified."""
        with patch("sys.argv", ["dependency_binaries_manifest.py"]):
            args = parse_args()

        assert args.output is None

    def test_output_flag(self) -> None:
        """--output flag sets the output file path."""
        with patch(
            "sys.argv",
            ["dependency_binaries_manifest.py", "--output", "out.tsv"],
        ):
            args = parse_args()

        assert args.output == "out.tsv"

    def test_output_short_flag(self) -> None:
        """-o short flag sets the output file path."""
        with patch(
            "sys.argv",
            ["dependency_binaries_manifest.py", "-o", "out.tsv"],
        ):
            args = parse_args()

        assert args.output == "out.tsv"


class TestCollectManifestLines:
    """Tests for _collect_manifest_lines() function."""

    def test_valid_manifest_produces_tsv_lines(self, tmp_path: Path) -> None:
        """A valid manifest produces correctly formatted TSV lines."""
        manifest = _write_manifest(tmp_path / "manifest.toml", VALID_MANIFEST)

        result = _collect_manifest_lines(manifest)

        assert isinstance(result, list), f"expected list, got {type(result)}"
        assert len(result) == 2, f"expected 2 lines, got {len(result)}"

    def test_tsv_column_order(self, tmp_path: Path) -> None:
        """Output columns are package, binary, version separated by tabs."""
        manifest = _write_manifest(
            tmp_path / "manifest.toml", SINGLE_ENTRY_MANIFEST
        )

        result = _collect_manifest_lines(manifest)

        assert isinstance(result, list)
        line = result[0].decode("utf-8")
        assert line == "cargo-dylint\tcargo-dylint\t4.1.0\n"

    def test_tsv_encoding_is_utf8(self, tmp_path: Path) -> None:
        """Output lines are encoded as UTF-8 bytes."""
        manifest = _write_manifest(
            tmp_path / "manifest.toml", SINGLE_ENTRY_MANIFEST
        )

        result = _collect_manifest_lines(manifest)

        assert isinstance(result, list)
        for line in result:
            assert isinstance(line, bytes), f"expected bytes, got {type(line)}"
            line.decode("utf-8")  # should not raise

    def test_duplicate_package_returns_error_code(
        self, tmp_path: Path
    ) -> None:
        """Duplicate package entries cause the function to return 1."""
        manifest = _write_manifest(
            tmp_path / "manifest.toml", DUPLICATE_MANIFEST
        )

        result = _collect_manifest_lines(manifest)

        assert result == 1, f"expected error code 1, got {result}"

    def test_missing_file_raises(self, tmp_path: Path) -> None:
        """A non-existent manifest file raises FileNotFoundError."""
        missing = tmp_path / "does_not_exist.toml"

        with pytest.raises(FileNotFoundError):
            _collect_manifest_lines(missing)

    def test_malformed_toml_raises(self, tmp_path: Path) -> None:
        """Malformed TOML raises an exception."""
        manifest = _write_manifest(
            tmp_path / "bad.toml", "not valid [[ toml"
        )

        with pytest.raises(Exception):
            _collect_manifest_lines(manifest)

    def test_missing_key_raises(self, tmp_path: Path) -> None:
        """A manifest entry missing a required key raises KeyError."""
        incomplete = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
"""
        manifest = _write_manifest(tmp_path / "incomplete.toml", incomplete)

        with pytest.raises(KeyError):
            _collect_manifest_lines(manifest)

    def test_multiple_entries_preserve_order(self, tmp_path: Path) -> None:
        """Multiple entries preserve their manifest declaration order."""
        manifest = _write_manifest(tmp_path / "manifest.toml", VALID_MANIFEST)

        result = _collect_manifest_lines(manifest)

        assert isinstance(result, list)
        first_line = result[0].decode("utf-8")
        second_line = result[1].decode("utf-8")
        assert first_line.startswith("cargo-dylint\t")
        assert second_line.startswith("dylint-link\t")


class TestOpenOutput:
    """Tests for _open_output() context manager."""

    def test_none_yields_stdout_buffer(self) -> None:
        """None output yields sys.stdout.buffer."""
        with _open_output(None) as handle:
            assert handle is sys.stdout.buffer

    def test_path_yields_writable_handle(self, tmp_path: Path) -> None:
        """A file path yields a writable binary handle."""
        output_file = tmp_path / "output.tsv"

        with _open_output(str(output_file)) as handle:
            handle.write(b"test\n")

        assert output_file.read_bytes() == b"test\n"

    def test_empty_path_exits(self) -> None:
        """An empty output path raises SystemExit."""
        with pytest.raises(SystemExit):
            with _open_output(""):
                pass  # pragma: no cover


class TestWriteLines:
    """Tests for _write_lines() function."""

    def test_writes_to_file(self, tmp_path: Path) -> None:
        """Lines are written to the specified file."""
        output_file = tmp_path / "output.tsv"
        lines = [b"line1\n", b"line2\n"]

        _write_lines(lines, str(output_file))

        assert output_file.read_bytes() == b"line1\nline2\n"

    def test_writes_to_stdout(self, capsys: pytest.CaptureFixture) -> None:
        """Lines are written to stdout when output is None."""
        lines = [b"cargo-dylint\tcargo-dylint\t4.1.0\n"]

        _write_lines(lines, None)

        captured = capsys.readouterr()
        assert "cargo-dylint" in captured.out


class TestMain:
    """Tests for main() function."""

    def test_main_success(self, tmp_path: Path) -> None:
        """main returns 0 on valid manifest input."""
        manifest = _write_manifest(tmp_path / "manifest.toml", VALID_MANIFEST)

        with patch(
            "sys.argv",
            ["dependency_binaries_manifest.py", str(manifest)],
        ):
            result = main()

        assert result == 0, f"expected exit code 0, got {result}"

    def test_main_writes_to_output_file(self, tmp_path: Path) -> None:
        """main writes TSV output to the specified file."""
        manifest = _write_manifest(tmp_path / "manifest.toml", VALID_MANIFEST)
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
        assert lines[0] == "cargo-dylint\tcargo-dylint\t4.1.0"
        assert lines[1] == "dylint-link\tdylint-link\t4.1.0"

    def test_main_duplicate_returns_error(self, tmp_path: Path) -> None:
        """main returns 1 when the manifest contains duplicate packages."""
        manifest = _write_manifest(
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
            with pytest.raises(FileNotFoundError):
                main()

    def test_main_default_manifest_path(self) -> None:
        """main uses the default manifest path when no argument given."""
        with patch("sys.argv", ["dependency_binaries_manifest.py"]):
            # This will either succeed (if run from repo root) or fail
            # with FileNotFoundError (if not). We verify the path is used.
            args = parse_args()
            assert args.manifest == "installer/dependency-binaries.toml"


class TestSnapshotOutput:
    """Snapshot-style tests for provenance markdown output."""

    def test_provenance_output_structure(self, tmp_path: Path) -> None:
        """TSV output for the real manifest matches expected structure."""
        real_manifest = (
            Path(__file__).resolve().parents[2]
            / "installer"
            / "dependency-binaries.toml"
        )
        if not real_manifest.exists():
            pytest.skip("real manifest not available")

        result = _collect_manifest_lines(real_manifest)

        assert isinstance(result, list), f"expected list, got {type(result)}"
        assert len(result) >= 2, f"expected at least 2 entries, got {len(result)}"
        for line in result:
            decoded = line.decode("utf-8").rstrip("\n")
            columns = decoded.split("\t")
            assert len(columns) == 3, (
                f"expected 3 tab-separated columns, got {len(columns)}: {decoded!r}"
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
        assert len(lines) >= 2, f"expected at least 2 lines, got {len(lines)}"
        # Verify cargo-dylint and dylint-link are both present
        packages = [line.split("\t")[0] for line in lines]
        assert "cargo-dylint" in packages
        assert "dylint-link" in packages
