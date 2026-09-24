"""Verify the published installer boundary before its common crate is released.

The source workspace uses a path dependency, but Cargo verifies a packaged
installer against the registry requirement. Stage both packages in a copied
workspace so a version bump can be reviewed before either crate is published.
"""

from __future__ import annotations

import json
import os
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[1]
TOOLCHAIN = "+1.85.0"


def _has_package_override(package: dict[str, object]) -> bool:
    """Return whether package metadata declares a Cargo override table."""
    return "patch" in package or "replace" in package


def _contains_local_root(contents: str, roots: tuple[Path, ...]) -> bool:
    """Return whether a manifest leaks an absolute local root."""
    return any(str(root) in contents for root in roots)


def _validate_packaged_manifest(manifest: Path, version: str, scratch: Path) -> None:
    """Reject a packaged installer whose common dependency leaks local policy.

    Cargo normalizes ``Cargo.toml`` in the archive, so inspect that copy rather
    than the workspace source manifest. A published consumer must see only a
    registry version requirement for ``whitaker-common``.
    """
    contents = manifest.read_text(encoding="utf-8")
    package = tomllib.loads(contents)
    dependency = package.get("dependencies", {}).get("whitaker-common")
    if dependency != {"version": version}:
        raise ValueError(
            "packaged whitaker-common dependency is not registry-only "
            f"{version}: {dependency!r}"
        )
    if _has_package_override(package):
        raise ValueError(
            "packaged installer manifest contains a local dependency override"
        )
    local_roots = (scratch, manifest.parent.parent)
    if _contains_local_root(contents, local_roots):
        raise ValueError(
            "packaged installer manifest contains a local dependency override"
        )


def _copy_workspace(destination: Path) -> None:
    """Copy tracked and untracked source inputs without an ambient Cargo cache.

    Ignored build outputs and Git internals stay out of the staged workspace;
    untracked source files remain available when a release bump is being gated.
    """
    result = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=REPOSITORY,
        check=True,
        capture_output=True,
    )
    for raw_name in result.stdout.split(b"\0"):
        if not raw_name:
            continue
        relative = Path(os.fsdecode(raw_name))
        source = REPOSITORY / relative
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target, follow_symlinks=False)


def _cargo(cargo: list[str], directory: Path, target: Path, *args: str) -> None:
    """Run one Cargo 1.85 stage with the shared package cache and isolated target."""
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    subprocess.run(
        [*cargo, TOOLCHAIN, *args],
        cwd=directory,
        env=environment,
        check=True,
        stdout=subprocess.DEVNULL if "metadata" in args else None,
    )


def _one_archive(target: Path, crate: str) -> Path:
    """Return the one package produced for a crate or fail on stale output."""
    archives = list((target / "package").glob(f"{crate}-*.crate"))
    if len(archives) != 1:
        raise ValueError(f"expected one {crate} archive, found {len(archives)}")
    return archives[0]


def _extract_archive(archive: Path, destination: Path, crate: str) -> Path:
    """Extract a Cargo-created archive and select its single crate directory."""
    destination.mkdir(parents=True)
    with tarfile.open(archive, "r:gz") as package:
        package.extractall(destination, filter="data")
    roots = list(destination.glob(f"{crate}-*"))
    if len(roots) != 1 or not roots[0].is_dir():
        raise ValueError(f"expected one extracted {crate} directory")
    return roots[0]


def _main() -> None:
    """Package and install the installer under Rust 1.85 using staged common."""
    cargo = shlex.split(os.environ.get("CARGO", "cargo"))
    if not cargo:
        raise ValueError("CARGO must name an executable")
    build_root = REPOSITORY / "target"
    build_root.mkdir(exist_ok=True)
    with (
        tempfile.TemporaryDirectory(prefix="installer-msrv-", dir=build_root) as temporary,
        tempfile.TemporaryDirectory(prefix="whitaker-installer-source-") as source_scratch,
    ):
        scratch = Path(temporary)
        workspace = scratch / "workspace"
        workspace.mkdir()
        _copy_workspace(workspace)
        target = scratch / "cargo-target"

        # Resolve and fetch dependencies before the offline package stages;
        # CI's registry cache need not already contain every locked crate.
        _cargo(cargo, workspace, target, "metadata", "--format-version", "1")
        _cargo(cargo, workspace, target, "fetch", "--locked")
        _cargo(
            cargo, workspace, target, "package", "--locked", "--offline",
            "--allow-dirty", "-p", "whitaker-common",
        )
        common_archive = _one_archive(target, "whitaker-common")
        version = common_archive.name.removeprefix(
            "whitaker-common-"
        ).removesuffix(".crate")
        common_root = _extract_archive(
            common_archive, scratch / "staged-common", "whitaker-common"
        )

        # This config is supplied only to Cargo invocations in this gate. It
        # updates the scratch lock for the verifier without changing a source
        # manifest or affecting the archive that users download.
        config = scratch / "common-patch.toml"
        path_literal = json.dumps(str(common_root))
        config.write_text(
            f"[patch.crates-io]\nwhitaker-common = {{ path = {path_literal} }}\n",
            encoding="utf-8",
        )
        selection = ("--config", str(config))
        _cargo(
            cargo, workspace, target, *selection, "metadata", "--offline",
            "--format-version", "1",
        )
        _cargo(
            cargo, workspace, target, *selection, "package", "--locked",
            "--offline", "--allow-dirty", "-p", "whitaker-installer",
        )
        installer_archive = _one_archive(target, "whitaker-installer")
        installer_root = _extract_archive(
            installer_archive,
            Path(source_scratch) / "extracted-installer",
            "whitaker-installer",
        )
        _validate_packaged_manifest(installer_root / "Cargo.toml", version, scratch)

        _cargo(
            cargo, installer_root, target, *selection, "install", "--locked",
            "--offline", "--path", str(installer_root), "--root",
            str(scratch / "install-root"),
        )
        binary = scratch / "install-root" / "bin" / "whitaker-installer"
        if os.name == "nt":
            binary = binary.with_suffix(".exe")
        result = subprocess.run(
            [str(binary), "--version"], check=True, capture_output=True, text=True
        )
        expected = f"whitaker-installer {version}"
        if result.stdout.strip() != expected:
            raise ValueError(
                f"installed version {result.stdout.strip()!r} != {expected!r}"
            )


if __name__ == "__main__":
    try:
        _main()
    except (
        OSError, ValueError, subprocess.CalledProcessError, tarfile.TarError
    ) as error:
        print(f"installer MSRV check failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
