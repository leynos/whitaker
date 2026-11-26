"""The Makefile recipes that implement the trunk writer's compile-only pairing.

`trunk_writer_contract_test` holds the job-level rule. This module owns the
recipe-level half: each ``EQUIVALENT_WRITER_COMMANDS`` pairing must compile the
reader's shapes without running them. A plain ``cargo`` reader is compared
token for token with ``--no-run`` appended; a script reader must assign both
runner overrides and make each one compile-only.

Run via ``make test-workflow-contracts``.
"""

import re
import shlex
import typing as typ

import pytest
from mutation_features import resolved_cargo
from trunk_writer import EQUIVALENT_WRITER_COMMANDS
from ubicloud_workflow_support import REPOSITORY_ROOT

_MAKEFILE: typ.Final = REPOSITORY_ROOT / "Makefile"


def _make_recipe(makefile: str, target: str) -> str:
    """Return a Make target's recipe, stopping at the next rule."""
    lines = makefile.splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{target}:")),
        None,
    )
    assert start is not None, f"{target} must be a Makefile target"
    body: list[str] = []
    for line in lines[start + 1 :]:
        if line and not line.startswith("\t"):
            break
        body.append(line)
    return "\n".join(body)


def _makefile_variable(makefile: str, name: str) -> str:
    """Return one Makefile variable's raw definition."""
    match = re.search(rf"^{re.escape(name)} \??= (.*)$", makefile, re.MULTILINE)
    assert match is not None, f"{name} must be defined in the Makefile"
    return match.group(1)


def _recipe_commands(makefile: str, target: str) -> list[list[str]]:
    """Return each executable recipe line's words, comments excluded.

    ``$(CARGO)`` and ``$(CARGO_LOCKED)`` are resolved, so a difference in
    package or feature selection is visible in the tokens rather than hidden
    behind a variable. A continued shell line ends with a backslash, which
    ``shlex`` reads as an unterminated escape; splitting on whitespace still
    exposes the flags on that line.
    """
    cargo = resolved_cargo(_makefile_variable(makefile, "CARGO"))
    commands: list[list[str]] = []
    for line in _make_recipe(makefile, target).splitlines():
        stripped = line.strip().lstrip("@-+")
        if not stripped or stripped.startswith("#"):
            continue
        resolved = stripped.replace("$(CARGO)", cargo).replace(
            "$(CARGO_LOCKED)", _makefile_variable(makefile, "CARGO_LOCKED")
        )
        try:
            commands.append(shlex.split(resolved, comments=True))
        except ValueError:
            commands.append(resolved.split())
    return commands


@pytest.mark.parametrize(
    ("reader_command", "writer_command"), sorted(EQUIVALENT_WRITER_COMMANDS.items())
)
def test_an_equivalent_writer_command_compiles_without_running(
    reader_command: str, writer_command: str
) -> None:
    """A twin earns its place only by building the reader's shape and stopping.

    A writer recipe such as ``cargo test --no-run -p unrelated`` would satisfy
    a bare ``--no-run`` check while warming nothing the reader builds. When
    every reader command is a plain ``cargo`` command, the writer must match
    each one token for token except for that flag. A reader that runs a script
    of overrides, such as ``make coverage``, hands its shapes to ``make test``
    through both runner variables, so both must be present and compile-only;
    the test recipe's own nested-build guards still run, which is the
    delegation's documented behaviour rather than the twin's claim.
    """
    makefile = _MAKEFILE.read_text(encoding="utf-8")
    reader = _recipe_commands(makefile, reader_command.removeprefix("make "))
    writer = _recipe_commands(makefile, writer_command.removeprefix("make "))
    assert any("--no-run" in words for words in writer), (
        f"{writer_command} must compile the reader's shape without running it; "
        f"got {writer!r}"
    )
    if reader and all(words[:1] == ["cargo"] for words in reader):
        expected = [words + ["--no-run"] for words in reader]
        assert writer == expected, (
            f"{writer_command} must compile exactly what {reader_command} "
            f"builds and stop; got {writer!r}"
        )
        return
    recipe = _make_recipe(makefile, writer_command.removeprefix("make "))
    _assert_compile_only_runners(recipe, writer_command)


def _assert_compile_only_runners(recipe: str, writer_command: str) -> None:
    """Require both runner overrides, each compiling rather than running."""
    values = dict(
        re.findall(r'(?<![A-Z_])(TEST_RUNNER|CORE_TEST_RUNNER)="([^"]*)"', recipe)
    )
    for name in ("TEST_RUNNER", "CORE_TEST_RUNNER"):
        assert name in values, (
            f"{writer_command} must assign {name}=...; a missing override "
            f"would fall back to a runner that executes tests: {recipe!r}"
        )
        assert "--no-run" in values[name], (
            f"{writer_command} must pass --no-run to {name}; got {values[name]!r}"
        )


@pytest.mark.parametrize(
    ("recipe", "expected"),
    [
        pytest.param(
            'TEST_RUNNER="build --no-run" CORE_TEST_RUNNER="build --no-run"',
            None,
            id="both-compile-only",
        ),
        pytest.param(
            'TEST_RUNNER="build --no-run"',
            "must assign CORE_TEST_RUNNER",
            id="missing-core-runner",
        ),
        pytest.param(
            'TEST_RUNNER="build --no-run" CORE_TEST_RUNNER="build"',
            "must pass --no-run to CORE_TEST_RUNNER",
            id="core-runner-runs-tests",
        ),
        pytest.param(
            'TEST_RUNNER="build" CORE_TEST_RUNNER="build --no-run"',
            "must pass --no-run to TEST_RUNNER",
            id="main-runner-runs-tests",
        ),
    ],
)
def test_a_writer_must_assign_both_runner_overrides(
    recipe: str, expected: str | None
) -> None:
    """A missing or test-running override would execute what the twin warms."""
    if expected is None:
        _assert_compile_only_runners(recipe, "make coverage-build")
        return
    with pytest.raises(AssertionError, match=re.escape(expected)):
        _assert_compile_only_runners(recipe, "make coverage-build")
