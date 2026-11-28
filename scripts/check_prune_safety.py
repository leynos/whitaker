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
each sidecar manifest to the archive it describes. A second, cheap check
holds each manifest's toolchain to the one shared-actions maps that installer
to (issue 555). This reproduces the installer's prune; it does not run the
installer, which has no route to install from a local archive. The
rolling-release ``publish`` job runs it before anything is uploaded.

Example
-------
    git fetch --depth=1 origin tag v0.2.11
    scripts/check_prune_safety.py --installer-tag v0.2.11 dist
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

from prune_safety_rules import (
    PruneSafetyError,
    SUITE_CRATE,
    parse_archive_name,
    parse_lint_crates,
    surviving_crates,
)

RESOLUTION_PATH = "installer/src/resolution.rs"


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
        raise PruneSafetyError(
            f"cannot read the files list of {manifest}: {error}"
        ) from error
    if not isinstance(files, list) or not all(isinstance(name, str) for name in files):
        raise PruneSafetyError(f"{manifest} has a malformed files list")
    return files


def check_archive(archive: Path, known: Sequence[str]) -> list[str]:
    """Return the problems with one archive, empty when it is safe."""
    problems: list[str] = []
    target = parse_archive_name(archive.name)["target"]
    left = surviving_crates(archive_entries(archive), known, target)
    if left != [SUITE_CRATE]:
        problems.append(
            f"{archive.name}: after the oldest installer's prune, {left} would remain "
            f"where only [{SUITE_CRATE!r}] may; a standalone library beside the suite "
            "makes rustc reject the duplicate lint registration"
        )
    return problems


def manifest_field(manifest: Path, field: str) -> object:
    """Return one top-level field of a sidecar manifest."""
    try:
        return json.loads(manifest.read_text(encoding="utf-8"))[field]
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise PruneSafetyError(
            f"cannot read {field} from {manifest}: {error}"
        ) from error


def check_archive_name(manifest: Path, archive: Path) -> list[str]:
    """Return the problems when the archive's name is not the manifest's.

    The installer derives ``whitaker-lints-<sha>-<toolchain>-<target>.tar.zst``
    from the manifest, so an archive published under any other name is never
    the one a consumer downloads.
    """
    parts = parse_archive_name(archive.name)
    wanted = {"sha": "git_sha", "toolchain": "toolchain", "target": "target"}
    return [
        f"{archive.name}: its {part} is {parts[part]!r} but {manifest.name} records "
        f"{field} {manifest_field(manifest, field)!r}"
        for part, field in wanted.items()
        if manifest_field(manifest, field) != parts[part]
    ]


def check_manifest(manifest: Path, archive: Path) -> list[str]:
    """Return the problems when a manifest disagrees with its archive."""
    in_archive = sorted(archive_entries(archive))
    in_manifest = sorted(manifest_files(manifest))
    problems = check_archive_name(manifest, archive)
    if in_archive != in_manifest:
        problems.append(
            f"{manifest.name}: lists {in_manifest} "
            f"but {archive.name} holds {in_archive}"
        )
    return problems


def snapshot_toolchain(tsv: Path, version: str) -> str:
    r"""Return the toolchain ``clippy-snapshots.tsv`` maps ``version`` to.

    Rows read ``<installer-version> <clippy-commit> <dylint-version> <toolchain>``;
    lines starting with ``#`` are comments.

    >>> import tempfile
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     table = Path(directory) / "t.tsv"
    ...     _ = table.write_text("# c\n0.2.11 abc 6.3.0 nightly-2026-05-28\n")
    ...     snapshot_toolchain(table, "0.2.11")
    'nightly-2026-05-28'
    """
    try:
        lines = tsv.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise PruneSafetyError(f"cannot read {tsv}: {error}") from error
    rows = [line.split() for line in lines if not line.startswith("#")]
    toolchains = {row[0]: row[3] for row in rows if len(row) >= 4}
    if version not in toolchains:
        raise PruneSafetyError(f"{tsv} has no row for installer {version}")
    return toolchains[version]


def manifest_toolchain(manifest: Path) -> object:
    """Return the ``toolchain`` field of a sidecar manifest."""
    try:
        return json.loads(manifest.read_text(encoding="utf-8"))["toolchain"]
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise PruneSafetyError(
            f"cannot read the toolchain of {manifest}: {error}"
        ) from error


def check_toolchain(directory: Path, expected: str) -> list[str]:
    """Return the problems with any manifest whose toolchain is not ``expected``.

    The installer reads its toolchain from the snapshot row the action maps to
    its version. Publishing libraries built for another toolchain breaks every
    caller on that installer at once (whitaker issue 555).
    """
    return [
        f"{manifest.name}: built for toolchain {toolchain!r} but the oldest "
        f"installer's snapshot row maps to {expected!r}"
        for manifest in sorted(directory.glob("manifest-*.json"))
        if (toolchain := manifest_toolchain(manifest)) != expected
    ]


def sidecar_manifest(archive: Path) -> Path:
    """Return the manifest path that pairs with ``archive``.

    >>> name = "whitaker-lints-abc-nightly-2026-05-28-x86_64-unknown-linux-gnu.tar.zst"
    >>> sidecar_manifest(Path("d") / name).name
    'manifest-x86_64-unknown-linux-gnu.json'
    """
    return archive.with_name(
        f"manifest-{parse_archive_name(archive.name)['target']}.json"
    )


def check_directory(directory: Path, known: Sequence[str]) -> list[str]:
    """Check every lint archive and manifest under ``directory``."""
    archives = sorted(directory.glob("whitaker-lints-*.tar.zst"))
    if not archives:
        raise PruneSafetyError(f"no whitaker-lints archives found in {directory}")
    problems: list[str] = []
    for archive in archives:
        problems.extend(check_archive(archive, known))
        manifest = sidecar_manifest(archive)
        if not manifest.is_file():
            problems.append(f"{archive.name}: its sidecar {manifest.name} is missing")
        else:
            problems.extend(check_manifest(manifest, archive))
    return problems


def main(arguments: Sequence[str] | None = None) -> int:
    """Run the checker; return a process exit status."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "directory", type=Path, help="directory holding the built archives"
    )
    parser.add_argument(
        "--installer-tag",
        required=True,
        help="release tag of the oldest installer a caller can select, such as v0.2.11",
    )
    parser.add_argument("--repository", type=Path, default=Path("."))
    parser.add_argument(
        "--snapshots-tsv",
        type=Path,
        help="shared-actions install-whitaker clippy-snapshots.tsv; when given, every "
        "manifest's toolchain must equal the one it maps the installer tag to",
    )
    return run(parser.parse_args(arguments))


def collect_problems(options: argparse.Namespace) -> tuple[list[str], int]:
    """Return every problem found and how many crates the installer prunes."""
    known = installer_lint_crates(options.installer_tag, options.repository)
    problems = check_directory(options.directory, known)
    if options.snapshots_tsv is not None:
        version = options.installer_tag.removeprefix("v")
        expected = snapshot_toolchain(options.snapshots_tsv, version)
        problems.extend(check_toolchain(options.directory, expected))
    return problems, len(known)


def run(options: argparse.Namespace) -> int:
    """Report the problems for ``options`` and return the exit status."""
    try:
        problems, known_count = collect_problems(options)
    except PruneSafetyError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    for problem in problems:
        print(f"error: {problem}", file=sys.stderr)
    if not problems:
        print(
            f"Every archive leaves only {SUITE_CRATE} after installer "
            f"{options.installer_tag} prunes by name ({known_count} known crates)."
        )
    return int(bool(problems))


if __name__ == "__main__":
    raise SystemExit(main())
