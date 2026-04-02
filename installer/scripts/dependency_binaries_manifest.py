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
import pathlib
import sys
import tomllib


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


def collect_output_lines(
    entries: list[dict],
) -> tuple[list[bytes], int]:
    """Validate entries for duplicate packages and serialise to TSV bytes.

    Parameters
    ----------
    entries:
        The list of dependency-binary dicts parsed from the TOML manifest.

    Returns
    -------
    tuple[list[bytes], int]
        A pair of (lines, exit_code).  On duplicate-package detection,
        lines is empty and exit_code is 1; otherwise lines contains one
        encoded TSV row per entry and exit_code is 0.

    """
    seen_packages: set[str] = set()
    output_lines: list[bytes] = []
    for entry in entries:
        package = entry["package"]
        if package in seen_packages:
            print(
                f"error: duplicate package '{package}' in manifest",
                file=sys.stderr,
            )
            return [], 1
        seen_packages.add(package)
        line = f"{entry['package']}\t{entry['binary']}\t{entry['version']}\n".encode()
        output_lines.append(line)
    return output_lines, 0


def write_output_lines(lines: list[bytes], output: str | None) -> None:
    """Write serialised TSV lines to a file or to stdout.

    Parameters
    ----------
    lines:
        Encoded TSV rows to write.
    output:
        Destination file path, or ``None`` to write to ``sys.stdout.buffer``.

    """
    if output:
        with pathlib.Path(output).open("wb") as out_handle:
            for line in lines:
                out_handle.write(line)
    else:
        for line in lines:
            sys.stdout.buffer.write(line)


def main() -> int:
    """Emit dependency-binary manifest entries as tab-separated rows.

    Parses CLI arguments, reads the TOML manifest at the specified path,
    and writes package/binary/version lines to stdout as tab-separated
    values (one line per dependency binary).

    Parameters
    ----------
    None
        Arguments are obtained via parse_args() from sys.argv.

    Returns
    -------
    int
        Exit code 0 on success.

    Raises
    ------
    FileNotFoundError
        If the specified manifest file does not exist.
    OSError
        If the manifest file cannot be read.
    tomllib.TOMLDecodeError
        If the manifest file contains invalid TOML syntax.
    KeyError
        If the manifest is missing the top-level 'dependency_binaries' key
        or when individual entries are missing required keys (package,
        binary, version).

    Side Effects
    ------------
    Writes tab-separated rows to sys.stdout.buffer (one per dependency
    binary, with columns: package, binary, version).

    Example
    -------
    >>> import sys, tempfile, os
    >>> manifest_content = b'[[dependency_binaries]]\\npackage = "pkg"\\nbinary = "bin"\\nversion = "1.0.0"\\n'
    >>> with tempfile.NamedTemporaryFile(delete=False, suffix=".toml") as f:
    ...     _ = f.write(manifest_content)
    ...     temp_path = f.name
    >>> sys.argv = ["script.py", temp_path]
    >>> try:
    ...     main()
    ... finally:
    ...         os.unlink(temp_path)
    0

    Notes
    -----
    The output format is suitable for shell processing with tools like
    cut, awk, or while-read loops.

    """
    args = parse_args()
    manifest_path = pathlib.Path(args.manifest)
    with manifest_path.open("rb") as handle:
        manifest = tomllib.load(handle)

    lines, exit_code = collect_output_lines(manifest["dependency_binaries"])
    if exit_code != 0:
        return exit_code

    write_output_lines(lines, args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
