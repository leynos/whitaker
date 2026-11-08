#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
"""Refuse to publish lint archives the oldest installer cannot prune.

An installer prunes an unrequested standalone library only when it knows the
crate's name, because it enumerates its compiled-in ``LINT_CRATES``. After a
suite install it therefore keeps ``libwhitaker_suite`` and any library it does
not know. A kept standalone library loads beside the suite, which already
registers the same lint, and rustc panics with ``duplicate specification of
lint`` (whitaker issue 565, and issue 355 before it).

This checker reads the crate list of the oldest installer a caller can still
select from that release tag, reads each built archive's real entries, applies
the installer's prune, and fails unless only the suite is left. It also holds
each sidecar manifest to the archive it describes. The rolling-release
``publish`` job runs it before anything is uploaded.

Example
-------
    git fetch --depth=1 origin tag v0.2.11
    scripts/check_prune_safety.py --installer-tag v0.2.11 dist
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

SUITE_CRATE = "whitaker_suite"
RESOLUTION_PATH = "installer/src/resolution.rs"

def _constant_pattern(name: str) -> re.Pattern[str]:
    """Return a pattern capturing the body of ``pub const <name>: &[&str]``."""
    return re.compile(
        rf"pub(?:\(crate\))?\s+const\s+{name}\s*:\s*&\[\s*&str\s*\]\s*=\s*&\[(.*?)\];",
        re.DOTALL,
    )


_LINT_CRATES_PATTERN = _constant_pattern("LINT_CRATES")
_EXPERIMENTAL_PATTERN = _constant_pattern("EXPERIMENTAL_LINT_CRATES")


class PruneSafetyError(Exception):
    """Represent a failure to establish or check the prune contract."""


def parse_lint_crates(source: str) -> list[str]:
    """Return every crate name the installer's prune knows by name.

    The prune removes each crate in ``LINT_CRATES`` and, when the installer
    has one, ``EXPERIMENTAL_LINT_CRATES`` that was not requested.

    >>> parse_lint_crates('pub const LINT_CRATES: &[&str] = &["a", "b"];')
    ['a', 'b']
    >>> parse_lint_crates(
    ...     'pub const LINT_CRATES: &[&str] = &["a"];'
    ...     'pub const EXPERIMENTAL_LINT_CRATES: &[&str] = &["x"];'
    ... )
    ['a', 'x']
    """
    match = _LINT_CRATES_PATTERN.search(source)
    if match is None:
        raise PruneSafetyError("no LINT_CRATES constant found in the installer source")
    crates = re.findall(r'"([^"]+)"', match.group(1))
    if not crates:
        raise PruneSafetyError("the installer's LINT_CRATES is empty")
    experimental = _EXPERIMENTAL_PATTERN.search(source)
    if experimental is not None:
        crates.extend(re.findall(r'"([^"]+)"', experimental.group(1)))
    return crates


def _run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    """Run a fixed argv without a shell; a missing tool is a checker error."""
    try:
        return subprocess.run(  # noqa: S603 - fixed argv, no shell
            argv, capture_output=True, text=True, check=False
        )
    except OSError as error:
        raise PruneSafetyError(f"cannot run {argv[0]}: {error}") from error


def installer_lint_crates(tag: str, repository: Path) -> list[str]:
    """Read the ``LINT_CRATES`` the installer at ``tag`` prunes by."""
    result = _run(["git", "-C", str(repository), "show", f"{tag}:{RESOLUTION_PATH}"])
    if result.returncode != 0:
        raise PruneSafetyError(
            f"cannot read {RESOLUTION_PATH} at tag {tag}: {result.stderr.strip()}"
        )
    return parse_lint_crates(result.stdout)


def crate_of_entry(entry: str) -> str:
    """Return the crate behind a library file name.

    Unix libraries (``.so``, ``.dylib``) carry a ``lib`` prefix; Windows
    ``.dll`` files do not, and a Windows ``libfoo.dll`` is the crate
    ``libfoo``, which the installer would not prune as ``foo``. The staged
    ``foo@toolchain.ext`` form is also understood.

    >>> crate_of_entry("libbumpy_road_function.so")
    'bumpy_road_function'
    >>> crate_of_entry("libfoo@nightly-2026-05-28-x86_64-unknown-linux-gnu.so")
    'foo'
    >>> crate_of_entry("whitaker_suite.dll")
    'whitaker_suite'
    >>> crate_of_entry("libmodule_max_lines.dll")
    'libmodule_max_lines'
    """
    name = Path(entry).name
    base = name.split(".", maxsplit=1)[0].split("@", maxsplit=1)[0]
    if name.endswith((".so", ".dylib")) and base.startswith("lib"):
        base = base[3:]
    return base


def surviving_crates(entries: Sequence[str], known: Sequence[str]) -> list[str]:
    """Return the crates left after a suite install prunes the ``known`` ones.

    >>> surviving_crates(["liba.so", "libb.so", "libwhitaker_suite.so"], ["a"])
    ['b', 'whitaker_suite']
    """
    crates = {crate_of_entry(entry) for entry in entries}
    return sorted(crates - set(known))


def archive_entries(archive: Path) -> list[str]:
    """List the member names of a ``.tar.zst`` archive."""
    result = _run(["tar", "--zstd", "-tf", str(archive)])
    if result.returncode != 0:
        raise PruneSafetyError(f"cannot list {archive}: {result.stderr.strip()}")
    names = (line.removeprefix("./") for line in result.stdout.splitlines())
    return [name for name in names if name and not name.endswith("/")]


def manifest_files(manifest: Path) -> list[str]:
    """Return the ``files`` list of a sidecar manifest."""
    try:
        files = json.loads(manifest.read_text(encoding="utf-8"))["files"]
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise PruneSafetyError(f"cannot read the files list of {manifest}: {error}") from error
    if not isinstance(files, list) or not all(isinstance(name, str) for name in files):
        raise PruneSafetyError(f"{manifest} has a malformed files list")
    return files


def check_archive(archive: Path, known: Sequence[str]) -> list[str]:
    """Return the problems with one archive, empty when it is safe."""
    problems: list[str] = []
    left = surviving_crates(archive_entries(archive), known)
    if left != [SUITE_CRATE]:
        problems.append(
            f"{archive.name}: after the oldest installer's prune, {left} would remain "
            f"where only [{SUITE_CRATE!r}] may; a standalone library beside the suite "
            "makes rustc reject the duplicate lint registration"
        )
    return problems


def check_manifest(manifest: Path, archive: Path) -> list[str]:
    """Return the problems when a manifest disagrees with its archive."""
    in_archive = sorted(archive_entries(archive))
    in_manifest = sorted(manifest_files(manifest))
    if in_archive != in_manifest:
        return [
            f"{manifest.name}: lists {in_manifest} but {archive.name} holds {in_archive}"
        ]
    return []


def check_directory(directory: Path, known: Sequence[str]) -> list[str]:
    """Check every lint archive and manifest under ``directory``."""
    archives = sorted(directory.glob("whitaker-lints-*.tar.zst"))
    if not archives:
        raise PruneSafetyError(f"no whitaker-lints archives found in {directory}")
    problems: list[str] = []
    for archive in archives:
        problems.extend(check_archive(archive, known))
        target = re.search(r"-nightly-[0-9-]+-(?P<target>.+)\.tar\.zst$", archive.name)
        manifest = directory / f"manifest-{target.group('target')}.json" if target else None
        if manifest is not None and manifest.is_file():
            problems.extend(check_manifest(manifest, archive))
    return problems


def main(arguments: Sequence[str] | None = None) -> int:
    """Run the checker; return a process exit status."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("directory", type=Path, help="directory holding the built archives")
    parser.add_argument(
        "--installer-tag",
        required=True,
        help="release tag of the oldest installer a caller can select, such as v0.2.11",
    )
    parser.add_argument("--repository", type=Path, default=Path("."))
    options = parser.parse_args(arguments)
    try:
        known = installer_lint_crates(options.installer_tag, options.repository)
        problems = check_directory(options.directory, known)
    except PruneSafetyError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    for problem in problems:
        print(f"error: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(
        f"Every archive leaves only {SUITE_CRATE} after installer "
        f"{options.installer_tag} prunes by name ({len(known)} known crates)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
