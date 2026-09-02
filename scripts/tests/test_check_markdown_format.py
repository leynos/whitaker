"""Exercise the Markdown formatter checker at its process boundary."""

import json
import os
import subprocess  # noqa: S404 - the boundary is under test.
import sys
import tempfile
import textwrap
from pathlib import Path

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CHECKER = REPOSITORY_ROOT / "scripts" / "check-markdown-format.sh"
FORMATTER_FLAGS = [
    "--in-place",
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
]
LINE_ALPHABET = tuple("abcdefghijklmnopqrstuvwxyz0123456789 -_[]*")


def _write_fake_formatter(directory: Path) -> tuple[Path, Path]:
    """Create a formatter fixture that records calls and canonicalizes bytes."""
    executable = directory / "mdtablefix"
    call_log = directory / "formatter-calls.jsonl"
    executable.write_text(
        textwrap.dedent(
            """\
            #!__PYTHON__
            import json
            import os
            import pathlib
            import sys

            arguments = sys.argv[1:]
            if arguments == ["--version"]:
                print("mdtablefix 0.5.0")
                raise SystemExit(0)

            expected_flags = [
                "--in-place",
                "--wrap",
                "--renumber",
                "--breaks",
                "--ellipsis",
                "--fences",
            ]
            if arguments[:len(expected_flags)] != expected_flags:
                print("unexpected formatter flags", file=sys.stderr)
                raise SystemExit(64)

            paths = arguments[len(expected_flags):]
            if not paths:
                print("expected at least one Markdown path", file=sys.stderr)
                raise SystemExit(64)

            with pathlib.Path(os.environ["MDTABLEFIX_CALL_LOG"]).open("a") as log:
                print(json.dumps(arguments), file=log)

            for path in paths:
                source = pathlib.Path(path)
                canonical = source.read_bytes().replace(b"\\r\\n", b"\\n")
                canonical = canonical.replace(b"\\r", b"\\n")
                source.write_bytes(canonical.replace(b"unformatted", b"formatted"))
            """
        ).replace("__PYTHON__", sys.executable),
        encoding="utf-8",
    )
    executable.chmod(0o755)
    return executable, call_log


def _write_fake_markdown_linter(directory: Path) -> Path:
    """Create a Markdown linter fixture that changes only lint-only markers."""
    executable = directory / "markdownlint-cli2"
    executable.write_text(
        textwrap.dedent(
            """\
            #!__PYTHON__
            import pathlib
            import sys

            if len(sys.argv) < 3 or sys.argv[1] != "--fix":
                print("expected --fix and Markdown paths", file=sys.stderr)
                raise SystemExit(64)

            for path in sys.argv[2:]:
                source = pathlib.Path(path)
                source.write_bytes(
                    source.read_bytes().replace(
                        b"needs-markdownlint-fix", b"fixed-by-markdownlint"
                    )
                )
            """
        ).replace("__PYTHON__", sys.executable),
        encoding="utf-8",
    )
    executable.chmod(0o755)
    return executable


@pytest.fixture
def formatter(tmp_path: Path) -> tuple[Path, Path]:
    """Provide a controlled formatter executable and its call log."""
    return _write_fake_formatter(tmp_path)


def _run_checker(
    formatter: Path,
    call_log: Path,
    markdown_linter: Path,
    *files: Path,
) -> subprocess.CompletedProcess[str]:
    """Run the checker against files with the controlled formatter fixture."""
    environment = os.environ | {
        "MDTABLEFIX": str(formatter),
        "MDTABLEFIX_CALL_LOG": str(call_log),
        "MDLINT": str(markdown_linter),
    }
    return subprocess.run(  # noqa: S603 - executes the controlled fixture.
        [str(CHECKER), *(str(file) for file in files)],
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )


def test_requires_at_least_one_markdown_file(formatter: tuple[Path, Path]) -> None:
    """Reject an empty file list before trying to invoke the formatter."""
    executable, call_log = formatter

    result = _run_checker(
        executable, call_log, _write_fake_markdown_linter(executable.parent)
    )

    assert result.returncode == 64, result.stderr
    assert result.stderr == "Usage: check-markdown-format.sh <file>...\n"
    assert not call_log.exists(), "the formatter must not run for an empty file list"


def test_reports_a_missing_formatter(tmp_path: Path) -> None:
    """Explain how to supply the formatter when its executable is unavailable."""
    source = tmp_path / "source.md"
    source.write_text("formatted\n", encoding="utf-8")

    result = _run_checker(
        tmp_path / "missing-mdtablefix",
        tmp_path / "calls",
        _write_fake_markdown_linter(tmp_path),
        source,
    )

    assert result.returncode == 127, result.stderr
    assert result.stderr == (
        "check-markdown-format.sh: "
        f"'{tmp_path / 'missing-mdtablefix'}' is not installed or not on PATH.\n"
    )


def test_reports_a_missing_markdown_linter(
    formatter: tuple[Path, Path], tmp_path: Path
) -> None:
    """Explain how to supply the Markdown linter when its executable is unavailable."""
    executable, call_log = formatter
    source = tmp_path / "source.md"
    source.write_text("formatted\n", encoding="utf-8")
    missing_linter = tmp_path / "missing-markdownlint-cli2"

    result = _run_checker(executable, call_log, missing_linter, source)

    assert result.returncode == 127, result.stderr
    assert result.stderr == (
        "check-markdown-format.sh: "
        f"'{missing_linter}' is not installed or not on PATH.\n"
    )


def test_accepts_lf_and_crlf_without_modifying_sources(
    formatter: tuple[Path, Path],
    tmp_path: Path,
) -> None:
    """Accept exact canonical output in either Git checkout line-ending form."""
    executable, call_log = formatter
    markdown_linter = _write_fake_markdown_linter(executable.parent)
    lf_source = tmp_path / "lf.md"
    crlf_source = tmp_path / "crlf.md"
    lf_source.write_bytes(b"formatted\n")
    crlf_source.write_bytes(b"formatted\r\n")

    result = _run_checker(executable, call_log, markdown_linter, lf_source, crlf_source)

    assert result.returncode == 0, result.stderr
    assert lf_source.read_bytes() == b"formatted\n", (
        "the checker modified the LF source"
    )
    assert crlf_source.read_bytes() == b"formatted\r\n", (
        "the checker modified the CRLF source"
    )
    calls = [
        json.loads(line) for line in call_log.read_text(encoding="utf-8").splitlines()
    ]
    assert len(calls) == 1, "the checker must invoke the formatter once per batch"
    assert calls[0][: len(FORMATTER_FLAGS)] == FORMATTER_FLAGS, (
        "the checker must pass the canonical formatter flags"
    )
    staged_paths = [Path(path) for path in calls[0][len(FORMATTER_FLAGS) :]]
    assert [path.name for path in staged_paths] == ["0.md", "1.md"], (
        "the formatter must receive staged source copies"
    )
    assert all(path.parent != tmp_path for path in staged_paths), (
        "the formatter must not receive tracked source paths"
    )


def test_reports_each_noncanonical_source_without_modifying_it(
    formatter: tuple[Path, Path],
    tmp_path: Path,
) -> None:
    """Reject altered and mixed-ending files while leaving every source intact."""
    executable, call_log = formatter
    markdown_linter = _write_fake_markdown_linter(executable.parent)
    unformatted_source = tmp_path / "unformatted.md"
    mixed_source = tmp_path / "mixed.md"
    canonical_source = tmp_path / "canonical.md"
    unformatted_source.write_bytes(b"unformatted\n")
    mixed_source.write_bytes(b"formatted\r\nsecond line\n")
    canonical_source.write_bytes(b"formatted\n")

    result = _run_checker(
        executable,
        call_log,
        markdown_linter,
        unformatted_source,
        mixed_source,
        canonical_source,
    )

    assert result.returncode == 1, result.stderr
    assert result.stderr == (
        "The following Markdown files are not formatted; run 'make fmt':\n"
        f"  {unformatted_source}\n"
        f"  {mixed_source}\n"
    )
    assert unformatted_source.read_bytes() == b"unformatted\n", (
        "the checker modified the unformatted source"
    )
    assert mixed_source.read_bytes() == b"formatted\r\nsecond line\n", (
        "the checker modified the mixed-ending source"
    )
    assert canonical_source.read_bytes() == b"formatted\n", (
        "the checker modified the canonical source"
    )
    calls = [
        json.loads(line) for line in call_log.read_text(encoding="utf-8").splitlines()
    ]
    assert len(calls) == 1, "the checker must invoke the formatter once per batch"
    assert len(calls[0]) == len(FORMATTER_FLAGS) + 3, (
        "the formatter must receive every source in the batch"
    )


def test_detects_a_markdownlint_only_fix_without_modifying_the_source(
    formatter: tuple[Path, Path],
    tmp_path: Path,
) -> None:
    """Reject a source requiring only the Markdown lint fixing pass."""
    executable, call_log = formatter
    markdown_linter = _write_fake_markdown_linter(executable.parent)
    source = tmp_path / "lint-only.md"
    source.write_bytes(b"formatted\nneeds-markdownlint-fix\n")

    result = _run_checker(executable, call_log, markdown_linter, source)

    assert result.returncode == 1, result.stderr
    assert str(source) in result.stderr, result.stderr
    assert source.read_bytes() == b"formatted\nneeds-markdownlint-fix\n", (
        "the checker modified the source requiring a Markdown lint fix"
    )


@given(
    lines=st.lists(st.text(alphabet=LINE_ALPHABET, max_size=40), min_size=2, max_size=5)
)
@settings(database=None, deadline=None, derandomize=True)
def test_accepts_only_uniform_canonical_line_endings(lines: list[str]) -> None:
    """Accept LF and CRLF canonical forms while rejecting a mixed equivalent."""
    canonical = "\n".join(lines) + "\n"
    with tempfile.TemporaryDirectory() as directory_name:
        directory = Path(directory_name)
        executable, call_log = _write_fake_formatter(directory)
        markdown_linter = _write_fake_markdown_linter(directory)
        lf_source = directory / "lf.md"
        crlf_source = directory / "crlf.md"
        mixed_source = directory / "mixed.md"
        lf_source.write_text(canonical, encoding="utf-8")
        crlf_source.write_bytes(canonical.replace("\n", "\r\n").encode())
        mixed_source.write_bytes(
            (lines[0] + "\r\n" + "\n".join(lines[1:]) + "\n").encode()
        )

        passing = _run_checker(
            executable, call_log, markdown_linter, lf_source, crlf_source
        )
        failing = _run_checker(executable, call_log, markdown_linter, mixed_source)

    assert passing.returncode == 0, passing.stderr
    assert failing.returncode == 1, failing.stderr
    assert str(mixed_source) in failing.stderr, failing.stderr


def test_check_fmt_make_target_invokes_the_markdown_checker() -> None:
    """Keep the checked-in formatting gate connected to the checker."""
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8")
    makefile_lines = makefile.splitlines()
    target_index = makefile_lines.index("check-fmt: ## Verify formatting")
    recipe_lines: list[str] = []
    for line in makefile_lines[target_index + 1 :]:
        if line and not line.startswith(("\t", " ")):
            break
        if line.startswith("\t"):
            recipe_lines.append(line)
    recipe = "\n".join(recipe_lines)

    required_fragments = {
        "$(MD_FILES_FIND)": "discover Markdown files",
        "xargs -0": "preserve Markdown paths containing whitespace",
        'if [ "$$#" -gt 0 ]': "guard an empty Markdown file list",
        'scripts/check-markdown-format.sh "$$@"': "check every discovered file",
    }
    missing_requirements = [
        description
        for fragment, description in required_fragments.items()
        if fragment not in recipe
    ]
    assert not missing_requirements, "check-fmt does not " + ", ".join(
        missing_requirements
    )

    required_scan_fragments = {
        "-type d": "identify cache directories",
        "-prune": "avoid traversing cache directories",
        "-name target": "exclude nested build directories",
        "-name node_modules": "exclude nested dependency directories",
        "-type f -name '*.md' -print0": "emit only Markdown source files",
    }
    missing_scan_requirements = [
        description
        for fragment, description in required_scan_fragments.items()
        if fragment not in makefile
    ]
    assert not missing_scan_requirements, "Markdown discovery does not " + ", ".join(
        missing_scan_requirements
    )
