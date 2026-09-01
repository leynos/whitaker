"""Test the ELF glibc-baseline checker.

Run with ``uv run pytest scripts/tests/test_check_glibc_baseline.py``.
"""

from __future__ import annotations

import importlib.util
import subprocess
import types
from pathlib import Path
from typing import Protocol, cast

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
GlibcVersion = tuple[int, int]


class Checker(Protocol):
    """Describe the imported checker surface used by these tests."""

    subprocess: types.ModuleType

    def main(self, arguments: list[str] | None = None) -> int:
        """Run the checker with the supplied arguments."""
        ...

    def parse_arguments(self, arguments: list[str] | None = None) -> object:
        """Parse the supplied checker arguments."""
        ...

    def read_required_glibc_versions(self, path: Path) -> tuple[GlibcVersion, ...]:
        """Read the GLIBC requirements for the supplied ELF path."""
        ...


@pytest.fixture(scope="module")
def checker() -> Checker:
    """Import the glibc checker script as a module."""
    spec = importlib.util.spec_from_file_location(
        "check_glibc_baseline", SCRIPTS / "check_glibc_baseline.py"
    )
    if spec is None or spec.loader is None:
        message = "could not load the glibc baseline checker"
        raise ImportError(message)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return cast("Checker", module)


@pytest.fixture
def elf_path(tmp_path: Path) -> Path:
    """Create a placeholder path accepted by the mocked ELF reader."""
    path = tmp_path / "binary"
    path.write_bytes(b"\x7fELF")
    return path


def version_needs(*versions: str) -> str:
    """Build minimal readelf output containing the supplied GLIBC requirements."""
    names = "\n".join(
        f"  0x00000000: Name: {version} Flags: none Version: 2" for version in versions
    )
    return f"Version needs section '.gnu.version_r' contains 1 entry:\n{names}\n"


def stub_readelf(
    monkeypatch: pytest.MonkeyPatch, checker: Checker, outputs: dict[Path, str]
) -> None:
    """Replace readelf with deterministic output keyed by the inspected path."""

    def run(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
        path = Path(command[-1])
        return subprocess.CompletedProcess(command, 0, outputs[path], "")

    monkeypatch.setattr(checker.subprocess, "run", run)


@pytest.mark.parametrize(
    "case",
    [
        (("GLIBC_2.17",), ((2, 17),)),
        (("GLIBC_2.17", "GLIBC_2.35"), ((2, 17), (2, 35))),
    ],
    ids=["older-baseline", "baseline"],
)
def test_read_required_glibc_versions_accepts_allowed_versions(
    checker: Checker,
    elf_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    case: tuple[tuple[str, ...], tuple[GlibcVersion, ...]],
) -> None:
    """Accept GLIBC requirements at or below the supported baseline."""
    versions, expected = case
    stub_readelf(monkeypatch, checker, {elf_path: version_needs(*versions)})

    assert checker.read_required_glibc_versions(elf_path) == expected


def test_main_rejects_glibc_239(
    checker: Checker,
    elf_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Reject a binary requiring a glibc version newer than the baseline."""
    stub_readelf(monkeypatch, checker, {elf_path: version_needs("GLIBC_2.39")})

    assert checker.main([str(elf_path)]) == 1
    captured = capsys.readouterr()
    assert "maximum required GLIBC version: GLIBC_2.39" in captured.out
    assert "GLIBC_2.39" in captured.err
    assert "GLIBC_2.35" in captured.err


def test_main_checks_multiple_files(
    checker: Checker,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Report every input, even when one exceeds the baseline."""
    allowed = tmp_path / "allowed"
    rejected = tmp_path / "rejected"
    _ = allowed.write_bytes(b"\x7fELF")
    _ = rejected.write_bytes(b"\x7fELF")
    stub_readelf(
        monkeypatch,
        checker,
        {
            allowed: version_needs("GLIBC_2.35"),
            rejected: version_needs("GLIBC_2.39"),
        },
    )

    assert checker.main([str(allowed), str(rejected)]) == 1
    output = capsys.readouterr()
    assert str(allowed) in output.out
    assert str(rejected) in output.out


def test_main_accepts_elf_without_glibc_references(
    checker: Checker,
    elf_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Accept an ELF binary with no versioned dynamic-library requirements."""
    stub_readelf(
        monkeypatch,
        checker,
        {
            elf_path: (
                "Version needs section '.gnu.version_r' contains 1 entry:\n"
                "  0x00000000: Name: GCC_3.0 Flags: none Version: 2\n"
            )
        },
    )

    assert checker.main([str(elf_path)]) == 0
    assert "maximum required GLIBC version: none" in capsys.readouterr().out


def test_main_rejects_readelf_failure(
    checker: Checker,
    elf_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Fail closed when readelf rejects an input as non-ELF."""

    def run(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(command, 1, "", "not an ELF file")

    monkeypatch.setattr(checker.subprocess, "run", run)

    assert checker.main([str(elf_path)]) == 2
    assert "could not inspect ELF file" in capsys.readouterr().err


def test_main_rejects_missing_file(
    checker: Checker, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Fail closed when an input path does not exist."""
    assert checker.main([str(tmp_path / "missing")]) == 2
    assert "not a readable file" in capsys.readouterr().err


def test_main_rejects_unparsable_version_information(
    checker: Checker,
    elf_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Fail closed when readelf succeeds without parsable ELF metadata."""
    stub_readelf(monkeypatch, checker, {elf_path: "unrecognized output"})

    assert checker.main([str(elf_path)]) == 2
    assert "could not parse ELF version information" in capsys.readouterr().err


@pytest.mark.parametrize("value", ["2.35", "GLIBC_2", "GLIBC_2.35.1"])
def test_parse_glibc_version_rejects_malformed_baseline(
    checker: Checker, value: str
) -> None:
    """Reject a baseline that is not an exact GLIBC_X.Y version."""
    with pytest.raises(SystemExit):
        checker.parse_arguments(["--maximum-glibc", value, "binary"])
