"""Test the prune-safety checker end to end through its entry point.

Run with ``make test-prune-safety``.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

import pytest

from prune_safety_support import (
    ARCHIVE,
    OLD_CRATES,
    SCRIPTS,
    TARGET,
    _check,
    _publish,
    checker,
)


def _raising_run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    """Stand in for ``_run`` with a tool that cannot be started."""
    raise checker.PruneSafetyError(f"cannot run {argv[0]}: not found")


def test_suite_only_after_prune_passes(repository: Path, tmp_path: Path) -> None:
    """An archive of known crates plus the suite is safe."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    status = _check(repository, tmp_path / "dist")
    assert status == 0


def test_unknown_standalone_library_fails(
    repository: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The #565 shape: a crate the installer cannot prune survives and fails."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "newcomer", "whitaker_suite"])
    status = _check(repository, tmp_path / "dist")
    assert status == 1
    assert "newcomer" in capsys.readouterr().err


def test_archive_without_suite_fails(repository: Path, tmp_path: Path) -> None:
    """An archive that leaves nothing after the prune is also refused."""
    _publish(tmp_path / "dist", list(OLD_CRATES))
    status = _check(repository, tmp_path / "dist")
    assert status == 1


def test_missing_tag_is_an_error_not_a_pass(repository: Path, tmp_path: Path) -> None:
    """Failing to read the installer's list must stop the publish."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    status = _check(repository, tmp_path / "dist", "v9.9.9")
    assert status == 2


def test_empty_directory_is_an_error(repository: Path, tmp_path: Path) -> None:
    """No archives at all is not a pass."""
    (tmp_path / "dist").mkdir()
    status = _check(repository, tmp_path / "dist")
    assert status == 2


@pytest.mark.parametrize(
    "manifest",
    [
        pytest.param(["libalpha.so"], id="missing-entry"),
        pytest.param(["libalpha.so", "whitaker_suite.dll"], id="different-file-name"),
    ],
)
def test_manifest_disagreeing_with_archive_fails(
    repository: Path,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    manifest: list[str],
) -> None:
    """A manifest that is not the archive's exact file list is refused."""
    _publish(tmp_path / "dist", ["alpha", "whitaker_suite"], manifest=manifest)
    assert _check(repository, tmp_path / "dist") == 1
    assert "manifest" in capsys.readouterr().err


def test_unsafe_archive_after_a_safe_one_still_fails(
    repository: Path, tmp_path: Path
) -> None:
    """Every archive in the directory is checked, not only the first."""
    dist = tmp_path / "dist"
    _publish(dist, [*OLD_CRATES, "whitaker_suite"])
    safe = dist / ARCHIVE
    safe.rename(
        dist / ARCHIVE.replace("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")
    )
    (dist / f"manifest-{TARGET}.json").unlink()
    _publish_second = tmp_path / "other"
    _publish(_publish_second, [*OLD_CRATES, "newcomer", "whitaker_suite"])
    (_publish_second / ARCHIVE).rename(dist / ARCHIVE)
    status = _check(repository, dist)
    assert status == 1


def test_missing_tool_is_an_error(
    repository: Path, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A missing ``tar`` stops the publish with status 2, not a traceback."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    monkeypatch.setenv("PATH", str(tmp_path / "empty"))
    monkeypatch.setattr(checker, "_run", _raising_run)
    status = _check(repository, tmp_path / "dist")
    assert status == 2


def test_manifest_with_non_object_root_is_an_error(
    repository: Path, tmp_path: Path
) -> None:
    """A manifest whose JSON root is not an object returns status 2."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    (tmp_path / "dist" / f"manifest-{TARGET}.json").write_text(
        "[1, 2]", encoding="utf-8"
    )
    status = _check(repository, tmp_path / "dist")
    assert status == 2


def test_command_line_exit_statuses() -> None:
    """The script run as a process exits 2 on an unusable invocation."""
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPTS / "check_prune_safety.py"),
            "--installer-tag",
            "v9.9.9",
            "/nonexistent",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 2
    assert "error:" in result.stderr


def test_failed_guard_exits_non_zero_as_a_process(
    repository: Path, tmp_path: Path
) -> None:
    """Run as a process, an unsafe archive exits 1 and a safe one exits 0."""
    script = str(SCRIPTS / "check_prune_safety.py")
    arguments = ["--installer-tag", "v0.0.1", "--repository", str(repository)]
    _publish(tmp_path / "safe", [*OLD_CRATES, "whitaker_suite"])
    _publish(tmp_path / "unsafe", [*OLD_CRATES, "newcomer", "whitaker_suite"])
    safe = subprocess.run(
        [sys.executable, script, *arguments, str(tmp_path / "safe")], check=False
    )
    unsafe = subprocess.run(
        [sys.executable, script, *arguments, str(tmp_path / "unsafe")], check=False
    )
    assert (safe.returncode, unsafe.returncode) == (0, 1)


def test_missing_tar_at_process_start_is_an_error(
    repository: Path, tmp_path: Path
) -> None:
    """With no ``tar`` on PATH the process exits 2 rather than raising."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    git = shutil.which("git")
    assert git is not None
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    (bin_dir / "git").symlink_to(git)
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPTS / "check_prune_safety.py"),
            "--installer-tag",
            "v0.0.1",
            "--repository",
            str(repository),
            str(tmp_path / "dist"),
        ],
        env={"PATH": str(bin_dir)},
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 2
    assert "tar" in result.stderr


def test_archive_without_its_manifest_fails(
    repository: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """A missing sidecar cannot let an archive skip the manifest checks."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    (tmp_path / "dist" / f"manifest-{TARGET}.json").unlink()
    assert _check(repository, tmp_path / "dist") == 1
    assert "sidecar" in capsys.readouterr().err


def test_archive_with_an_unrecognized_name_is_an_error(
    repository: Path, tmp_path: Path
) -> None:
    """A name that does not follow the convention stops the publish."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    (tmp_path / "dist" / ARCHIVE).rename(
        tmp_path / "dist" / "whitaker-lints-odd.tar.zst"
    )
    assert _check(repository, tmp_path / "dist") == 2
