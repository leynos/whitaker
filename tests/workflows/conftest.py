"""Shared fixtures for workflow-level tests."""

from __future__ import annotations

import importlib.util
import types
from pathlib import Path

import pytest

from tests.workflows.workflow_test_helpers import WORKFLOW_PATH


def load_script_module(script_name: str) -> types.ModuleType:
    """Load a script from installer/scripts/ as a module via importlib.

    This helper avoids sys.path mutation and provides a stable import
    mechanism for scripts located outside the package hierarchy.

    Parameters
    ----------
    script_name : str
        The name of the script file (e.g., "dependency_binaries_manifest.py").

    Returns
    -------
    module
        The loaded module with all public APIs.

    Raises
    ------
    ImportError
        If the module spec cannot be created or the loader is unavailable.
    """
    script_path = (
        Path(__file__).resolve().parents[2]
        / "installer"
        / "scripts"
        / script_name
    )
    module_name = script_name.replace(".py", "")
    spec = importlib.util.spec_from_file_location(module_name, script_path)
    if spec is None or spec.loader is None:
        raise ImportError(f"Failed to load module spec from {script_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def write_manifest():
    """Fixture that returns a helper to write TOML manifests to disk.

    Returns
    -------
    callable
        A function that takes (path: Path, content: str) and writes the
        content to path, returning the path for convenience.
    """
    def _write(path: Path, content: str) -> Path:
        """Write a TOML manifest to *path* and return it."""
        path.write_text(content, encoding="utf-8")
        return path
    return _write


@pytest.fixture
def real_manifest() -> Path:
    """Return the path to the real dependency-binaries.toml manifest.

    Returns
    -------
    Path
        Absolute path to installer/dependency-binaries.toml in the project root.
    """
    return (
        Path(__file__).resolve().parents[2]
        / "installer"
        / "dependency-binaries.toml"
    )


@pytest.fixture
def workflow_text() -> str:
    """Return rolling-release workflow YAML as text."""
    return WORKFLOW_PATH.read_text(encoding="utf-8")
