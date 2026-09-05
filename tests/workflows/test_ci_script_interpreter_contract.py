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

# A repository script in command position: at the start of a command, after a
# shell separator, or after leading environment assignments.
_DIRECT_INVOCATION = re.compile(
    r"""(?:^|[;&|(]|\bthen\b|\bdo\b|\belse\b)\s*
        (?:[A-Za-z_][\w]*=\S*\s+)*
        (?P<script>(?:[\w.@-]+/)+[\w.@-]+\.py)\b""",
    re.VERBOSE | re.MULTILINE,
)

_REQUIRES_PYTHON = re.compile(
    r'^#\s*requires-python\s*=\s*"[><=~^]*(?P<version>[\d.]+)"'
)


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
        for match in _DIRECT_INVOCATION.finditer(text):
            script = _repository_script(match.group("script"))
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
    versions = [
        tuple(int(part) for part in match.group("version").split("."))
        for line in lines[1:12]
        if (match := _REQUIRES_PYTHON.match(line))
    ]
    assert versions, f"{script.name} declares no requires-python in its script metadata"
    assert versions[0] >= MINIMUM_PYTHON, (
        f"{script.name} allows Python {versions[0]}, below the repository floor"
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
