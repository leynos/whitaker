"""Unit tests for the generate_checksums script.

This module provides comprehensive test coverage for
`scripts/generate_checksums.py`, including:

- SHA-256 computation for files of various sizes
- Archive discovery with glob patterns
- Checksum file generation
- CLI argument parsing and validation
- Error handling paths including NoArchivesFoundError

Examples
--------
Run all tests:
    python3 -m pytest tests/workflows/test_generate_checksums.py -v

Run specific test:
    python3 -m pytest tests/workflows/test_generate_checksums.py::test_compute_sha256 -v
"""

from __future__ import annotations

import importlib.util
from hashlib import sha256
from pathlib import Path
from unittest.mock import patch

import pytest


def _load_generate_checksums_module():
    """Load the generate_checksums script as a module via importlib.

    This helper avoids sys.path mutation and provides a stable import
    mechanism for scripts located outside the package hierarchy.

    Returns
    -------
    module
        The loaded generate_checksums module with all public APIs.
    """
    script_path = Path(__file__).resolve().parents[2] / "scripts" / "generate_checksums.py"
    spec = importlib.util.spec_from_file_location("generate_checksums", script_path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Load the module once at import time
_generate_checksums = _load_generate_checksums_module()

# Expose public API for tests
NoArchivesFoundError = _generate_checksums.NoArchivesFoundError
compute_sha256 = _generate_checksums.compute_sha256
find_archives = _generate_checksums.find_archives
generate_checksums = _generate_checksums.generate_checksums
main = _generate_checksums.main


class TestComputeSha256:
    """Tests for compute_sha256() function."""

    def test_compute_sha256_empty_file(self, tmp_path: Path) -> None:
        """SHA-256 of empty file is correct."""
        test_file = tmp_path / "empty.txt"
        test_file.write_bytes(b"")

        result = compute_sha256(test_file)
        expected = sha256(b"").hexdigest()

        assert result == expected
        assert len(result) == 64

    def test_compute_sha256_small_file(self, tmp_path: Path) -> None:
        """SHA-256 of small file matches hashlib reference."""
        content = b"Hello, World!"
        test_file = tmp_path / "small.txt"
        test_file.write_bytes(content)

        result = compute_sha256(test_file)
        expected = sha256(content).hexdigest()

        assert result == expected

    def test_compute_sha256_large_file(self, tmp_path: Path) -> None:
        """SHA-256 handles files larger than buffer size via streaming."""
        # Create file larger than 64KB buffer
        content = b"x" * (128 * 1024)
        test_file = tmp_path / "large.txt"
        test_file.write_bytes(content)

        result = compute_sha256(test_file)
        expected = sha256(content).hexdigest()

        assert result == expected

    def test_compute_sha256_binary_content(self, tmp_path: Path) -> None:
        """SHA-256 correctly handles binary content."""
        content = bytes(range(256))
        test_file = tmp_path / "binary.bin"
        test_file.write_bytes(content)

        result = compute_sha256(test_file)
        expected = sha256(content).hexdigest()

        assert result == expected

    def test_compute_sha256_nonexistent_file(self, tmp_path: Path) -> None:
        """compute_sha256 raises FileNotFoundError for missing files."""
        nonexistent = tmp_path / "does_not_exist.txt"

        with pytest.raises(FileNotFoundError):
            compute_sha256(nonexistent)


class TestFindArchives:
    """Tests for find_archives() function."""

    def test_find_archives_finds_tgz_files(self, tmp_path: Path) -> None:
        """find_archives discovers .tgz files."""
        (tmp_path / "archive1.tgz").write_text("content1")
        (tmp_path / "archive2.tgz").write_text("content2")

        result = find_archives(tmp_path)

        assert len(result) == 2
        assert all(path.suffix == ".tgz" for path in result)

    def test_find_archives_finds_zip_files(self, tmp_path: Path) -> None:
        """find_archives discovers .zip files."""
        (tmp_path / "archive1.zip").write_text("content1")
        (tmp_path / "archive2.zip").write_text("content2")

        result = find_archives(tmp_path)

        assert len(result) == 2
        assert all(path.suffix == ".zip" for path in result)

    def test_find_archives_finds_mixed_archives(self, tmp_path: Path) -> None:
        """find_archives discovers both .tgz and .zip files."""
        (tmp_path / "archive.tgz").write_text("tgz content")
        (tmp_path / "archive.zip").write_text("zip content")

        result = find_archives(tmp_path)

        assert len(result) == 2
        assert any(path.suffix == ".tgz" for path in result)
        assert any(path.suffix == ".zip" for path in result)

    def test_find_archives_returns_sorted_list(self, tmp_path: Path) -> None:
        """find_archives returns archives in sorted order."""
        (tmp_path / "zebra.tgz").write_text("z")
        (tmp_path / "alpha.tgz").write_text("a")
        (tmp_path / "beta.zip").write_text("b")

        result = find_archives(tmp_path)
        names = [path.name for path in result]

        assert names == ["alpha.tgz", "beta.zip", "zebra.tgz"]

    def test_find_archives_ignores_other_files(self, tmp_path: Path) -> None:
        """find_archives ignores non-archive files."""
        (tmp_path / "archive.tgz").write_text("archive")
        (tmp_path / "readme.txt").write_text("readme")
        (tmp_path / "script.py").write_text("script")
        (tmp_path / "data.json").write_text("data")

        result = find_archives(tmp_path)

        assert len(result) == 1
        assert result[0].name == "archive.tgz"

    def test_find_archives_empty_directory_raises(self, tmp_path: Path) -> None:
        """find_archives raises NoArchivesFoundError for empty directory."""
        with pytest.raises(NoArchivesFoundError):
            find_archives(tmp_path)

    def test_find_archives_no_matching_files_raises(self, tmp_path: Path) -> None:
        """find_archives raises NoArchivesFoundError when no archives match."""
        (tmp_path / "readme.txt").write_text("readme")
        (tmp_path / "script.py").write_text("script")

        with pytest.raises(NoArchivesFoundError):
            find_archives(tmp_path)

    def test_find_archives_nonexistent_directory_raises(self, tmp_path: Path) -> None:
        """find_archives raises NoArchivesFoundError for non-existent directory."""
        nonexistent = tmp_path / "does_not_exist"

        with pytest.raises(NoArchivesFoundError):
            find_archives(nonexistent)


class TestGenerateChecksums:
    """Tests for generate_checksums() function."""

    def test_generate_checksums_creates_sha256_files(self, tmp_path: Path) -> None:
        """generate_checksums creates .sha256 files for each archive."""
        content = b"test archive content"
        (tmp_path / "archive1.tgz").write_bytes(content)
        (tmp_path / "archive2.zip").write_bytes(content)

        generate_checksums(tmp_path)

        assert (tmp_path / "archive1.tgz.sha256").exists()
        assert (tmp_path / "archive2.zip.sha256").exists()

    def test_generate_checksums_content_format(self, tmp_path: Path) -> None:
        """Generated .sha256 files contain hash and filename."""
        content = b"test content"
        (tmp_path / "test.tgz").write_bytes(content)
        expected_hash = sha256(content).hexdigest()

        generate_checksums(tmp_path)

        checksum_file = tmp_path / "test.tgz.sha256"
        checksum_content = checksum_file.read_text(encoding="ascii")

        assert checksum_content == f"{expected_hash}  test.tgz\n"

    def test_generate_checksums_raises_on_empty_directory(self, tmp_path: Path) -> None:
        """generate_checksums propagates NoArchivesFoundError."""
        with pytest.raises(NoArchivesFoundError):
            generate_checksums(tmp_path)


class TestMain:
    """Tests for main() function."""

    def test_main_success_with_archives(self, tmp_path: Path) -> None:
        """main returns 0 when checksums are generated successfully."""
        (tmp_path / "archive.tgz").write_text("content")

        with patch("sys.argv", ["generate_checksums.py", str(tmp_path)]):
            result = main()

        assert result == 0
        assert (tmp_path / "archive.tgz.sha256").exists()

    def test_main_default_directory(self, tmp_path: Path) -> None:
        """main uses 'dist' as default directory when no argument given."""
        dist_dir = tmp_path / "dist"
        dist_dir.mkdir()
        (dist_dir / "archive.tgz").write_text("content")

        # Change to tmp_path to make default "dist" resolve correctly
        import os

        original_cwd = os.getcwd()
        try:
            os.chdir(tmp_path)
            with patch("sys.argv", ["generate_checksums.py"]):
                result = main()

            assert result == 0
            assert (dist_dir / "archive.tgz.sha256").exists()
        finally:
            os.chdir(original_cwd)

    def test_main_nonexistent_directory(self, tmp_path: Path) -> None:
        """main returns 1 when directory does not exist."""
        nonexistent = tmp_path / "does_not_exist"

        with patch("sys.argv", ["generate_checksums.py", str(nonexistent)]):
            result = main()

        assert result == 1

    def test_main_file_instead_of_directory(self, tmp_path: Path) -> None:
        """main returns 1 when path is a file, not a directory."""
        file_path = tmp_path / "not_a_directory.txt"
        file_path.write_text("content")

        with patch("sys.argv", ["generate_checksums.py", str(file_path)]):
            result = main()

        assert result == 1

    def test_main_no_archives_found(self, tmp_path: Path) -> None:
        """main returns 1 when no archives are found in directory."""
        with patch("sys.argv", ["generate_checksums.py", str(tmp_path)]):
            result = main()

        assert result == 1


class TestNoArchivesFoundError:
    """Tests for NoArchivesFoundError exception."""

    def test_exception_is_exception(self) -> None:
        """NoArchivesFoundError is an Exception subclass."""
        assert issubclass(NoArchivesFoundError, Exception)

    def test_exception_can_be_raised_with_path(self, tmp_path: Path) -> None:
        """NoArchivesFoundError can be raised with a path argument."""
        with pytest.raises(NoArchivesFoundError) as exc_info:
            raise NoArchivesFoundError(tmp_path)

        assert str(tmp_path) in str(exc_info.value)

    def test_exception_can_be_caught_as_generic(self) -> None:
        """NoArchivesFoundError can be caught as generic Exception."""
        try:
            raise NoArchivesFoundError("test")
        except Exception as e:  # noqa: BLE001  # Testing generic catch
            assert isinstance(e, NoArchivesFoundError)
