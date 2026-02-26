"""Shared fixtures for workflow-level tests."""

from __future__ import annotations

import pytest

from tests.workflows.workflow_test_helpers import WORKFLOW_PATH


@pytest.fixture
def workflow_text() -> str:
    """Return rolling-release workflow YAML as text."""
    return WORKFLOW_PATH.read_text(encoding="utf-8")
