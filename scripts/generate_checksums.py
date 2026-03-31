#!/usr/bin/env python3
"""Generate SHA-256 checksum files for release archives.

This script generates `.sha256` checksum files for all archive files
in the specified directory. Archives are processed using a streaming
approach to avoid memory pressure with large files.

Example
-------
    python scripts/generate_checksums.py dist/

The script expects to find archive files matching the configured patterns
and produces `<archive>.sha256` files in the same directory.
"""

from __future__ import annotations

import argparse
import sys
from hashlib import sha256
from pathlib import Path

# Archive glob patterns to match for checksum generation.
# These should be kept in sync with the upload patterns in the release workflow.
ARCHIVE_PATTERNS: tuple[str, ...] = ("*.tgz", "*.zip")

# Buffer size for streaming hash computation (64KB).
READ_BUFFER_SIZE: int = 64 * 1024


class NoArchivesFoundError(Exception):
    """Raised when no archive files are found in the specified directory."""

    pass


def compute_sha256(path: Path) -> str:
    """Compute SHA-256 hex digest for a file using streaming reads.

    Parameters
    ----------
    path : Path
        Path to the file to hash.

    Returns
    -------
    str
        Hexadecimal SHA-256 digest string.

    Example
    -------
        >>> digest = compute_sha256(Path("archive.tgz"))
        >>> len(digest)
        64
    """
    hasher = sha256()
    with path.open("rb") as f:
        while chunk := f.read(READ_BUFFER_SIZE):
            hasher.update(chunk)
    return hasher.hexdigest()


def find_archives(directory: Path) -> list[Path]:
    """Find all archive files matching configured patterns.

    Parameters
    ----------
    directory : Path
        Directory to search for archives.

    Returns
    -------
    list[Path]
        Sorted list of paths to archive files.

    Raises
    ------
    NoArchivesFoundError
        If no archives matching the configured patterns are found.
    """
    archives = sorted(
        path for pattern in ARCHIVE_PATTERNS for path in directory.glob(pattern)
    )

    if not archives:
        raise NoArchivesFoundError(directory)

    return archives


def generate_checksums(directory: Path) -> None:
    """Generate SHA-256 checksum files for all archives in directory.

    Parameters
    ----------
    directory : Path
        Directory containing archive files.
    """
    archive_paths = find_archives(directory)

    for archive_path in archive_paths:
        digest = compute_sha256(archive_path)
        checksum_path = archive_path.with_name(f"{archive_path.name}.sha256")
        checksum_path.write_text(f"{digest}  {archive_path.name}\n", encoding="ascii")
        print(f"Generated {checksum_path.name}")


def main() -> int:
    """Entry point for the checksum generation script.

    Returns
    -------
    int
        Exit code (0 for success, non-zero for failure).
    """
    parser = argparse.ArgumentParser(
        description="Generate SHA-256 checksums for release archives."
    )
    parser.add_argument(
        "directory",
        type=Path,
        nargs="?",
        default=Path("dist"),
        help="Directory containing archives (default: dist)",
    )
    args = parser.parse_args()

    if not args.directory.is_dir():
        print(f"Error: Not a directory: {args.directory}", file=sys.stderr)
        return 1

    try:
        generate_checksums(args.directory)
    except NoArchivesFoundError:
        print(
            f"No archives found in {args.directory} matching patterns: {ARCHIVE_PATTERNS}",
            file=sys.stderr,
        )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
