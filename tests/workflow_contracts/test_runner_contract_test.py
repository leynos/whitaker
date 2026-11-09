"""Keep every `TEST_RUNNER` override executable by the `test` recipe.

The `test` recipe runs `$(TEST_RUNNER)` directly: the runner it names is
executed as a command, not passed to `$(CARGO)`. An override therefore has to
begin with an installed binary, either a cargo subcommand variable such as
`$(CARGO_NEXTEST)`, a Make expression that expands to a path, or an explicit
script path that exists. A bare `nextest` is not on PATH (the distribution
installs `cargo-nextest`), so that spelling fails at once with
`nextest: command not found`.

`make coverage-build` depends on the same rule for a second reason. Its runner
must be the `scripts/cargo-coverage-build.sh` stand-in, because that script is
what applies `cargo llvm-cov show-env`. Naming only `nextest` there compiles
nothing and aborts the main coverage-warmup lane.

Discovery covers quoted and unquoted assignments alike, because an unquoted
`TEST_RUNNER=nextest` obeys the same execution rule and would otherwise escape
the check. The Makefile's unquoted `?=` default is matched separately, so an
override is never mistaken for it.

These tests read the real Makefile and the scripts that drive it, so a new
override cannot reintroduce the defect that turned main red after #537. Each
override is its own test case, so one bad runner does not hide the rest.

Run this contract with:

```sh
make test-workflow-contracts
```
"""

from __future__ import annotations

import os
import re
from pathlib import Path

import pytest

from ubicloud_workflow_support import REPOSITORY_ROOT

MAKEFILE = REPOSITORY_ROOT / "Makefile"
SCRIPTS_DIRECTORY = REPOSITORY_ROOT / "scripts"
COVERAGE_BUILD_SCRIPT = SCRIPTS_DIRECTORY / "cargo-coverage-build.sh"
BENCHMARK_SCRIPT = SCRIPTS_DIRECTORY / "run-nextest-split-benchmark.sh"

#: A `TEST_RUNNER` override, quoted or bare, wherever it appears: a Makefile
#: recipe line or a shell script. The Makefile's unquoted `?=` default is
#: matched separately, so this pattern deliberately accepts only `=`.
_RUNNER_ASSIGNMENT = re.compile(r'TEST_RUNNER\s*=\s*(?:"([^"]*)"|(\S+))')

#: The unquoted default in the Makefile: `TEST_RUNNER ?= $(CARGO_NEXTEST) ...`.
_RUNNER_DEFAULT = re.compile(r"^TEST_RUNNER\s*\?=\s*(\S+)", re.MULTILINE)

#: The default path of the coverage stand-in script.
_COVERAGE_BUILD_CARGO = re.compile(
    r"^COVERAGE_BUILD_CARGO\s*\?=\s*\$\(CURDIR\)/(\S+)", re.MULTILINE
)

#: First tokens that name an executable the lanes install. Everything else
#: must be a Make expression or an existing script path, so the Makefile keeps
#: owning the choice instead of relying on a PATH lookup for an absent binary.
_INSTALLED_COMMANDS = frozenset({"cargo", "cargo-nextest", "cargo-llvm-cov"})

#: The exact wiring `coverage-build` must hand the stand-in. Splitting the
#: string keeps the Make and shell quoting readable.
_COVERAGE_BUILD_RUNNER = (
    'TEST_RUNNER="$(COVERAGE_BUILD_CARGO) nextest run --no-run -E '
    "'$(COVERAGE_NEXTEST_FILTER)'\""
)


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


def _assigned_runners(text: str) -> list[str]:
    """Return every override value in `text`, whether quoted or bare.

    `finditer` is used rather than `findall` because the latter reports a
    non-participating group as an empty string, which would make an unquoted
    value indistinguishable from a quoted empty one.
    """
    runners: list[str] = []
    for match in _RUNNER_ASSIGNMENT.finditer(text):
        quoted = match.group(1)
        if quoted is not None:
            runners.append(quoted)
            continue
        bare = match.group(2)
        assert bare is not None, "the alternation must match exactly one branch"
        runners.append(bare)
    return runners


def _assert_executable_runner(runner: str, source: str) -> None:
    """Fail when a runner starts with a token the recipe cannot execute."""
    tokens = runner.split()
    assert tokens, f"{source} sets TEST_RUNNER to an empty command"

    first = tokens[0]
    if first.startswith(("$(", "${")):
        # A Make expansion such as `$(CARGO_NEXTEST)` or
        # `$(CURDIR)/scripts/…`: Make owns the resolved command.
        return
    if "/" in first:
        assert (REPOSITORY_ROOT / first).is_file(), (
            f"{source} sets TEST_RUNNER to a path that does not exist: {first!r}"
        )
        return
    assert first in _INSTALLED_COMMANDS, (
        f"{source} sets TEST_RUNNER to a command the recipe cannot execute: {first!r}"
    )


def _makefile_runners() -> list[str]:
    """Return the Makefile's `TEST_RUNNER` overrides, default excluded."""
    return _assigned_runners(MAKEFILE.read_text(encoding="utf-8"))


def _script_runners() -> list[object]:
    """Return one case per `TEST_RUNNER` override in the shell scripts."""
    cases: list[object] = []
    for path in sorted(SCRIPTS_DIRECTORY.glob("*.sh")):
        name = str(path.relative_to(REPOSITORY_ROOT))
        for index, runner in enumerate(
            _assigned_runners(path.read_text(encoding="utf-8")), start=1
        ):
            cases.append(pytest.param(name, runner, id=f"{name}:{index}"))
    return cases


_MAKEFILE_RUNNERS = _makefile_runners()
_SCRIPT_RUNNERS = _script_runners()


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


def test_the_makefile_defines_runner_overrides() -> None:
    """A vanished override would silently empty the per-case checks below."""
    assert _MAKEFILE_RUNNERS, "the Makefile must define at least one TEST_RUNNER override"


def test_the_scripts_define_runner_overrides() -> None:
    """A vanished script override would silently empty the per-case checks."""
    assert _SCRIPT_RUNNERS, "the scripts must define at least one TEST_RUNNER override"


@pytest.mark.parametrize("runner", _MAKEFILE_RUNNERS)
def test_makefile_runner_overrides_name_an_installed_command(runner: str) -> None:
    """Every Makefile override must begin with a runnable command."""
    _assert_executable_runner(runner, "Makefile")


@pytest.mark.parametrize(("script", "runner"), _SCRIPT_RUNNERS)
def test_shell_script_runners_name_an_installed_command(
    script: str, runner: str
) -> None:
    """The benchmark scripts use the same runner contract as the Makefile."""
    _assert_executable_runner(runner, script)


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        pytest.param(
            'TEST_RUNNER="cargo-nextest nextest run"',
            ["cargo-nextest nextest run"],
            id="quoted",
        ),
        pytest.param(
            "TEST_RUNNER=cargo-nextest",
            ["cargo-nextest"],
            id="unquoted",
        ),
        pytest.param(
            "TEST_RUNNER?=$(CARGO_NEXTEST) nextest run",
            [],
            id="default-is-not-an-override",
        ),
    ],
)
def test_runner_discovery_covers_quoted_and_unquoted_values(
    text: str, expected: list[str]
) -> None:
    """Discovery must not let an unquoted override escape the checks."""
    assert _assigned_runners(text) == expected


@pytest.mark.parametrize("runner", ["", "   "], ids=["empty", "whitespace"])
def test_an_empty_runner_is_rejected(runner: str) -> None:
    """An empty runner fails with the contract's message, not an IndexError."""
    with pytest.raises(AssertionError, match="empty command"):
        _assert_executable_runner(runner, "fixture")


@pytest.mark.parametrize(
    "runner",
    ["nextest run", "nextest list --message-format json"],
    ids=["bare-nextest", "bare-nextest-list"],
)
def test_a_bare_nextest_runner_is_rejected(runner: str) -> None:
    """A bare `nextest` is the exact spelling that broke coverage-warmup."""
    with pytest.raises(AssertionError, match="cannot execute"):
        _assert_executable_runner(runner, "fixture")


def test_a_missing_script_path_is_rejected() -> None:
    """An explicit path still has to exist before the recipe can run it."""
    with pytest.raises(AssertionError, match="does not exist"):
        _assert_executable_runner("scripts/no-such-runner.sh", "fixture")


def test_the_coverage_build_runner_is_the_coverage_stand_in() -> None:
    """Compile instrumented tests through the script that sets up llvm-cov."""
    recipe = _recipe("coverage-build")

    assert 'CARGO="$(COVERAGE_BUILD_CARGO)"' in recipe, (
        "coverage-build must point nested Cargo calls at the coverage stand-in"
    )
    assert _COVERAGE_BUILD_RUNNER in recipe, (
        "coverage-build must run the coverage stand-in with `nextest run "
        "--no-run` and the coverage filter, so the warmed cache matches "
        "`make coverage`"
    )


def test_the_coverage_stand_in_is_an_executable_script() -> None:
    """The path `coverage-build` names must resolve to a runnable script."""
    match = _COVERAGE_BUILD_CARGO.search(MAKEFILE.read_text(encoding="utf-8"))

    assert match is not None, "COVERAGE_BUILD_CARGO must keep an explicit default"
    script = REPOSITORY_ROOT / match.group(1)
    assert script.is_file(), f"{script} must be the coverage stand-in"
    assert os.access(script, os.X_OK), f"{script} must be executable"


def test_the_benchmark_runners_pass_the_required_nextest_arguments() -> None:
    """A dropped subcommand or argument would measure the wrong lane."""
    runners = _assigned_runners(BENCHMARK_SCRIPT.read_text(encoding="utf-8"))

    assert any(
        "nextest run" in runner and "--config-file" in runner for runner in runners
    ), "the benchmark run must pass `nextest run` and its generated config"
    assert any(
        "nextest list" in runner and "--message-format json" in runner
        for runner in runners
    ), "the membership discovery must pass `nextest list` and JSON output"