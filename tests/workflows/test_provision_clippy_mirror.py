"""Behavioural tests for the Clippy source mirror provisioning script.

`scripts/provision-clippy-mirror.sh` owns the only copy of the Clippy
sources that `dylint_driver`'s build script may clone, so it both creates
that copy and deletes stale generations of it. These tests exercise the
script end to end against a local upstream repository, with no network
access: a per-test ``HOME`` carries a ``url.<local>.insteadOf`` rule for the
upstream URL, and git records the canonical URL in ``remote.origin.url``
regardless, so the script's origin check runs against the real value.

Covered behaviour:

- a cold root clones the mirror once and reports a miss;
- a restored valid generation is reused untouched and reports a hit;
- a non-bare or wrong-origin generation is replaced rather than reused;
- a path outside the trusted cache root is refused, even when it ends in
  ``rust-clippy.git``, and its contents survive;
- a non-directory at the mirror path aborts instead of being deleted; and
- the wrong argument count is a usage error.

Examples
--------
Run all tests:
    PYTHONPATH=. python3 -m pytest tests/workflows/test_provision_clippy_mirror.py
"""

from __future__ import annotations

import dataclasses as dc
import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "provision-clippy-mirror.sh"
CLIPPY_URL = "https://github.com/rust-lang/rust-clippy"
MIRROR_NAME = "rust-clippy.git"
SENTINEL = "whitaker-reuse-sentinel"


@dc.dataclass(frozen=True)
class Harness:
    """A self-contained provisioning environment for one test."""

    home: Path
    upstream: Path
    root: Path
    mirror: Path
    output: Path
    env: dict[str, str]


def _git(*arguments: str, env: dict[str, str] | None = None) -> str:
    """Run git with a deterministic identity and return its stdout."""
    result = subprocess.run(
        ["git", *arguments],
        capture_output=True,
        text=True,
        check=True,
        env=env,
    )
    return result.stdout.strip()


def _make_upstream(path: Path, env: dict[str, str]) -> Path:
    """Create a bare upstream repository holding one commit."""
    work = path.parent / f"{path.name}-work"
    _git("init", "--quiet", str(work), env=env)
    (work / "README.md").write_text("clippy\n", encoding="utf-8")
    _git("-C", str(work), "add", "README.md", env=env)
    _git("-C", str(work), "commit", "--quiet", "-m", "seed", env=env)
    _git("clone", "--quiet", "--bare", str(work), str(path), env=env)
    return path


def _make_harness(tmp_path: Path) -> Harness:
    """Build a harness whose upstream URL resolves to a local repository."""
    home = tmp_path / "home"
    home.mkdir()
    root = tmp_path / "cache" / "whitaker-mirrors"
    root.mkdir(parents=True)
    output = tmp_path / "github-output"
    output.touch()
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(home),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_AUTHOR_NAME": "Whitaker Tests",
        "GIT_AUTHOR_EMAIL": "tests@example.invalid",
        "GIT_COMMITTER_NAME": "Whitaker Tests",
        "GIT_COMMITTER_EMAIL": "tests@example.invalid",
        "CLIPPY_MIRROR_ROOT": str(root),
        "GITHUB_OUTPUT": str(output),
    }
    upstream = _make_upstream(tmp_path / "upstream.git", env)
    (home / ".gitconfig").write_text(
        f'[init]\n\tdefaultBranch = main\n[url "{upstream}"]\n'
        f"\tinsteadOf = {CLIPPY_URL}\n",
        encoding="utf-8",
    )
    return Harness(
        home=home,
        upstream=upstream,
        root=root,
        mirror=root / MIRROR_NAME,
        output=output,
        env=env,
    )


def _run(
    harness: Harness,
    mirror: Path | None = None,
) -> subprocess.CompletedProcess[str]:
    """Invoke the script for one mirror path."""
    target = harness.mirror if mirror is None else mirror
    return subprocess.run(
        ["bash", str(SCRIPT), str(target)],
        capture_output=True,
        text=True,
        check=False,
        env=harness.env,
    )


def _hit(harness: Harness) -> str:
    """Return the recorded ``clippy-mirror-hit`` value."""
    lines = harness.output.read_text(encoding="utf-8").splitlines()
    values = [
        line.removeprefix("clippy-mirror-hit=")
        for line in lines
        if line.startswith("clippy-mirror-hit=")
    ]
    assert len(values) == 1, f"expected one hit line, got {lines}"
    return values[0]


def _seed_valid_mirror(harness: Harness) -> None:
    """Clone a valid generation into the cache root without the script."""
    _git(
        "clone",
        "--quiet",
        "--mirror",
        CLIPPY_URL,
        str(harness.mirror),
        env=harness.env,
    )
    (harness.mirror / SENTINEL).write_text("kept\n", encoding="utf-8")


def _is_bare(harness: Harness) -> str:
    """Return git's bare-repository verdict for the provisioned mirror."""
    return _git(
        "-C", str(harness.mirror), "rev-parse", "--is-bare-repository", env=harness.env
    )


def test_cold_root_clones_the_mirror_once(tmp_path: Path) -> None:
    """An empty cache root is provisioned and reported as a miss."""
    harness = _make_harness(tmp_path)

    result = _run(harness)

    assert result.returncode == 0, result.stderr
    assert _hit(harness) == "false", "a cold root must report a cache miss"
    assert _is_bare(harness) == "true", "the mirror must be a bare repository"
    origin = _git(
        "-C",
        str(harness.mirror),
        "config",
        "--get",
        "remote.origin.url",
        env=harness.env,
    )
    assert origin == CLIPPY_URL, f"the mirror must track {CLIPPY_URL}, got {origin}"
    rewritten = _git(
        "config",
        "--global",
        "--get-all",
        f"url.{harness.mirror}.insteadOf",
        env=harness.env,
    )
    assert rewritten.splitlines() == [CLIPPY_URL, f"{CLIPPY_URL}.git"], (
        f"both upstream spellings must resolve to the mirror, got {rewritten!r}"
    )


def test_restored_generation_is_reused_untouched(tmp_path: Path) -> None:
    """A valid restored mirror is refreshed in place and reported as a hit."""
    harness = _make_harness(tmp_path)
    _seed_valid_mirror(harness)

    result = _run(harness)

    assert result.returncode == 0, result.stderr
    assert _hit(harness) == "true", "a restored generation must report a hit"
    assert (harness.mirror / SENTINEL).exists(), (
        "a reused generation must not be discarded and re-cloned"
    )


@pytest.mark.parametrize("stale", ["non-bare", "wrong-origin"])
def test_unusable_generation_is_replaced(tmp_path: Path, stale: str) -> None:
    """A generation git cannot vouch for is rebuilt, not reused."""
    harness = _make_harness(tmp_path)
    if stale == "non-bare":
        _git("init", "--quiet", str(harness.mirror), env=harness.env)
    else:
        other = _make_upstream(tmp_path / "other.git", harness.env)
        _git(
            "clone",
            "--quiet",
            "--mirror",
            str(other),
            str(harness.mirror),
            env=harness.env,
        )
    (harness.mirror / SENTINEL).write_text("stale\n", encoding="utf-8")

    result = _run(harness)

    assert result.returncode == 0, result.stderr
    assert _hit(harness) == "false", "an unusable generation must report a miss"
    assert not (harness.mirror / SENTINEL).exists(), (
        "the stale generation must be discarded before the clone"
    )
    assert _is_bare(harness) == "true", "the replacement must be a bare mirror"


def test_path_outside_the_cache_root_is_refused(tmp_path: Path) -> None:
    """A matching directory name outside the root is never managed."""
    harness = _make_harness(tmp_path)
    intruder = tmp_path / "project" / MIRROR_NAME
    intruder.mkdir(parents=True)
    (intruder / "important.txt").write_text("keep me\n", encoding="utf-8")

    result = _run(harness, mirror=intruder)

    assert result.returncode == 1, result.stdout
    assert "refusing to manage" in result.stderr, result.stderr
    assert (intruder / "important.txt").exists(), (
        "an unrelated directory must survive the refusal"
    )


def test_wrongly_named_child_of_the_root_is_refused(tmp_path: Path) -> None:
    """Only the one mirror name inside the root is managed."""
    harness = _make_harness(tmp_path)

    result = _run(harness, mirror=harness.root / "other.git")

    assert result.returncode == 1, result.stdout
    assert "refusing to manage" in result.stderr, result.stderr


def test_non_directory_at_the_mirror_path_aborts(tmp_path: Path) -> None:
    """A file where the mirror belongs is a machine fault, not a stale cache."""
    harness = _make_harness(tmp_path)
    harness.mirror.write_text("not a repository\n", encoding="utf-8")

    result = _run(harness)

    assert result.returncode == 1, result.stdout
    assert "refusing to discard it" in result.stderr, result.stderr
    assert harness.mirror.is_file(), "the unexpected file must be left in place"


def test_missing_github_output_still_provisions(tmp_path: Path) -> None:
    """The hit record is optional; provisioning does not depend on it."""
    harness = _make_harness(tmp_path)
    env = dict(harness.env)
    del env["GITHUB_OUTPUT"]
    stripped = dc.replace(harness, env=env)

    result = _run(stripped)

    assert result.returncode == 0, result.stderr
    assert _is_bare(stripped) == "true", "the mirror must still be provisioned"
    assert harness.output.read_text(encoding="utf-8") == "", (
        "no hit may be recorded when GITHUB_OUTPUT is unset"
    )


@pytest.mark.parametrize(
    "arguments",
    [pytest.param([], id="none"), pytest.param(["a", "b"], id="two")],
)
def test_rejects_wrong_argument_count(arguments: list[str]) -> None:
    """The script demands exactly one mirror path."""
    result = subprocess.run(
        ["bash", str(SCRIPT), *arguments],
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2, result.stdout
    assert "usage:" in result.stderr, result.stderr


def test_script_passes_shellcheck() -> None:
    """The provisioning script stays clean under the repository linter."""
    if shutil.which("shellcheck") is None:
        pytest.skip("shellcheck is not installed")
    result = subprocess.run(
        ["shellcheck", str(SCRIPT)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stdout
