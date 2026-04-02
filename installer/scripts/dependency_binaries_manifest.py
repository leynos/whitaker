#!/usr/bin/env python3
"""Emit dependency-binary manifest entries as tab-separated rows.

This script reads the installer/dependency-binaries.toml manifest and outputs
package/binary/version rows as tab-separated values for CI consumption.

The default manifest path is "installer/dependency-binaries.toml". Each output
line contains three tab-separated columns: package name, binary name, and version.

Example invocation:

    $ python dependency_binaries_manifest.py
    cargo-dylint\tcargo-dylint\t4.1.0\n
    $ python dependency_binaries_manifest.py custom-manifest.toml
    cargo-dylint\tcargo-dylint\t4.1.0\n
The TSV output is suitable for shell processing with tools like cut, awk, or
while-read loops.
"""

from __future__ import annotations

import argparse
import contextlib
import pathlib
import sys
import tomllib
from collections.abc import Generator


def parse_args() -> argparse.Namespace:
    """Parse command-line arguments for the dependency-binaries manifest tool.

    Reads CLI arguments to determine the path to the dependency-binaries
    manifest file. The default path is "installer/dependency-binaries.toml".

    Parameters
    ----------
    None
        Arguments are read from sys.argv.

    Returns
    -------
    argparse.Namespace
        Parsed arguments with the 'manifest' attribute containing the path to
        the dependency-binaries manifest and 'output' attribute containing the
        path to the output file (or None for stdout).

    Example
    -------
    >>> args = parse_args()
    >>> print(args.manifest)
    'installer/dependency-binaries.toml'

    Notes
    -----
    The manifest argument is optional and defaults to the standard location
    within the installer directory. Use --output to write to a file instead
    of stdout.

    """
    parser = argparse.ArgumentParser(
        description="Read installer/dependency-binaries.toml and emit TSV rows."
    )
    parser.add_argument(
        "manifest",
        nargs="?",
        default="installer/dependency-binaries.toml",
        help="Path to the dependency-binaries manifest.",
    )
    parser.add_argument(
        "--output",
        "-o",
        default=None,
        help="Output file path (default: stdout).",
    )
    return parser.parse_args()


def _collect_manifest_lines(
    manifest_path: pathlib.Path,
) -> list[bytes] | int:
    """Collect and return encoded TSV lines from the manifest."""
    with manifest_path.open("rb") as handle:
        manifest = tomllib.load(handle)

    seen_packages: set[str] = set()
    output_lines: list[bytes] = []
    for entry in manifest["dependency_binaries"]:
        package = entry["package"]
        if package in seen_packages:
            print(
                f"error: duplicate package '{package}' in manifest",
                file=sys.stderr,
            )
            return 1
        seen_packages.add(package)
        line = f"{entry['package']}\t{entry['binary']}\t{entry['version']}\n".encode()
        output_lines.append(line)
    return output_lines


@contextlib.contextmanager
def _open_output(output: str | None) -> Generator:
    """Yield a binary-writable handle for *output*, or ``sys.stdout.buffer``."""
    if output is not None:
        if output == "":
            print("error: output path cannot be empty", file=sys.stderr)
            raise SystemExit(1)
        with pathlib.Path(output).open("wb") as handle:
            yield handle
    else:
        yield sys.stdout.buffer


def _write_lines(lines: list[bytes], output: str | None) -> None:
    """Write encoded lines to the output stream."""
    with _open_output(output) as handle:
        handle.writelines(lines)


def main() -> int:
    """Emit dependency-binary manifest entries as tab-separated rows.

    Parses CLI arguments, reads the TOML manifest, validates uniqueness of
    package names, and writes ``package\\tbinary\\tversion`` lines to the
    specified output destination.

    Returns
    -------
    int
        0 on success, 1 if the manifest contains a duplicate package name.
    """
    args = parse_args()
    result = _collect_manifest_lines(pathlib.Path(args.manifest))
    match result:
        case int():
            return result
        case list():
            _write_lines(result, args.output)
            return 0


if __name__ == "__main__":
    raise SystemExit(main())
