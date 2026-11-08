"""Verify the published installer boundary before its common crate is released.

The source workspace uses a path dependency, but Cargo verifies a packaged
installer against the registry requirement. Stage both packages in a copied
workspace so a version bump can be reviewed before either crate is published.
"""

from __future__ import annotations

import contextlib
import json
import os
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from collections.abc import Iterator
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[1]
INSTALLER_MANIFEST = REPOSITORY / "installer" / "Cargo.toml"


def _toolchain() -> str:
    """Return the Cargo toolchain selector for the installer's declared MSRV.

    `rust-version` in `installer/Cargo.toml` is the single place the minimum
    lives; this check, and the CI step that installs the toolchain, follow it.
    A two-part version such as `1.88` selects `+1.88.0`.
    """
    manifest = tomllib.loads(INSTALLER_MANIFEST.read_text(encoding="utf-8"))
    version = manifest["package"]["rust-version"]
    return f"+{version}.0" if version.count(".") == 1 else f"+{version}"


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
    """Run one Cargo stage under the declared MSRV with the shared package cache and isolated target."""
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    subprocess.run(
        [*cargo, _toolchain(), *args],
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


@contextlib.contextmanager
def _scratch_root() -> Iterator[Path]:
    """Yield the scratch root this check builds in, and remove it on exit.

    The Makefile names a fixed directory (`INSTALLER_MSRV_SCRATCH`) in the
    user's private cache: sccache keys compilations on absolute paths, so a
    fresh random directory per run would miss every time. It is cleared first
    and removed on exit. A direct run falls back to a random directory under
    the ignored `target/`. Either way the root holds the copied workspace and
    the extracted installer as siblings, so Cargo never walks up from the
    installer into a workspace root.
    """
    fixed = os.environ.get("INSTALLER_MSRV_SCRATCH")
    if fixed:
        root = Path(fixed)
        shutil.rmtree(root, ignore_errors=True)
        root.mkdir(parents=True)
        try:
            yield root
        finally:
            shutil.rmtree(root, ignore_errors=True)
        return
    build_root = REPOSITORY / "target"
    build_root.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="installer-msrv-", dir=build_root) as temporary:
        yield Path(temporary)


def _main() -> None:
    """Package and install the installer under its declared MSRV using staged common."""
    cargo = shlex.split(os.environ.get("CARGO", "cargo"))
    if not cargo:
        raise ValueError("CARGO must name an executable")
    with _scratch_root() as scratch:
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
            scratch / "installer-source" / "extracted-installer",
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
