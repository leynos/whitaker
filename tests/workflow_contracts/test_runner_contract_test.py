"""Keep every `TEST_RUNNER` override executable by the `test` recipe.

The `test` recipe runs `$(TEST_RUNNER)` directly: the runner it names is
executed as a command, not passed to `$(CARGO)`. An override therefore has to
begin with an installed binary, either a cargo subcommand variable such as
`$(CARGO_NEXTEST)` or an explicit script path. A bare `nextest` is not on
PATH (the distribution installs `cargo-nextest`), so that spelling fails at
once with `nextest: command not found`.

`make coverage-build` depends on the same rule for a second reason. Its runner
must be the `scripts/cargo-coverage-build.sh` stand-in, because that script is
what applies `cargo llvm-cov show-env`. Naming only `nextest` there compiles
nothing and aborts the main coverage-warmup lane.

These tests read the real Makefile and the scripts that drive it, so a new
override cannot reintroduce the defect that turned main red after #537.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

from __future__ import annotations

import re
from pathlib import Path

from ubicloud_workflow_support import REPOSITORY_ROOT

MAKEFILE = REPOSITORY_ROOT / "Makefile"
SCRIPTS_DIRECTORY = REPOSITORY_ROOT / "scripts"

#: A `TEST_RUNNER` assignment, quoted, wherever it appears: a Makefile recipe
#: line or a shell script. The default definition is matched separately.
_RUNNER_ASSIGNMENT = re.compile(r'TEST_RUNNER\s*[?:]?=\s*"([^"]*)"')

#: The unquoted default in the Makefile: `TEST_RUNNER ?= $(CARGO_NEXTEST) ...`.
_RUNNER_DEFAULT = re.compile(r"^TEST_RUNNER\s*\?=\s*(\S+)", re.MULTILINE)

#: First tokens that name an executable the lanes install. Everything else
#: must be a Make variable or an explicit path, so the Makefile keeps owning
#: the choice instead of relying on a PATH lookup for an absent binary.
_INSTALLED_COMMANDS = frozenset({"cargo", "cargo-nextest", "cargo-llvm-cov"})


def _recipe(target: str) -> str:
    """Return one Make target's recipe, blank lines included.

    Splitting on the first blank line would truncate the recipe, because Make
    permits a blank line inside one, which would let a later command escape
    the assertions below.
    """
    lines = MAKEFILE.read_text(encoding="utf-8").splitlines()
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{target}:")),
        None,
    )
    assert start is not None, f"{target} must be defined in the Makefile"
    body: list[str] = []
    for line in lines[start + 1 :]:
        if line and not line.startswith("\t"):
            break
        body.append(line)
    return "\n".join(body)


def _assert_executable_runner(runner: str, source: str) -> None:
    """Fail when a runner starts with a token the recipe cannot execute."""
    first = runner.split()[0]
    is_make_variable = first.startswith("$(") and first.endswith(")")
    is_explicit_path = "/" in first
    assert is_make_variable or is_explicit_path or first in _INSTALLED_COMMANDS, (
        f"{source} sets TEST_RUNNER to a command the recipe cannot execute: {first!r}"
    )


def test_the_test_recipe_runs_the_runner_as_a_command() -> None:
    """The runner is executed directly, so it must name an executable."""
    recipe = _recipe("test")

    assert "$(TEST_RUNNER)" in recipe, "the test recipe must run $(TEST_RUNNER)"
    assert "$(CARGO) $(TEST_RUNNER)" not in recipe, (
        "the test recipe must not prefix the runner with $(CARGO); overrides "
        "then have to name the executable themselves"
    )


def test_the_default_runner_names_the_cargo_subcommand() -> None:
    """Rely on the installed binary, not a bare `nextest` PATH entry."""
    match = _RUNNER_DEFAULT.search(MAKEFILE.read_text(encoding="utf-8"))

    assert match is not None, "TEST_RUNNER must keep an explicit default"
    assert match.group(1) == "$(CARGO_NEXTEST)", (
        "the default runner must start with $(CARGO_NEXTEST), because only "
        "`cargo-nextest` is installed"
    )


def test_makefile_runner_overrides_name_an_installed_command() -> None:
    """Every quoted Makefile override must begin with a runnable command."""
    runners = _RUNNER_ASSIGNMENT.findall(MAKEFILE.read_text(encoding="utf-8"))

    assert runners, "the Makefile must define at least one TEST_RUNNER override"
    for runner in runners:
        _assert_executable_runner(runner, "Makefile")


def test_the_coverage_build_runner_is_the_coverage_stand_in() -> None:
    """Compile instrumented tests through the script that sets up llvm-cov."""
    recipe = _recipe("coverage-build")

    assert 'CARGO="$(COVERAGE_BUILD_CARGO)"' in recipe, (
        "coverage-build must point nested Cargo calls at the coverage stand-in"
    )
    assert 'TEST_RUNNER="$(COVERAGE_BUILD_CARGO) nextest run' in recipe, (
        "coverage-build must run the coverage stand-in, which applies "
        "`cargo llvm-cov show-env` before `cargo nextest run --no-run`"
    )


def test_shell_script_runners_name_an_installed_command() -> None:
    """The benchmark scripts use the same runner contract as the Makefile."""
    for path in sorted(SCRIPTS_DIRECTORY.glob("*.sh")):
        runners = _RUNNER_ASSIGNMENT.findall(path.read_text(encoding="utf-8"))
        for runner in runners:
            _assert_executable_runner(runner, str(path.relative_to(REPOSITORY_ROOT)))