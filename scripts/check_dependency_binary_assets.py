#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9", "plumbum"]
# ///
"""Decide whether the rolling dependency binaries need rebuilding.

The rolling-release gate builds dependency binaries only when the manifest
changes in the triggering push. That alone cannot recover from a failed
publish, so this probe compares the release's asset list against every
archive the manifest implies and requests a rebuild when any is absent
(issue #288). The result is written as ``should_build=true|false`` to the
GitHub Actions output file, or to stdout when run outside Actions.

Run it from the repository root:

    scripts/check_dependency_binary_assets.py --release rolling
"""

from __future__ import annotations

import tomllib
import typing as typ
from pathlib import Path

import cyclopts
from cyclopts import App, Parameter
from plumbum import local

app = App(config=cyclopts.config.Env("INPUT_", command=False))

#: Targets whose archives the release workflows publish. Windows targets
#: package as ``.zip``; every other target packages as ``.tgz``.
ARCHIVE_TARGETS: tuple[str, ...] = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)


def expected_assets(
    manifest_path: Path, targets: typ.Sequence[str] = ARCHIVE_TARGETS
) -> list[str]:
    """Return every asset name the manifest implies for the targets.

    Each archive is accompanied by its ``.sha256`` sidecar: the installer
    downloads and verifies the checksum before accepting an archive, so a
    release missing a sidecar is as broken as one missing the archive.
    """
    manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    assets: list[str] = []
    for entry in manifest.get("dependency_binaries", []):
        package = entry["package"]
        version = entry["version"]
        for target in targets:
            extension = "zip" if "windows" in target else "tgz"
            archive = f"{package}-{target}-v{version}.{extension}"
            assets.append(archive)
            assets.append(f"{archive}.sha256")
    return assets


def release_asset_names(release: str) -> list[str] | None:
    """Return the release's asset names, or ``None`` when unreadable."""
    gh = local["gh"]
    return_code, stdout, _stderr = gh[
        "release", "view", release, "--json", "assets", "--jq", ".assets[].name"
    ].run(retcode=None)
    if return_code != 0:
        return None
    return [line for line in stdout.splitlines() if line]


def missing_assets(expected: typ.Sequence[str], present: typ.Sequence[str]) -> list[str]:
    """Return the expected assets absent from the present asset names."""
    present_set = set(present)
    return [asset for asset in expected if asset not in present_set]


def write_should_build(output_path: Path | None, *, should_build: bool) -> None:
    """Record the decision for the workflow, or stdout outside Actions."""
    line = f"should_build={'true' if should_build else 'false'}"
    if output_path is None:
        print(line)
        return
    with output_path.open("a", encoding="utf-8") as handle:
        handle.write(f"{line}\n")


@app.default
def main(
    *,
    manifest: Path = Path("installer/dependency-binaries.toml"),
    release: str = "rolling",
    github_output: typ.Annotated[
        Path | None, Parameter(env_var="GITHUB_OUTPUT")
    ] = None,
) -> None:
    """Probe the release for the manifest's archives and record the verdict."""
    present = release_asset_names(release)
    if present is None:
        print(f"Release {release} not readable; rebuilding dependency binaries.")
        write_should_build(github_output, should_build=True)
        return
    missing = missing_assets(expected_assets(manifest), present)
    for asset in missing:
        print(f"Missing {release} asset: {asset}")
    write_should_build(github_output, should_build=bool(missing))


if __name__ == "__main__":
    app()
