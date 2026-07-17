"""Validate the installer release dry-run Makefile contract."""

from __future__ import annotations

from pathlib import Path

import pytest

MAKEFILE_PATH = Path(__file__).resolve().parents[2] / "Makefile"
TARGET_NAME = "release-installer-dry-run"


@pytest.fixture(scope="module")
def release_installer_dry_run_recipe() -> str:
    """Return the recipe body for the installer release dry-run target."""
    lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    target_header = f"{TARGET_NAME}:"
    try:
        target_index = next(
            index for index, line in enumerate(lines) if line.startswith(target_header)
        )
    except StopIteration:
        pytest.fail(f"Makefile must define {TARGET_NAME}")

    recipe_lines: list[str] = []
    for line in lines[target_index + 1 :]:
        if line and not line.startswith(("\t", " ")):
            break
        recipe_lines.append(line)
    return "\n".join(recipe_lines)


def test_release_dry_run_checks_required_tools(
    release_installer_dry_run_recipe: str,
) -> None:
    """Ensure missing shell tools fail before build work starts."""
    assert "PYTHON=$$(command -v python3 || command -v python || true)" in (
        release_installer_dry_run_recipe
    ), "release-installer-dry-run must resolve python3 or python"
    assert "Install python3 or python to run release-installer-dry-run" in (
        release_installer_dry_run_recipe
    ), "release-installer-dry-run must explain missing Python tools"
    assert '[ -n "$(CARGO)" ] || { echo "Install cargo to run release-installer-dry-run"; exit 1; }' in (
        release_installer_dry_run_recipe
    ), "release-installer-dry-run must validate the Cargo command"
    assert "for tool in awk jq mktemp rustc; do" in (
        release_installer_dry_run_recipe
    ), "release-installer-dry-run must validate required shell tools"
    assert "Install $$tool to run release-installer-dry-run" in (
        release_installer_dry_run_recipe
    ), "release-installer-dry-run must explain missing shell tools"


def test_release_dry_run_detects_host_triple_with_rustc(
    release_installer_dry_run_recipe: str,
) -> None:
    """Ensure archive naming and build output use the detected host triple."""
    assert "HOST_TRIPLE=$$(rustc -vV | awk -F ': ' '/host:/ {print $$2}')" in (
        release_installer_dry_run_recipe
    ), "HOST_TRIPLE detection must be emitted"
    assert (
        "--target \"$$HOST_TRIPLE\"" in release_installer_dry_run_recipe
    ), "builds must target HOST_TRIPLE"


def test_release_dry_run_applies_cargo_locked_to_metadata(
    release_installer_dry_run_recipe: str,
) -> None:
    """Ensure metadata uses the same locked dependency graph as the builds."""
    assert "$(CARGO) metadata $(CARGO_LOCKED) --manifest-path installer/Cargo.toml" in (
        release_installer_dry_run_recipe
    ), "release-installer-dry-run metadata must honour CARGO_LOCKED"


def test_release_dry_run_builds_binaries_in_target_scoped_tree(
    release_installer_dry_run_recipe: str,
) -> None:
    """Ensure ambient Cargo target settings cannot split binary outputs."""
    assert (
        "--target \"$$HOST_TRIPLE\"" in release_installer_dry_run_recipe
    ), "builds must target HOST_TRIPLE"
    assert (
        "whitaker-installer" in release_installer_dry_run_recipe
    ), "builds must include the whitaker-installer package"
    assert (
        "whitaker-package-installer" in release_installer_dry_run_recipe
    ), "builds must include the whitaker-package-installer binary"
    assert (
        'INSTALLER_BIN="target/$$HOST_TRIPLE/release/whitaker-installer"'
        in release_installer_dry_run_recipe
    ), "INSTALLER_BIN must use the HOST_TRIPLE target tree"
    assert (
        'PACKAGER="./target/$$HOST_TRIPLE/release/whitaker-package-installer"'
        in release_installer_dry_run_recipe
    ), "PACKAGER must use the HOST_TRIPLE target tree"


def test_release_dry_run_uses_platform_archive_names(
    release_installer_dry_run_recipe: str,
) -> None:
    """Ensure Windows and non-Windows archives use the expected suffixes."""
    assert (
        'ARCHIVE_GLOB="$$DIST_DIR/*.zip"' in release_installer_dry_run_recipe
    ), "Windows branch must emit .zip archive glob"
    assert (
        'ARCHIVE_GLOB="$$DIST_DIR/*.tgz"' in release_installer_dry_run_recipe
    ), "non-Windows branch must emit .tgz archive glob"
    assert (
        'INSTALLER_BIN="target/$$HOST_TRIPLE/release/whitaker-installer.exe"'
        in release_installer_dry_run_recipe
    ), "Windows installer path must use the .exe suffix"
    assert (
        'PACKAGER="./target/$$HOST_TRIPLE/release/whitaker-package-installer.exe"'
        in release_installer_dry_run_recipe
    ), "Windows packager path must use the .exe suffix"


def test_release_dry_run_generates_and_validates_checksums(
    release_installer_dry_run_recipe: str,
) -> None:
    """Ensure archive and checksum creation are both validated."""
    assert '"$$PYTHON" scripts/generate_checksums.py "$$DIST_DIR"' in (
        release_installer_dry_run_recipe
    ), "checksum generation and validation must be invoked"
    assert "Expected installer archive matching $$ARCHIVE_GLOB" in (
        release_installer_dry_run_recipe
    ), "release dry run must validate archive creation"
    assert "Expected installer checksum in $$DIST_DIR" in (
        release_installer_dry_run_recipe
    ), "release dry run must validate checksum creation"
