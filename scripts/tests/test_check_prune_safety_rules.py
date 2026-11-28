"""Test the prune-safety checker's pure naming and toolchain rules.

Run with ``make test-prune-safety``.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from prune_safety_support import (
    OLD_CRATES,
    TARGET,
    _check,
    _publish,
    _table,
    checker,
    rules,
)


def test_windows_library_with_lib_prefix_is_not_mistaken_for_a_known_crate() -> None:
    """The installer prunes ``alpha@...dll``, not a Windows ``libalpha.dll``."""
    windows = "x86_64-pc-windows-msvc"
    assert rules.crate_of_entry("libalpha.dll", windows) == "libalpha"
    assert checker.surviving_crates(
        ["libalpha.dll", "whitaker_suite.dll"], ["alpha"], windows
    ) == ["libalpha", "whitaker_suite"]


def test_experimental_crates_the_installer_prunes_are_known() -> None:
    """The prune also removes ``EXPERIMENTAL_LINT_CRATES``, so they are known."""
    source = (
        'pub const LINT_CRATES: &[&str] = &["alpha"];\n'
        'pub const EXPERIMENTAL_LINT_CRATES: &[&str] = &["exp"];\n'
    )
    known = checker.parse_lint_crates(source)
    assert checker.surviving_crates(
        ["libalpha.so", "libexp.so", "libwhitaker_suite.so"], known
    ) == ["whitaker_suite"]


def test_non_canonical_library_name_is_not_mistaken_for_a_known_crate() -> None:
    """``libalpha.extra.so`` is not pruned as ``alpha`` and so must survive."""
    left = checker.surviving_crates(
        ["libalpha.extra.so", "libwhitaker_suite.so"], ["alpha"]
    )
    assert left == ["libalpha.extra.so", "whitaker_suite"]


def test_toolchain_matching_the_snapshot_row_passes(
    repository: Path, tmp_path: Path
) -> None:
    """A manifest toolchain equal to the table's passes."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    table = _table(tmp_path, "nightly-2026-05-28")
    status = checker.main(
        [
            "--installer-tag",
            "v0.0.1",
            "--repository",
            str(repository),
            "--snapshots-tsv",
            str(table),
            str(tmp_path / "dist"),
        ]
    )
    assert status == 0


def test_toolchain_moving_ahead_of_the_snapshot_row_fails(
    repository: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The #555 shape: a toolchain bump the oldest installer's row lacks."""
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    table = _table(tmp_path, "nightly-old")
    status = checker.main(
        [
            "--installer-tag",
            "v0.0.1",
            "--repository",
            str(repository),
            "--snapshots-tsv",
            str(table),
            str(tmp_path / "dist"),
        ]
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
        [
            "--installer-tag",
            "v0.0.1",
            "--repository",
            str(repository),
            "--snapshots-tsv",
            str(table),
            str(tmp_path / "dist"),
        ]
    )
    assert status == 2


@pytest.mark.parametrize(
    "entry",
    ["libalpha@nightly-2020-01-01.so", "whitaker_suite.txt", "libalpha.extra.so"],
)
def test_non_canonical_entries_are_not_pruned_as_known_crates(entry: str) -> None:
    """Only canonical library names are pruned; anything else survives."""
    assert checker.surviving_crates(
        [entry, "libwhitaker_suite.so"], ["alpha"]
    ) == sorted([entry, "whitaker_suite"])


@pytest.mark.parametrize("entry", ["whitaker_suite.so", "module_max_lines.so"])
def test_linux_library_without_the_lib_prefix_is_not_prunable_or_the_suite(
    entry: str,
) -> None:
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
        (
            "x86_64-unknown-linux-gnu",
            "libwhitaker_suite.dylib",
            "libwhitaker_suite.dylib",
        ),
    ],
)
def test_library_names_follow_the_archive_targets_rules(
    target: str, entry: str, crate: str
) -> None:
    """Prefix and extension are the target's, as the installer derives them."""
    assert rules.crate_of_entry(entry, target) == crate


@pytest.mark.parametrize(
    "change",
    [
        pytest.param(("git_sha", "fffffff"), id="sha"),
        pytest.param(("toolchain", "nightly-2020-01-01"), id="toolchain"),
        pytest.param(("target", "aarch64-apple-darwin"), id="target"),
    ],
)
def test_archive_name_must_match_its_manifest(
    repository: Path, tmp_path: Path, change: tuple[str, str]
) -> None:
    """The name the installer derives from the manifest must be the archive's."""
    field, value = change
    _publish(tmp_path / "dist", [*OLD_CRATES, "whitaker_suite"])
    manifest = tmp_path / "dist" / f"manifest-{TARGET}.json"
    data = json.loads(manifest.read_text(encoding="utf-8"))
    data[field] = value
    manifest.write_text(json.dumps(data), encoding="utf-8")
    assert _check(repository, tmp_path / "dist") == 1
