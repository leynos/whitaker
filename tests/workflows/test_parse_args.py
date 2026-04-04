"""Tests for argument parsing in dependency_binaries_manifest script.

This module tests the parse_args() function that handles command-line
arguments for the manifest script.
"""

from __future__ import annotations

from unittest.mock import patch

import pytest

from tests.workflows.conftest import load_script_module

# Load the dependency_binaries_manifest module
_manifest_module = load_script_module("dependency_binaries_manifest.py")
parse_args = _manifest_module.parse_args


class TestParseArgs:
    """Tests for parse_args() function."""

    @pytest.mark.parametrize(
        ("argv", "expected_manifest", "expected_output"),
        [
            (
                ["dependency_binaries_manifest.py"],
                "installer/dependency-binaries.toml",
                None,
            ),
            (
                ["dependency_binaries_manifest.py", "custom.toml"],
                "custom.toml",
                None,
            ),
            (
                ["dependency_binaries_manifest.py", "--output", "out.tsv"],
                "installer/dependency-binaries.toml",
                "out.tsv",
            ),
            (
                ["dependency_binaries_manifest.py", "-o", "out.tsv"],
                "installer/dependency-binaries.toml",
                "out.tsv",
            ),
            (
                [
                    "dependency_binaries_manifest.py",
                    "custom.toml",
                    "--output",
                    "out.tsv",
                ],
                "custom.toml",
                "out.tsv",
            ),
        ],
        ids=[
            "default_manifest_and_output",
            "custom_manifest",
            "output_long_flag",
            "output_short_flag",
            "custom_manifest_and_output",
        ],
    )
    def test_argument_parsing(
        self,
        argv: list[str],
        expected_manifest: str,
        expected_output: str | None,
    ) -> None:
        """parse_args correctly handles various argument combinations."""
        with patch("sys.argv", argv):
            args = parse_args()

        assert args.manifest == expected_manifest, (
            f"expected manifest={expected_manifest!r}, got {args.manifest!r}"
        )
        assert args.output == expected_output, (
            f"expected output={expected_output!r}, got {args.output!r}"
        )
