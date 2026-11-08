"""Test the prune-safety checker that guards the rolling-release publish.

Run with ``make test-prune-safety``.
"""

from __future__ import annotations

import importlib.util
import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
_SPEC = importlib.util.spec_from_file_location(
    "check_prune_safety", SCRIPTS / "check_prune_safety.py"
)
assert _SPEC is not None
assert _SPEC.loader is not None
checker = importlib.util.module_from_spec(_SPEC)
sys.modules["check_prune_safety"] = checker
_SPEC.loader.exec_module(checker)

OLD_CRATES = ["alpha", "beta"]
TARGET = "x86_64-unknown-linux-gnu"
ARCHIVE = f"whitaker-lints-abc1234-nightly-2026-05-28-{TARGET}.tar.zst"


def _git(repository: Path, *arguments: str) -> None:
    """Run git in ``repository``, failing the test on any error."""
    subprocess.run(["git", "-C", str(repository), *arguments], check=True, capture_output=True)


@pytest.fixture
def repository(tmp_path: Path) -> Path:
    """Create a repository whose ``v0.0.1`` tag lists ``OLD_CRATES``."""
    root = tmp_path / "repo"
    (root / "installer/src").mkdir(parents=True)
    names = ", ".join(f'"{name}"' for name in OLD_CRATES)
    (root / "installer/src/resolution.rs").write_text(
        f"pub const LINT_CRATES: &[&str] = &[{names}];\n", encoding="utf-8"
    )
    _git(root, "init", "-q")
    _git(root, "add", "-A")
    _git(
        root,
        "-c", "user.name=t", "-c", "user.email=t@example.invalid",
        "commit", "-q", "-m", "tag",
    )
    _git(root, "tag", "v0.0.1")
    return root


def _publish(
    directory: Path,
    libraries: list[str],
    manifest: list[str] | None = None,
    toolchain: str = "nightly-2026-05-28",
) -> None:
    """Write an archive holding ``libraries`` and its sidecar manifest."""
    directory.mkdir(exist_ok=True)
    staging = directory / "staging"
    staging.mkdir()
    for crate in libraries:
        (staging / f"lib{crate}.so").write_bytes(b"x")
    subprocess.run(
        ["tar", "--zstd", "-cf", str(directory / ARCHIVE), "-C", str(staging), "."],
        check=True,
    )
    for crate in libraries:
        (staging / f"lib{crate}.so").unlink()
    staging.rmdir()
    files = [f"lib{crate}.so" for crate in libraries] if manifest is None else manifest
    (directory / f"manifest-{TARGET}.json").write_text(
        json.dumps({"files": files, "toolchain": toolchain, "git_sha": "abc1234", "target": TARGET}), encoding="utf-8"
    )


def _check(repository: Path, directory: Path, tag: str = "v0.0.1") -> int:
    """Run the checker's entry point against ``directory`` and return its status."""
    return checker.main(
        ["--installer-tag", tag, "--repository", str(repository), str(directory)]
    )


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


def test_windows_library_with_lib_prefix_is_not_mistaken_for_a_known_crate() -> None:
    """The installer prunes ``alpha@...dll``, not a Windows ``libalpha.dll``."""
    windows = "x86_64-pc-windows-msvc"
    assert checker.crate_of_entry("libalpha.dll", windows) == "libalpha"
    assert checker.surviving_crates(
        ["libalpha.dll", "whitaker_suite.dll"], ["alpha"], windows
    ) == ["libalpha", "whitaker_suite"]


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


def test_experimental_crates_the_installer_prunes_are_known() -> None:
    """The prune also removes ``EXPERIMENTAL_LINT_CRATES``, so they are known."""
    source = (
        'pub const LINT_CRATES: &[&str] = &["alpha"];\n'
        'pub const EXPERIMENTAL_LINT_CRATES: &[&str] = &["exp"];\n'
    )
    known = checker.parse_lint_crates(source)
    assert checker.surviving_crates(["libalpha.so", "libexp.so", "libwhitaker_suite.so"], known) == [
        "whitaker_suite"
    ]


def test_unsafe_archive_after_a_safe_one_still_fails(repository: Path, tmp_path: Path) -> None:
    """Every archive in the directory is checked, not only the first."""
    dist = tmp_path / "dist"
    _publish(dist, [*OLD_CRATES, "whitaker_suite"])
    safe = dist / ARCHIVE
    safe.rename(dist / ARCHIVE.replace("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"))
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


def _raising_run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    """Stand in for ``_run`` with a tool that cannot be started."""
    raise checker.PruneSafetyError(f"cannot run {argv[0]}: not found")


def test_manifest_with_non_object_root_is_an_error(repository: Path, tmp_path: Path) -> None:
    """A manifest whose JSON root is not an object returns status 2."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    (tmp_path / "dist" / f"manifest-{TARGET}.json").write_text("[1, 2]", encoding="utf-8")
    status = _check(repository, tmp_path / "dist")
    assert status == 2


def test_command_line_exit_statuses() -> None:
    """The script run as a process exits 2 on an unusable invocation."""
    result = subprocess.run(
        [sys.executable, str(SCRIPTS / "check_prune_safety.py"), "--installer-tag", "v9.9.9", "/nonexistent"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 2
    assert "error:" in result.stderr


def test_non_canonical_library_name_is_not_mistaken_for_a_known_crate() -> None:
    """``libalpha.extra.so`` is not pruned as ``alpha`` and so must survive."""
    left = checker.surviving_crates(["libalpha.extra.so", "libwhitaker_suite.so"], ["alpha"])
    assert left == ["libalpha.extra.so", "whitaker_suite"]


def test_failed_guard_exits_non_zero_as_a_process(repository: Path, tmp_path: Path) -> None:
    """Run as a process, an unsafe archive exits 1 and a safe one exits 0."""
    script = str(SCRIPTS / "check_prune_safety.py")
    arguments = ["--installer-tag", "v0.0.1", "--repository", str(repository)]
    _publish(tmp_path / "safe", [*OLD_CRATES, "whitaker_suite"])
    _publish(tmp_path / "unsafe", [*OLD_CRATES, "newcomer", "whitaker_suite"])
    safe = subprocess.run([sys.executable, script, *arguments, str(tmp_path / "safe")], check=False)
    unsafe = subprocess.run([sys.executable, script, *arguments, str(tmp_path / "unsafe")], check=False)
    assert (safe.returncode, unsafe.returncode) == (0, 1)


def test_missing_tar_at_process_start_is_an_error(repository: Path, tmp_path: Path) -> None:
    """With no ``tar`` on PATH the process exits 2 rather than raising."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    git = shutil.which("git")
    assert git is not None
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    (bin_dir / "git").symlink_to(git)
    result = subprocess.run(
        [
            sys.executable, str(SCRIPTS / "check_prune_safety.py"),
            "--installer-tag", "v0.0.1", "--repository", str(repository), str(tmp_path / "dist"),
        ],
        env={"PATH": str(bin_dir)},
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 2
    assert "tar" in result.stderr


def _table(tmp_path: Path, toolchain: str) -> Path:
    """Write a snapshot table mapping installer 0.0.1 to ``toolchain``."""
    table = tmp_path / "clippy-snapshots.tsv"
    table.write_text(f"# header\n0.0.1 abc 6.3.0 {toolchain}\n", encoding="utf-8")
    return table


def test_toolchain_matching_the_snapshot_row_passes(repository: Path, tmp_path: Path) -> None:
    """A manifest toolchain equal to the table's passes."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    table = _table(tmp_path, "nightly-2026-05-28")
    status = checker.main(
        ["--installer-tag", "v0.0.1", "--repository", str(repository),
         "--snapshots-tsv", str(table), str(tmp_path / "dist")]
    )
    assert status == 0


def test_toolchain_moving_ahead_of_the_snapshot_row_fails(
    repository: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The #555 shape: a toolchain bump the oldest installer's row lacks."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    table = _table(tmp_path, "nightly-old")
    status = checker.main(
        ["--installer-tag", "v0.0.1", "--repository", str(repository),
         "--snapshots-tsv", str(table), str(tmp_path / "dist")]
    )
    assert status == 1
    err = capsys.readouterr().err
    assert "nightly-2026-05-28" in err
    assert "nightly-old" in err


def test_snapshot_table_without_the_installer_row_is_an_error(
    repository: Path, tmp_path: Path
) -> None:
    """No row for the installer means the relationship cannot be checked."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    table = tmp_path / "t.tsv"
    table.write_text("9.9.9 a b c\n", encoding="utf-8")
    status = checker.main(
        ["--installer-tag", "v0.0.1", "--repository", str(repository),
         "--snapshots-tsv", str(table), str(tmp_path / "dist")]
    )
    assert status == 2


@pytest.mark.parametrize(
    "entry",
    ["libalpha@nightly-2020-01-01.so", "whitaker_suite.txt", "libalpha.extra.so"],
)
def test_non_canonical_entries_are_not_pruned_as_known_crates(entry: str) -> None:
    """Only canonical library names are pruned; anything else survives."""
    assert checker.surviving_crates([entry, "libwhitaker_suite.so"], ["alpha"]) == sorted(
        [entry, "whitaker_suite"]
    )


def test_archive_without_its_manifest_fails(
    repository: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """A missing sidecar cannot let an archive skip the manifest checks."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    (tmp_path / "dist" / f"manifest-{TARGET}.json").unlink()
    assert _check(repository, tmp_path / "dist") == 1
    assert "sidecar" in capsys.readouterr().err


@pytest.mark.parametrize("entry", ["whitaker_suite.so", "module_max_lines.so"])
def test_linux_library_without_the_lib_prefix_is_not_prunable_or_the_suite(entry: str) -> None:
    """The installer neither stages nor prunes a Linux name lacking `lib`."""
    left = checker.surviving_crates([entry], ["module_max_lines"])
    assert left == [entry]
    assert left != [checker.SUITE_CRATE]


@pytest.mark.parametrize(
    ("target", "entry", "crate"),
    [
        ("x86_64-pc-windows-msvc", "whitaker_suite.dll", "whitaker_suite"),
        ("x86_64-pc-windows-msvc", "libwhitaker_suite.dll", "libwhitaker_suite"),
        ("x86_64-pc-windows-msvc", "whitaker_suite.so", "whitaker_suite.so"),
        ("aarch64-apple-darwin", "libwhitaker_suite.dylib", "whitaker_suite"),
        ("aarch64-apple-darwin", "libwhitaker_suite.so", "libwhitaker_suite.so"),
        ("x86_64-unknown-linux-gnu", "libwhitaker_suite.dylib", "libwhitaker_suite.dylib"),
    ],
)
def test_library_names_follow_the_archive_targets_rules(target: str, entry: str, crate: str) -> None:
    """Prefix and extension are the target's, as the installer derives them."""
    assert checker.crate_of_entry(entry, target) == crate


@pytest.mark.parametrize(
    ("field", "value"),
    [("git_sha", "fffffff"), ("toolchain", "nightly-2020-01-01"), ("target", "aarch64-apple-darwin")],
)
def test_archive_name_must_match_its_manifest(
    repository: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str], field: str, value: str
) -> None:
    """The name the installer derives from the manifest must be the archive's."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    manifest = tmp_path / "dist" / f"manifest-{TARGET}.json"
    data = json.loads(manifest.read_text(encoding="utf-8"))
    data[field] = value
    manifest.write_text(json.dumps(data), encoding="utf-8")
    assert _check(repository, tmp_path / "dist") == 1
    assert field in capsys.readouterr().err


def test_archive_with_an_unrecognised_name_is_an_error(repository: Path, tmp_path: Path) -> None:
    """A name that does not follow the convention stops the publish."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    (tmp_path / "dist" / ARCHIVE).rename(tmp_path / "dist" / "whitaker-lints-odd.tar.zst")
    assert _check(repository, tmp_path / "dist") == 2
