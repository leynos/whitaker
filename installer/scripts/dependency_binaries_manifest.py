#!/usr/bin/env python3
"""Emit dependency-binary manifest entries as tab-separated rows."""

from __future__ import annotations

import argparse
import pathlib
import sys
import tomllib


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Read installer/dependency-binaries.toml and emit TSV rows."
    )
    parser.add_argument(
        "manifest",
        nargs="?",
        default="installer/dependency-binaries.toml",
        help="Path to the dependency-binaries manifest.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    manifest_path = pathlib.Path(args.manifest)
    with manifest_path.open("rb") as handle:
        manifest = tomllib.load(handle)

    for entry in manifest["dependency_binaries"]:
        line = f"{entry['package']}\t{entry['binary']}\t{entry['version']}\n".encode()
        sys.stdout.buffer.write(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
