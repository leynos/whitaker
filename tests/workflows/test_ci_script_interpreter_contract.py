"""Hold repository scripts that CI runs to one interpreter convention.

The release workflow builds its x86_64 Linux artefacts on ``ubuntu-22.04`` for
the glibc baseline, and that image's ``python`` is 3.10. A script invoked there
as ``python scripts/thing.py`` therefore runs on an interpreter nobody chose,
and the first use of a newer standard-library feature fails at release time and
nowhere else. The repository's answer is a ``uv run --script`` shebang with an
explicit ``requires-python``, invoked directly.

These tests assert the commands rather than the file names, so deleting the
convention from a call site fails them.
"""

from __future__ import annotations

import re
import subprocess
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from collections.abc import Iterator

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_DIRECTORY = REPO_ROOT / ".github" / "workflows"
MAKEFILE = REPO_ROOT / "Makefile"

UV_SHEBANG = "#!/usr/bin/env -S uv run --script"
MINIMUM_PYTHON = (3, 13)

# A repository script run through an interpreter chosen by the image rather
# than by the script. `-m` module invocations are excluded: they name a module,
# not a repository file, and `uv run ... -m pytest` is the supported form.
_AMBIENT_INTERPRETER = re.compile(
    r"""(?<![\w./-])
        (?:python[0-9.]*|"\$\$PYTHON"|\$\$?\{?PYTHON\}?)
        \s+(?!-m\b)
        (?P<script>(?:[\w.@-]+/)*[\w.@-]+\.py)\b""",
    re.VERBOSE,
)

# A repository script in command position. Two shapes reach one: a workflow's
# single-line `run:` value, and the start of a shell command, which is the
# start of a line or a separator, optionally behind control keywords and
# environment assignments.
#
# The keywords and `run:` are anchored to a boundary rather than matched
# anywhere. `then`, `do`, `else` and `run:` are ordinary words elsewhere, so an
# unanchored alternative reads `echo then scripts/tool.py` as an invocation of
# a script that is really an argument to `echo`.
_DIRECT_INVOCATION = re.compile(
    r"""(?:
            ^[ \t]*(?:-[ \t]+)?run:[ \t]*
          | (?:^|[;&|(])[ \t]*(?:(?:then|do|else)[ \t]+)*
        )
        (?:[A-Za-z_]\w*=(?:"[^"]*"|'[^']*'|\S*)[ \t]+)*
        (?P<script>(?:[\w.@-]+/)+[\w.@-]+\.py)\b""",
    re.VERBOSE | re.MULTILINE,
)

# The whole specifier is captured, not a version glimpsed inside it: `<3.13`
# and `^3.13` both contain "3.13" while permitting or requiring an interpreter
# the repository does not support.
_REQUIRES_PYTHON = re.compile(r'^#\s*requires-python\s*=\s*"(?P<specifier>[^"]*)"')
_SUPPORTED_SPECIFIER = re.compile(r"^>=\s*(?P<version>\d+(?:\.\d+)*)$")

#: A quoted argument. A regular expression cannot parse a shell, so rather than
#: pretending otherwise the scanner locates quoted spans and refuses to read a
#: command out of one: `echo "(scripts/tool.py)"` contains a separator and a
#: path, and neither means what the shape of the text suggests.
_QUOTED_SPAN = re.compile(r"'[^']*'|\"[^\"]*\"")


def _quoted_ranges(line: str) -> list[tuple[int, int]]:
    """Return the character ranges of quoted arguments in one line."""
    return [match.span() for match in _QUOTED_SPAN.finditer(line)]


def command_position_scripts(text: str) -> list[str]:
    """Return the script paths that text invokes as commands.

    Matching runs a line at a time, and a match whose path falls inside a
    quoted argument is discarded: quotes are the one piece of shell structure
    that changes what a separator means, and ignoring them turns ordinary
    argument text into a call site the contract would then police.
    """
    found: list[str] = []
    for line in text.splitlines():
        quoted = _quoted_ranges(line)
        found.extend(
            match.group("script")
            for match in _DIRECT_INVOCATION.finditer(line)
            if not any(start <= match.start("script") < end for start, end in quoted)
        )
    return found


def _scanned_files() -> Iterator[Path]:
    """Yield the files that describe how CI and the Makefile call scripts."""
    yield MAKEFILE
    yield from sorted(WORKFLOW_DIRECTORY.glob("*.yml"))


def _repository_script(candidate: str) -> Path | None:
    """Return the tracked script a matched path refers to, if any."""
    path = REPO_ROOT / candidate
    return path if path.is_file() else None


def _shebang_scripts() -> list[Path]:
    """Return every repository script invoked directly by CI or the Makefile."""
    found: dict[Path, None] = {}
    for source in _scanned_files():
        text = source.read_text(encoding="utf-8")
        for candidate in command_position_scripts(text):
            script = _repository_script(candidate)
            if script is not None:
                found[script] = None
    return sorted(found)


def _index_mode(path: Path) -> str:
    """Return the file mode Git records for a path, not the checkout's mode."""
    relative = path.relative_to(REPO_ROOT).as_posix()
    result = subprocess.run(  # noqa: S603 - fixed argument vector
        ["git", "ls-files", "-s", "--", relative],  # noqa: S607 - PATH lookup is intended
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.stdout, f"{relative} is not tracked"
    return result.stdout.split()[0]


@pytest.mark.parametrize(
    "line",
    [
        pytest.param("      - run: scripts/tool.py --flag", id="inline-run"),
        pytest.param("          scripts/tool.py --flag", id="block-run"),
        pytest.param("          set -e; scripts/tool.py", id="after-separator"),
        pytest.param("          PYTHONPATH=x scripts/tool.py", id="after-assignment"),
        pytest.param(
            '          RELEASE_NOTE="candidate build" scripts/tool.py',
            id="after-quoted-assignment",
        ),
        pytest.param(
            "          A=1 B='two words' scripts/tool.py",
            id="after-several-assignments",
        ),
        pytest.param("          if x; then scripts/tool.py; fi", id="after-then"),
    ],
)
def test_command_positions_are_recognized(line: str) -> None:
    """Cover the call-site shapes a workflow may use.

    The single-line `run:` form is the one a boundary written only for shell
    syntax misses, and a missed call site makes the contract silently weaker
    rather than red.
    """
    assert command_position_scripts(line) == ["scripts/tool.py"]


@pytest.mark.parametrize(
    "line",
    [
        pytest.param("          python scripts/tool.py", id="ambient-interpreter"),
        pytest.param("          uv run --script scripts/tool.py", id="explicit-uv-run"),
        pytest.param("          echo then scripts/tool.py", id="keyword-as-argument"),
        pytest.param("          echo do scripts/tool.py", id="do-as-argument"),
        pytest.param("          grep run: scripts/tool.py", id="run-key-as-argument"),
        pytest.param('          echo "(scripts/tool.py)"', id="quoted-argument"),
        pytest.param("          echo '; scripts/tool.py'", id="quoted-separator"),
    ],
)
def test_arguments_are_not_command_positions(line: str) -> None:
    """A script that is an argument must not be read as a command.

    A false positive is not merely noise here: it would hold an unrelated path
    to the shebang and executable-bit rules, so the contract could fail on
    workflow text that is perfectly correct.
    """
    assert not command_position_scripts(line)


def test_direct_invocations_are_discovered() -> None:
    """The scan finds call sites; an empty scan would make the suite vacuous."""
    scripts = _shebang_scripts()
    assert scripts, "no directly invoked repository script was found"
    names = {script.name for script in scripts}
    assert {"check_glibc_baseline.py", "generate_checksums.py"} <= names


@pytest.mark.parametrize("script", _shebang_scripts(), ids=lambda path: path.name)
def test_directly_invoked_scripts_pin_their_interpreter(script: Path) -> None:
    """A script CI runs by path must choose its own interpreter and version."""
    lines = script.read_text(encoding="utf-8").splitlines()
    assert lines[0] == UV_SHEBANG, (
        f"{script.name} is invoked by path, so its shebang selects the "
        f"interpreter; expected {UV_SHEBANG!r}, found {lines[0]!r}"
    )
    specifiers = [
        match.group("specifier")
        for line in lines[1:12]
        if (match := _REQUIRES_PYTHON.match(line))
    ]
    assert specifiers, (
        f"{script.name} declares no requires-python in its script metadata"
    )
    supported = _SUPPORTED_SPECIFIER.match(specifiers[0])
    assert supported, (
        f"{script.name} declares requires-python {specifiers[0]!r}; the "
        'repository supports only a lower bound of the form ">=X.Y"'
    )
    version = tuple(int(part) for part in supported.group("version").split("."))
    assert version >= MINIMUM_PYTHON, (
        f"{script.name} allows Python {version}, below the repository floor"
    )


@pytest.mark.parametrize("script", _shebang_scripts(), ids=lambda path: path.name)
def test_directly_invoked_scripts_are_executable(script: Path) -> None:
    """A shebang is inert without the executable bit Git records."""
    assert _index_mode(script) == "100755", (
        f"{script.name} is invoked by path but Git records it as non-executable, "
        "which fails on a fresh checkout with exit code 126"
    )


@pytest.mark.parametrize("source", list(_scanned_files()), ids=lambda path: path.name)
def test_no_repository_script_runs_on_an_ambient_interpreter(source: Path) -> None:
    """No call site hands a repository script to whichever Python the image has."""
    offenders = [
        match.group("script")
        for match in _AMBIENT_INTERPRETER.finditer(source.read_text(encoding="utf-8"))
        if _repository_script(match.group("script")) is not None
    ]
    assert not offenders, (
        f"{source.name} runs {offenders} through an ambient interpreter; invoke "
        "the script directly so its uv shebang selects the version"
    )
