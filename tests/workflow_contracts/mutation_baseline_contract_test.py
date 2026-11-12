"""Guard the mutation caller's baseline on every pull request.

The scheduled mutation run tests each package with its default features.
Passing ``--all-features`` enabled ``dylint-driver`` across the workspace,
Cargo unified it into the lint crates' integration-test binaries, and those
binaries cannot link the compiler's ``rustc_private`` libraries: every shard of
run 33961579896 failed its baseline with 19 duplicate-dependency errors before
testing a mutant.

``coverage-check`` therefore runs ``make mutation-baseline``, which reproduces
the root package's baseline under the same command cargo-mutants runs.
``lane_deduplication_contract_test`` records that execution as distinct from
the all-feature run; this module holds the guard's shape: it runs in the
pull-request lane after the toolchain setup, nothing can skip or reconfigure
it, and its Make recipe selects the same package and features as the caller.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import shlex
import tomllib

import pytest
from mutation_features import feature_tokens, resolved_cargo
from pull_request_reach import declares_trigger
from ubicloud_workflow_support import (
    REPOSITORY_ROOT,
    load_job,
    load_workflow,
    step_names,
    steps_by_name,
)

MAKEFILE = REPOSITORY_ROOT / "Makefile"
ROOT_MANIFEST = REPOSITORY_ROOT / "Cargo.toml"

#: The step the coverage lane runs and the target it names.
MUTATION_BASELINE_STEP = "Run the mutation baseline"
MUTATION_BASELINE_COMMAND = "make mutation-baseline"
MUTATION_BASELINE_TARGET = "mutation-baseline"


def _make_recipe(target: str) -> str:
    """Return one Make target's recipe, blank lines included.

    Splitting on the first blank line would truncate the recipe, because Make
    permits a blank line inside one. A command placed after that line would
    then escape every assertion made about the recipe.
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


def _makefile_variable(name: str) -> str:
    """Return one Makefile variable's raw definition."""
    text = MAKEFILE.read_text(encoding="utf-8")
    match = re.search(rf"^{name} \??= (.*)$", text, re.MULTILINE)
    assert match is not None, f"{name} must be defined in the Makefile"
    return match.group(1)


def _recipe_commands(target: str) -> list[list[str]]:
    """Return each executable command line in a target's recipe, tokenized.

    ``$(CARGO)`` and ``$(CARGO_LOCKED)`` are resolved from their Makefile
    definitions, so the assertion reads the command Make would run rather
    than the spelling the recipe uses. A ``CARGO`` definition that appends
    arguments is refused, because the substitution would otherwise hide them,
    and so is Make's ``-`` prefix, which would ignore a failing baseline.

    >>> _recipe_commands("mutation-baseline")
    [['cargo', 'test', '--locked', '-p', 'whitaker']]
    """
    cargo = resolved_cargo(_makefile_variable("CARGO"))
    commands: list[list[str]] = []
    for line in _make_recipe(target).splitlines():
        command = line.strip()
        if not command or command.startswith("#"):
            continue
        assert not command.startswith("-"), (
            "mutation-baseline must not carry Make's ignore-error prefix; a "
            f"failing baseline would then pass the pull request: {command!r}"
        )
        command = command.lstrip("@+")
        if not command or command.startswith("#"):
            continue
        resolved = command.replace("$(CARGO)", cargo).replace(
            "$(CARGO_LOCKED)", _makefile_variable("CARGO_LOCKED")
        )
        commands.append(shlex.split(resolved, comments=True))
    return commands


def _root_package_name() -> str:
    """Return the root package's name from its manifest."""
    manifest = tomllib.loads(ROOT_MANIFEST.read_text(encoding="utf-8"))
    return manifest["package"]["name"]


def test_the_mutation_baseline_runs_in_the_pull_request_lane() -> None:
    """A baseline only a nightly run executes cannot guard a pull request."""
    workflow = load_workflow("ci.yml")
    assert declares_trigger(workflow, "pull_request"), (
        "ci.yml must be reachable by a pull request for its lanes to gate one"
    )
    job_if = load_job("coverage-check").get("if")
    assert job_if == "github.event_name == 'pull_request'", (
        "coverage-check must stay the pull-request lane"
    )


def test_the_mutation_baseline_follows_the_toolchain_setup() -> None:
    """A guard that runs before checkout has nothing to build."""
    names = step_names(load_job("coverage-check"))
    assert MUTATION_BASELINE_STEP in names, (
        f"coverage-check must run the {MUTATION_BASELINE_STEP!r} step"
    )
    baseline_index = names.index(MUTATION_BASELINE_STEP)
    for prerequisite in ("Checkout", "Setup Rust"):
        assert prerequisite in names, (
            f"coverage-check must keep its {prerequisite!r} step"
        )
        assert names.index(prerequisite) < baseline_index, (
            f"{MUTATION_BASELINE_STEP!r} must follow {prerequisite!r}"
        )


def test_the_mutation_baseline_step_cannot_be_skipped_or_reconfigured() -> None:
    """A condition, override or working directory would hide a link failure."""
    step = steps_by_name(load_job("coverage-check"))[MUTATION_BASELINE_STEP]
    assert step.get("run") == MUTATION_BASELINE_COMMAND, (
        f"{MUTATION_BASELINE_STEP!r} must run exactly "
        f"{MUTATION_BASELINE_COMMAND!r}, got {step.get('run')!r}"
    )
    for key in ("if", "continue-on-error", "working-directory", "env"):
        assert key not in step, (
            f"{MUTATION_BASELINE_STEP!r} must inherit the lane's environment "
            f"and cannot be skipped or allowed to fail; found {key!r}"
        )


def test_the_mutation_baseline_recipe_selects_only_the_root_package() -> None:
    """The guard must run the command cargo-mutants runs, and nothing broader."""
    commands = _recipe_commands(MUTATION_BASELINE_TARGET)
    assert len(commands) == 1, (
        "mutation-baseline must run one command, so no unasserted flag can "
        f"hide a second; got {commands!r}"
    )
    assert commands[0] == ["cargo", "test", "--locked", "-p", _root_package_name()], (
        "mutation-baseline must run the root package's default-feature "
        f"baseline exactly; got {commands[0]!r}"
    )


def test_the_mutation_baseline_matches_the_caller_feature_selection() -> None:
    """The guard must run the feature set the mutation caller selects."""
    caller = load_workflow("mutation-testing.yml")["jobs"]["mutation"]
    with_block = caller.get("with") or {}
    caller_features = feature_tokens(shlex.split(str(with_block.get("extra-args", ""))))
    recipe_features = feature_tokens(_recipe_commands(MUTATION_BASELINE_TARGET)[0])
    assert recipe_features == caller_features, (
        "mutation-baseline must select the same features as the mutation "
        f"caller's extra-args; caller {caller_features!r}, "
        f"recipe {recipe_features!r}"
    )