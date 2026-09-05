#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
"""Reject Linux binaries that require a newer glibc baseline.

The release workflow builds Linux artefacts on Ubuntu 22.04, whose glibc
baseline is ``GLIBC_2.35``. This checker reads the ELF version-needs metadata
instead of trusting the runner label, so publishing fails if a build image
silently changes.

Run through uv rather than the ambient interpreter. The x86_64 Linux legs that
call this script run on ubuntu-22.04, where ``python`` is 3.10; anything this
module later borrows from a newer standard library would fail there, at release
time, and nowhere else. Pinning the interpreter here removes that class of
failure rather than the one instance of it.

Example
-------
    scripts/check_glibc_baseline.py dist/whitaker-installer
"""

from __future__ import annotations

import argparse
import itertools
import re
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import cast

DEFAULT_MAXIMUM_GLIBC = "GLIBC_2.35"
READELF = "readelf"
GLIBC_VERSION_PATTERN = re.compile(r"\bGLIBC_(?P<major>\d+)\.(?P<minor>\d+)\b")
GlibcVersion = tuple[int, int]


class ElfInspectionError(Exception):
    """Represent a failure to inspect an input ELF binary."""


def parse_glibc_version(value: str) -> GlibcVersion:
    """Parse a ``GLIBC_X.Y`` version string into a comparable tuple.

    Example
    -------
        >>> parse_glibc_version("GLIBC_2.35")
        (2, 35)
    """
    match = GLIBC_VERSION_PATTERN.fullmatch(value)
    if match is None:
        message = f"expected GLIBC_X.Y, got {value!r}"
        raise argparse.ArgumentTypeError(message)
    return int(match["major"]), int(match["minor"])


def format_glibc_version(version: GlibcVersion) -> str:
    """Format a comparable glibc version as ``GLIBC_X.Y``.

    Example
    -------
        >>> format_glibc_version((2, 35))
        'GLIBC_2.35'
    """
    major, minor = version
    return f"GLIBC_{major}.{minor}"


def _version_needs_lines(version_info: str) -> tuple[str, ...]:
    """Return normalized lines from the ELF version-needs section.

    Example
    -------
        >>> _version_needs_lines("Version needs section '.gnu.version_r':\n Name: GLIBC_2.35")
        ('Name: GLIBC_2.35',)
    """
    lines = tuple(line.strip() for line in version_info.splitlines())
    section_start = next(
        (
            index
            for index, line in enumerate(lines)
            if line.startswith("Version needs section")
        ),
        None,
    )
    if section_start is None:
        message = "readelf output did not contain ELF version information"
        raise ElfInspectionError(message)
    section_body = lines[section_start + 1 :]
    return tuple(
        itertools.takewhile(lambda line: not line.startswith("Version "), section_body)
    )


def parse_required_glibc_versions(version_info: str) -> tuple[GlibcVersion, ...]:
    """Return GLIBC requirements from ``readelf --version-info`` output.

    Only the ELF version-needs section records versions required from external
    libraries. Limiting parsing to that section avoids treating version
    definitions provided by the inspected binary as requirements.

    Example
    -------
        >>> parse_required_glibc_versions("No version information found")
        ()
    """
    if "No version information found" in version_info:
        return ()

    requirements = {
        (int(match["major"]), int(match["minor"]))
        for line in _version_needs_lines(version_info)
        for match in GLIBC_VERSION_PATTERN.finditer(line)
    }
    return tuple(sorted(requirements))


def _require_readable_file(path: Path) -> None:
    """Reject a path that does not identify a readable file.

    Example
    -------
        >>> _require_readable_file(Path("missing"))
        Traceback (most recent call last):
        ...
        ElfInspectionError: input is not a readable file: missing
    """
    if not path.is_file():
        message = f"input is not a readable file: {path}"
        raise ElfInspectionError(message)


def _run_readelf(path: Path) -> subprocess.CompletedProcess[str]:
    """Run the fixed readelf inspection command for an ELF path.

    Example
    -------
        >>> _run_readelf(Path("target/release/whitaker")).returncode
        0
    """
    try:
        return subprocess.run(  # noqa: S603,S607  # Static tool and path come from this CLI.
            [READELF, "--version-info", "--wide", str(path)],
            capture_output=True,
            check=False,
            text=True,
        )
    except OSError as error:
        message = f"could not execute {READELF} for {path}: {error}"
        raise ElfInspectionError(message) from error


def _require_successful_readelf(
    path: Path, completed: subprocess.CompletedProcess[str]
) -> None:
    """Reject a failed readelf inspection result.

    Example
    -------
        >>> result = subprocess.CompletedProcess([], 1, "", "not an ELF file")
        >>> _require_successful_readelf(Path("binary"), result)
        Traceback (most recent call last):
        ...
        ElfInspectionError: could not inspect ELF file binary: not an ELF file
    """
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        message = f"could not inspect ELF file {path}: {detail}"
        raise ElfInspectionError(message)


def _parse_readelf_requirements(
    path: Path, version_info: str
) -> tuple[GlibcVersion, ...]:
    """Parse readelf output with path-specific failure context.

    Example
    -------
        >>> _parse_readelf_requirements(Path("binary"), "No version information found")
        ()
    """
    try:
        return parse_required_glibc_versions(version_info)
    except ElfInspectionError as error:
        message = f"could not parse ELF version information for {path}: {error}"
        raise ElfInspectionError(message) from error


def read_required_glibc_versions(path: Path) -> tuple[GlibcVersion, ...]:
    """Inspect an ELF binary and return its required GLIBC versions.

    Example
    -------
        >>> read_required_glibc_versions(Path("target/release/whitaker"))
        ((2, 35),)
    """
    _require_readable_file(path)
    completed = _run_readelf(path)
    _require_successful_readelf(path, completed)
    return _parse_readelf_requirements(path, completed.stdout)


def maximum_required_glibc(
    requirements: Sequence[GlibcVersion],
) -> GlibcVersion | None:
    """Return the greatest required GLIBC version, when one exists.

    Example
    -------
        >>> maximum_required_glibc(((2, 17), (2, 35)))
        (2, 35)
    """
    return max(requirements, default=None)


def requirements_exceed_baseline(
    requirements: Sequence[GlibcVersion], baseline: GlibcVersion
) -> tuple[GlibcVersion, ...]:
    """Return the requirements that exceed the requested glibc baseline.

    Example
    -------
        >>> requirements_exceed_baseline(((2, 35), (2, 39)), (2, 35))
        ((2, 39),)
    """
    return tuple(version for version in requirements if version > baseline)


def parse_arguments(
    arguments: Sequence[str] | None = None,
) -> tuple[GlibcVersion, list[Path]]:
    """Parse the ELF inputs and maximum glibc baseline from CLI arguments.

    Example
    -------
        >>> parse_arguments(["binary"])[0]
        (2, 35)
    """
    parser = argparse.ArgumentParser(
        description="Reject ELF files requiring glibc newer than the release baseline."
    )
    _ = parser.add_argument(
        "--maximum-glibc",
        default=DEFAULT_MAXIMUM_GLIBC,
        type=parse_glibc_version,
        help=f"greatest permitted GLIBC version (default: {DEFAULT_MAXIMUM_GLIBC})",
    )
    _ = parser.add_argument(
        "files",
        metavar="ELF",
        nargs="+",
        type=Path,
        help="ELF binary to inspect",
    )
    parsed = parser.parse_args(arguments)
    return (
        cast("GlibcVersion", parsed.maximum_glibc),
        cast("list[Path]", parsed.files),
    )


def main(arguments: Sequence[str] | None = None) -> int:
    """Check each requested ELF file against the selected glibc baseline.

    Example
    -------
        >>> main(["target/release/whitaker"])
        0
    """
    baseline, files = parse_arguments(arguments)
    requirements_by_path: dict[Path, tuple[GlibcVersion, ...]] = {}
    try:
        for path in files:
            requirements_by_path[path] = read_required_glibc_versions(path)
    except ElfInspectionError as error:
        print(f"Error: {error}", file=sys.stderr)
        return 2

    failures = False
    for path, requirements in requirements_by_path.items():
        maximum = maximum_required_glibc(requirements)
        maximum_text = "none" if maximum is None else format_glibc_version(maximum)
        print(f"{path}: maximum required GLIBC version: {maximum_text}")
        exceeded = requirements_exceed_baseline(requirements, baseline)
        if exceeded:
            failures = True
            versions = ", ".join(map(format_glibc_version, exceeded))
            message = (
                f"Error: {path} requires {versions}, above "
                f"{format_glibc_version(baseline)}."
            )
            print(message, file=sys.stderr)
    return int(failures)


if __name__ == "__main__":
    raise SystemExit(main())
