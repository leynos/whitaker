"""Unit tests for workflow test helper metadata lookups.

This module exercises the private Cargo metadata helpers directly so workflow
contract tests fail with clear messages when metadata resolution changes.
"""

from __future__ import annotations

import json
import subprocess
from unittest.mock import patch

import pytest

from tests.workflows import workflow_test_helpers


class TestCargoMetadata:
    """Tests for `_cargo_metadata()` error handling and edge cases."""

    def test_requires_cargo_on_path(self) -> None:
        """The helper fails fast when `cargo` cannot be resolved."""
        with (
            patch.object(workflow_test_helpers.shutil, "which", return_value=None),
            pytest.raises(
                AssertionError,
                match="cargo executable must be available in PATH",
            ),
        ):
            workflow_test_helpers._cargo_metadata()

    def test_reports_subprocess_failure(self) -> None:
        """The helper surfaces `cargo metadata` stderr on failure."""
        completed = subprocess.CompletedProcess(
            args=["/usr/bin/cargo", "metadata"],
            returncode=1,
            stdout="",
            stderr="metadata broke",
        )

        with (
            patch.object(
                workflow_test_helpers.shutil, "which", return_value="/usr/bin/cargo"
            ),
            patch.object(
                workflow_test_helpers.subprocess, "run", return_value=completed
            ),
            pytest.raises(
                AssertionError,
                match="cargo metadata failed while resolving workspace metadata",
            ),
        ):
            workflow_test_helpers._cargo_metadata()

    @pytest.mark.parametrize(
        ("stdout", "expected_type_name"),
        [
            (json.dumps([]), "list"),
            (json.dumps("workspace"), "str"),
        ],
        ids=["json-list", "json-string"],
    )
    def test_rejects_non_mapping_json(
        self, stdout: str, expected_type_name: str
    ) -> None:
        """The helper accepts only object-shaped `cargo metadata` output."""
        completed = subprocess.CompletedProcess(
            args=["/usr/bin/cargo", "metadata"],
            returncode=0,
            stdout=stdout,
            stderr="",
        )

        with (
            patch.object(
                workflow_test_helpers.shutil, "which", return_value="/usr/bin/cargo"
            ),
            patch.object(
                workflow_test_helpers.subprocess, "run", return_value=completed
            ),
            pytest.raises(
                AssertionError,
                match="cargo metadata must return a JSON object",
            ),
        ):
            workflow_test_helpers._cargo_metadata()

        parsed = json.loads(stdout)
        assert type(parsed).__name__ == expected_type_name

    def test_returns_parsed_workspace_metadata(self) -> None:
        """The helper returns parsed JSON for successful metadata calls."""
        payload = {"packages": [{"name": "whitaker-installer"}]}
        completed = subprocess.CompletedProcess(
            args=["/usr/bin/cargo", "metadata"],
            returncode=0,
            stdout=json.dumps(payload),
            stderr="",
        )

        with (
            patch.object(
                workflow_test_helpers.shutil, "which", return_value="/usr/bin/cargo"
            ),
            patch.object(
                workflow_test_helpers.subprocess, "run", return_value=completed
            ) as run_mock,
        ):
            metadata = workflow_test_helpers._cargo_metadata()

        assert metadata == payload
        run_mock.assert_called_once_with(
            ["/usr/bin/cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=workflow_test_helpers.REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
            timeout=workflow_test_helpers.CARGO_METADATA_TIMEOUT_SECONDS,
        )


class TestWorkspacePackageMetadata:
    """Tests for `_workspace_package_metadata()` lookups."""

    @pytest.mark.parametrize(
        ("return_value", "expected_match"),
        [
            (
                {"packages": "not-a-list"},
                "cargo metadata must include a packages list",
            ),
            (
                {"packages": [{"name": "whitaker-common"}]},
                "workspace package 'whitaker-installer' is missing",
            ),
        ],
        ids=["non-list-packages", "missing-package"],
    )
    def test_rejects_invalid_metadata(
        self, return_value: dict, expected_match: str
    ) -> None:
        """The helper rejects non-list packages and absent package names."""
        with (
            patch.object(
                workflow_test_helpers, "_cargo_metadata", return_value=return_value
            ),
            pytest.raises(AssertionError, match=expected_match),
        ):
            workflow_test_helpers._workspace_package_metadata("whitaker-installer")

    def test_returns_matching_package_and_ignores_non_dict_entries(self) -> None:
        """The helper skips malformed entries and returns the matching package."""
        package = {
            "name": "whitaker-installer",
            "targets": [{"name": "whitaker-installer", "kind": ["bin"]}],
        }
        with patch.object(
            workflow_test_helpers,
            "_cargo_metadata",
            return_value={"packages": ["invalid", {"name": 42}, package]},
        ):
            metadata = workflow_test_helpers._workspace_package_metadata(
                "whitaker-installer"
            )

        assert metadata == package
