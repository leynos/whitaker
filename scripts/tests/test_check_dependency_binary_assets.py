"""Test the rolling-release dependency-asset probe.

This suite imports ``check_dependency_binary_assets.py`` and validates the
asset-name derivation, the missing-asset computation, and the decision
output across its pure boundaries. Run it with
``uv run --with cyclopts --with plumbum pytest
scripts/tests/test_check_dependency_binary_assets.py``.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]

MANIFEST = """\
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "6.0.1"

[[dependency_binaries]]
package = "dylint-link"
binary = "dylint-link"
version = "6.0.1"
"""


@pytest.fixture(scope="module")
def probe():
    """Import the probe script as a module."""
    spec = importlib.util.spec_from_file_location(
        "check_dependency_binary_assets",
        SCRIPTS / "check_dependency_binary_assets.py",
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def manifest_path(tmp_path: Path) -> Path:
    """Write a two-tool manifest and return its path."""
    path = tmp_path / "dependency-binaries.toml"
    path.write_text(MANIFEST, encoding="utf-8")
    return path


def test_expected_assets_cover_every_tool_and_target(probe, manifest_path) -> None:
    """Every tool gains one archive per target with the right extension."""
    assets = probe.expected_assets(manifest_path)

    assert len(assets) == 2 * len(probe.ARCHIVE_TARGETS)
    assert "cargo-dylint-x86_64-unknown-linux-gnu-v6.0.1.tgz" in assets
    assert "dylint-link-x86_64-pc-windows-msvc-v6.0.1.zip" in assets
    assert not [a for a in assets if "windows" in a and a.endswith(".tgz")]


def test_expected_assets_respects_target_override(probe, manifest_path) -> None:
    """A caller-supplied target list narrows the derived assets."""
    assets = probe.expected_assets(manifest_path, ["x86_64-unknown-linux-gnu"])

    assert assets == [
        "cargo-dylint-x86_64-unknown-linux-gnu-v6.0.1.tgz",
        "dylint-link-x86_64-unknown-linux-gnu-v6.0.1.tgz",
    ]


@pytest.mark.parametrize(
    ("present", "expected_missing"),
    [
        (["a.tgz", "b.tgz"], []),
        (["a.tgz"], ["b.tgz"]),
        ([], ["a.tgz", "b.tgz"]),
    ],
    ids=["complete", "partial", "empty"],
)
def test_missing_assets(probe, present, expected_missing) -> None:
    """Missing assets are exactly the expected names not present."""
    assert probe.missing_assets(["a.tgz", "b.tgz"], present) == expected_missing


def test_write_should_build_appends_to_output_file(probe, tmp_path: Path) -> None:
    """The decision appends to the Actions output file without clobbering."""
    output = tmp_path / "github-output"
    output.write_text("earlier=value\n", encoding="utf-8")

    probe.write_should_build(output, should_build=True)
    probe.write_should_build(output, should_build=False)

    assert output.read_text(encoding="utf-8").splitlines() == [
        "earlier=value",
        "should_build=true",
        "should_build=false",
    ]


def test_write_should_build_prints_without_output_file(probe, capsys) -> None:
    """Outside Actions the decision goes to stdout."""
    probe.write_should_build(None, should_build=True)

    assert capsys.readouterr().out.strip() == "should_build=true"
