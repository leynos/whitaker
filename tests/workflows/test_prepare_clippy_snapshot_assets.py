"""`prepare-clippy-snapshot-assets.sh` stages the snapshot as one pair.

The release workflows call the script to decide what the release needs. It
must never stage a sidecar that does not describe the published bundle's own
bytes, and it must refuse an orphaned sidecar rather than guess.
"""

from __future__ import annotations

import hashlib
import os
import stat
import subprocess
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "prepare-clippy-snapshot-assets.sh"


def _commit() -> str:
    """Return the commit the committed manifest pins."""
    text = (REPO_ROOT / "installer" / "clippy-snapshot.toml").read_text(encoding="utf-8")
    return next(line.split('"')[1] for line in text.splitlines() if line.startswith("commit = "))


ASSET = f"whitaker-clippy-snapshot-{_commit()}.bundle"

_GH_STUB = """#!/usr/bin/env bash
printf 'gh %s\\n' "$*" >> "$STUB_LOG"
case "$1 $2" in
    "release view") printf '%s\\n' ${PUBLISHED_NAMES} ;;
    "release download")
        dir=""; previous=""
        for argument in "$@"; do
            [[ "$previous" == --dir ]] && dir="$argument"
            previous="$argument"
        done
        printf 'published bundle bytes' > "$dir/$ASSET"
        ;;
esac
"""

_MAKE_STUB = """#!/usr/bin/env bash
printf 'make %s\\n' "$*" >> "$STUB_LOG"
case "$1" in
    clippy-snapshot-bundle)
        dist="${2#CLIPPY_SNAPSHOT_DIST=}"
        printf 'rebuilt bundle bytes' > "$dist/$ASSET"
        printf 'rebuilt  %s\\n' "$ASSET" > "$dist/$ASSET.sha256"
        ;;
    clippy-snapshot) exit "${STUB_VERIFY_EXIT:-0}" ;;
esac
"""


class Prepared(typ.NamedTuple):
    """One recorded run of the script."""

    returncode: int
    output: str
    log: str
    state: str
    staged: list[str]


def _write_stub(directory: Path, name: str, body: str) -> None:
    path = directory / name
    path.write_text(body, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


@pytest.fixture
def prepare(tmp_path: Path) -> typ.Callable[..., Prepared]:
    """Return a callable that runs the script against stubbed `gh` and `make`."""
    stubs = tmp_path / "bin"
    stubs.mkdir()
    _write_stub(stubs, "gh", _GH_STUB)
    _write_stub(stubs, "make", _MAKE_STUB)

    def _run(*, published: list[str], verify_exit: int = 0) -> Prepared:
        dist = tmp_path / "dist"
        log = tmp_path / "stub.log"
        output = tmp_path / "github-output"
        env = {
            key: value
            for key, value in os.environ.items()
            if key != "BASH_ENV"  # a session BASH_ENV can define its own `gh`
        } | {
            "PATH": f"{stubs}{os.pathsep}{os.environ['PATH']}",
            "STUB_LOG": str(log),
            "PUBLISHED_NAMES": " ".join(published),
            "ASSET": ASSET,
            "STUB_VERIFY_EXIT": str(verify_exit),
            "GITHUB_OUTPUT": str(output),
        }
        result = subprocess.run(  # noqa: S603 - fixed argument vector
            ["bash", str(SCRIPT), "rolling", str(dist)],  # noqa: S607
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        state = output.read_text(encoding="utf-8").strip() if output.exists() else ""
        return Prepared(
            result.returncode,
            result.stdout + result.stderr,
            log.read_text(encoding="utf-8") if log.exists() else "",
            state,
            sorted(path.name for path in dist.iterdir()) if dist.exists() else [],
        )

    return _run


def test_a_complete_published_pair_needs_nothing(prepare: typ.Callable[..., Prepared]) -> None:
    """Both halves published: no build, no download, nothing staged."""
    run = prepare(published=[ASSET, f"{ASSET}.sha256"])

    assert run.returncode == 0, run.output
    assert run.state == "state=published"
    assert run.staged == []
    assert "make" not in run.log


def test_a_published_bundle_gets_a_sidecar_from_its_own_bytes(
    prepare: typ.Callable[..., Prepared], tmp_path: Path
) -> None:
    """An interrupted publication is repaired from the published bytes only."""
    run = prepare(published=[ASSET])

    assert run.returncode == 0, run.output
    assert run.state == "state=repaired"
    assert run.staged == [f"{ASSET}.sha256"], "never a rebuilt bundle"
    digest = hashlib.sha256(b"published bundle bytes").hexdigest()
    sidecar = (tmp_path / "dist" / f"{ASSET}.sha256").read_text(encoding="utf-8")
    assert sidecar == f"{digest}  {ASSET}\n"
    assert "make clippy-snapshot CLIPPY_SNAPSHOT_BUNDLE=" in run.log, "verified first"
    assert "clippy-snapshot-bundle" not in run.log


def test_an_unverifiable_published_bundle_is_not_vouched_for(
    prepare: typ.Callable[..., Prepared],
) -> None:
    """A published bundle that fails verification gets no sidecar."""
    run = prepare(published=[ASSET], verify_exit=1)

    assert run.returncode != 0
    assert run.staged == []


def test_an_orphaned_sidecar_is_refused(prepare: typ.Callable[..., Prepared]) -> None:
    """No rebuilt bundle can match a sidecar whose bundle was lost."""
    run = prepare(published=[f"{ASSET}.sha256"])

    assert run.returncode != 0
    assert "gh release delete-asset rolling" in run.output
    assert "make" not in run.log
    assert run.staged == []


def test_an_absent_pair_is_built(prepare: typ.Callable[..., Prepared]) -> None:
    """With nothing published, the producer builds both halves."""
    run = prepare(published=[])

    assert run.returncode == 0, run.output
    assert run.state == "state=built"
    assert run.staged == [ASSET, f"{ASSET}.sha256"]
