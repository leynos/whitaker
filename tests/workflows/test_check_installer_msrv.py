"""Keep the staged MSRV package gate bound to publishable Cargo metadata.

The full gate builds the real packaged installer under Rust 1.85. These fast
controls catch a leaked local override and lost Make routing before that build.
"""

from __future__ import annotations

import importlib.util
import subprocess
from pathlib import Path

import pytest

REPOSITORY = Path(__file__).resolve().parents[2]
SCRIPT = REPOSITORY / "scripts" / "check_installer_msrv.py"


def _validator():
    """Load the gate's manifest validator without executing its Cargo stages."""
    spec = importlib.util.spec_from_file_location("check_installer_msrv", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module._validate_packaged_manifest


def _manifest(tmp_path: Path, dependency: str, addition: str = "") -> Path:
    """Write one normalized archive manifest candidate for the validator."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "whitaker-installer"\nversion = "0.2.9"\n'
        f"[dependencies.whitaker-common]\n{dependency}\n{addition}",
        encoding="utf-8",
    )
    return manifest


def test_registry_only_common_requirement_passes(tmp_path: Path) -> None:
    """Accept the version-only dependency Cargo publishes to consumers."""
    manifest = _manifest(tmp_path, 'version = "0.2.9"')
    _validator()(manifest, "0.2.9", tmp_path)


@pytest.mark.parametrize(
    "dependency,addition",
    [
        ('version = "0.2.8"', ""),
        ('version = "0.2.9"\npath = "../common"', ""),
        ('version = "0.2.9"\ngit = "https://example.invalid/common.git"', ""),
        (
            'version = "0.2.9"',
            '[patch.crates-io]\nwhitaker-common = { path = "../common" }\n',
        ),
        (
            'version = "0.2.9"',
            '[replace]\n"whitaker-common:0.2.9" = { path = "../common" }\n',
        ),
    ],
    ids=[
        "wrong-version",
        "path-dependency",
        "git-dependency",
        "patch-table",
        "replace-table",
    ],
)
def test_local_override_or_wrong_version_fails(
    tmp_path: Path, dependency: str, addition: str
) -> None:
    """Reject local-only metadata that would hide a failed registry release."""
    manifest = _manifest(tmp_path, dependency, addition)
    with pytest.raises(ValueError):
        _validator()(manifest, "0.2.9", tmp_path)


def test_scratch_path_in_archive_manifest_fails(tmp_path: Path) -> None:
    """Reject an absolute gate path even outside the dependency table."""
    manifest = _manifest(
        tmp_path,
        'version = "0.2.9"',
        f'[package.metadata.gate]\nsource = "{tmp_path}"\n',
    )
    with pytest.raises(ValueError, match="local dependency override"):
        _validator()(manifest, "0.2.9", tmp_path)


def test_extracted_source_path_in_archive_manifest_fails(tmp_path: Path) -> None:
    """Reject a leaked path to the external extraction scratch directory."""
    extracted = tmp_path / "extracted-installer"
    manifest_dir = extracted / "whitaker-installer-0.2.9"
    manifest_dir.mkdir(parents=True)
    manifest = _manifest(
        manifest_dir,
        'version = "0.2.9"',
        f'[package.metadata.gate]\nsource = "{extracted}"\n',
    )
    with pytest.raises(ValueError, match="local dependency override"):
        _validator()(manifest, "0.2.9", tmp_path / "build")


def test_make_routes_injected_cargo_to_the_gate() -> None:
    """Keep the repository's injectable Cargo setting in the Make contract."""
    result = subprocess.run(
        ["make", "--dry-run", "installer-msrv-check", "CARGO=probe-cargo"],
        cwd=REPOSITORY,
        check=True,
        capture_output=True,
        text=True,
    )
    assert (
        'CARGO="probe-cargo" python3 scripts/check_installer_msrv.py'
        in result.stdout
    )
